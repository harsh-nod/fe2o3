//! Exact production KIR-to-LLVM-to-final-HSACO semantic anchors.
//!
//! This additive contract describes compiler-inserted LLVM pseudo-probe anchors. An anchor is a
//! correspondence point, not complete machine-instruction coverage, a schedule, or a refinement
//! proof.

use std::{error::Error, fmt};

use dialect_amdgcn::{
    CanonicalProductionKirToLlvmReplayEvidenceV1, MAX_PRODUCTION_SEMANTIC_ANCHORS_V1,
    ProductionKirToLlvmReplayModeV1, ProductionReplayKernelIrIdentityV1,
    ProductionReplayKernelIrVersionV1,
};
use fe2o3_compiler_lineage::InertSemanticToLlvmAssociationV3;
use fe2o3_kernel_ir::{Module, ProductionSemanticDebugReceiptExtensionV1, SemanticDebugLocationV1};
use object::{Object, ObjectSection, ObjectSymbol, SymbolKind};
use sha2::{Digest, Sha256};

use crate::{
    ContentIdentityV1, PreparedFinalizedProtectedWorkerV3HsacoV1,
    semantic_debug_map_v1::validate_production_association,
};

const GUID_DOMAIN_V1: &[u8] = b"FE2O3/PRODUCTION-KIR-LLVM-ISA-ANCHOR-GUID/V1\0";
const HASH_DOMAIN_V1: &[u8] = b"FE2O3/PRODUCTION-KIR-LLVM-ISA-ANCHOR-HASH/V1\0";
const IDENTITY_DOMAIN_V1: &[u8] = b"FE2O3/PRODUCTION-KIR-LLVM-ISA-ANCHOR-ID/V1\0";
const MAX_ANCHORS_V1: usize = MAX_PRODUCTION_SEMANTIC_ANCHORS_V1;
const MAX_ANCHOR_METADATA_DEFINITIONS_V1: usize = MAX_ANCHORS_V1 + 3;
const MAX_PROBE_SECTION_BYTES_V1: usize = 64 * 1024 * 1024;

/// What happened to one compiler-inserted pseudo-probe record during backend lowering.
///
/// These values describe anchor-record cardinality and address sharing only. They do not prove
/// that the corresponding KIR operation itself was preserved, duplicated, coalesced, or
/// eliminated in machine code.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductionSemanticAnchorTransformationV1 {
    Preserved,
    Duplicated,
    Coalesced,
    DuplicatedAndCoalesced,
    Eliminated,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductionSemanticAnchorUnavailableV1 {
    LegacySemanticAttachment,
    LegacyUninstrumentedReplay,
    NoOperations,
    MultipleDefinedBodies,
    CompilerInstrumentationAbsent,
}

#[derive(Debug)]
pub enum ProductionSemanticAnchorAdmissionV1 {
    Admitted(Box<AdmittedProductionSemanticAnchorsV1>),
    Unavailable(ProductionSemanticAnchorUnavailableV1),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdmittedProductionSemanticAnchorV1 {
    semantic_operation_id: [u8; 32],
    kir: SemanticDebugLocationV1,
    compiler_handoff_llvm: SemanticDebugLocationV1,
    isa: Vec<SemanticDebugLocationV1>,
    transformation: ProductionSemanticAnchorTransformationV1,
}

impl AdmittedProductionSemanticAnchorV1 {
    pub const fn semantic_operation_id(&self) -> &[u8; 32] {
        &self.semantic_operation_id
    }

    pub const fn kir(&self) -> SemanticDebugLocationV1 {
        self.kir
    }

    /// Exact LLVM coordinate in the compiler handoff consumed by the Worker.
    ///
    /// It does not name optimized LLVM retained inside the Worker or final LLVM after O3.
    pub const fn compiler_handoff_llvm(&self) -> SemanticDebugLocationV1 {
        self.compiler_handoff_llvm
    }

    pub fn isa(&self) -> &[SemanticDebugLocationV1] {
        &self.isa
    }

    pub const fn transformation(&self) -> ProductionSemanticAnchorTransformationV1 {
        self.transformation
    }

    /// This contract does not infer machine-lowering shape from pseudo-probe cardinality.
    pub const fn proves_kir_operation_machine_shape(&self) -> bool {
        false
    }

    pub const fn proves_optimized_or_final_llvm_custody(&self) -> bool {
        false
    }
}

#[derive(Debug)]
pub struct AdmittedProductionSemanticAnchorsV1 {
    artifact_identity: ContentIdentityV1,
    target_bound_kir_version: ProductionReplayKernelIrVersionV1,
    target_bound_kir_sha256: [u8; 32],
    target_bound_kir_bytes: u64,
    target: String,
    pseudo_probe_desc_bytes: u64,
    pseudo_probe_bytes: u64,
    anchors: Vec<AdmittedProductionSemanticAnchorV1>,
}

impl AdmittedProductionSemanticAnchorsV1 {
    pub const fn artifact_identity(&self) -> ContentIdentityV1 {
        self.artifact_identity
    }

    pub const fn target_bound_kir_sha256(&self) -> &[u8; 32] {
        &self.target_bound_kir_sha256
    }

    pub const fn target_bound_kir_version(&self) -> ProductionReplayKernelIrVersionV1 {
        self.target_bound_kir_version
    }

    pub const fn target_bound_kir_bytes(&self) -> u64 {
        self.target_bound_kir_bytes
    }

    pub fn target(&self) -> &str {
        &self.target
    }

    /// Exact final-artifact bytes occupied by the descriptor section.
    pub const fn pseudo_probe_desc_bytes(&self) -> u64 {
        self.pseudo_probe_desc_bytes
    }

    /// Exact final-artifact bytes occupied by the address-bearing probe section.
    pub const fn pseudo_probe_bytes(&self) -> u64 {
        self.pseudo_probe_bytes
    }

    pub fn anchors(&self) -> &[AdmittedProductionSemanticAnchorV1] {
        &self.anchors
    }

    pub const fn proves_complete_machine_instruction_coverage(&self) -> bool {
        false
    }

    pub const fn proves_a_schedule(&self) -> bool {
        false
    }

    pub const fn proves_semantic_refinement(&self) -> bool {
        false
    }

    /// V1 starts at exact target-bound production KIR coordinates and does not join the frozen V7
    /// Source/MIR/KIR carrier to ISA. The target-bound identity can be V8 or V9, according to the
    /// exact replay evidence.
    pub const fn proves_source_to_isa_round_trip(&self) -> bool {
        false
    }

    pub const fn grants_execution_authority(&self) -> bool {
        false
    }

    pub const fn proves_general_executable_bytes_unchanged(&self) -> bool {
        false
    }

    pub const fn proves_general_resource_metadata_unchanged(&self) -> bool {
        false
    }

    pub const fn proves_zero_runtime_or_code_size_overhead(&self) -> bool {
        false
    }
}

impl PreparedFinalizedProtectedWorkerV3HsacoV1 {
    /// Replays exact target-bound KIR custody, parses compiler anchors from the exact compiler
    /// handoff consumed by the Worker, and
    /// independently joins the corresponding final-HSACO pseudo-probe addresses to metadata-order
    /// kernel entry ranges.
    pub fn admit_production_semantic_anchors_v1(
        &self,
    ) -> Result<ProductionSemanticAnchorAdmissionV1, ProductionSemanticAnchorErrorV1> {
        let outer = self.outer_handoff();
        let receipts = outer.capsule().receipts();
        let receipt_bytes = receipts.semantic_to_llvm().canonical_preimage();
        let extension =
            match ProductionSemanticDebugReceiptExtensionV1::from_canonical_bytes(receipt_bytes) {
                Ok(extension) => extension,
                Err(_) if InertSemanticToLlvmAssociationV3::decode(receipt_bytes).is_ok() => {
                    return Ok(ProductionSemanticAnchorAdmissionV1::Unavailable(
                        ProductionSemanticAnchorUnavailableV1::LegacySemanticAttachment,
                    ));
                }
                Err(_) => return Err(ProductionSemanticAnchorErrorV1::InvalidCompilerAttachment),
            };
        let association = InertSemanticToLlvmAssociationV3::decode(extension.association_v3())
            .map_err(|_| ProductionSemanticAnchorErrorV1::InvalidProductionAssociation)?;
        validate_production_association(outer, association.inputs())
            .map_err(|_| ProductionSemanticAnchorErrorV1::InvalidProductionAssociation)?;
        let _source_carrier_status = extension.carrier_v1().availability();

        let replay = CanonicalProductionKirToLlvmReplayEvidenceV1::decode(
            receipts.amdgpu_lowering().canonical_preimage(),
        )
        .and_then(|evidence| {
            evidence.validate_against_neutral_kernel_ir(receipts.kernel_ir().canonical_preimage())
        })
        .map_err(|_| ProductionSemanticAnchorErrorV1::InvalidKirToLlvmReplay)?;
        if replay.llvm_mode() != ProductionKirToLlvmReplayModeV1::SemanticAnchorsV1 {
            return Ok(ProductionSemanticAnchorAdmissionV1::Unavailable(
                ProductionSemanticAnchorUnavailableV1::LegacyUninstrumentedReplay,
            ));
        }
        let expected_target = replay.evidence().profile().device_target();
        if expected_target != self.target().to_string()
            || expected_target != outer.module_handoff().target().to_string()
        {
            return Err(ProductionSemanticAnchorErrorV1::TargetMismatch);
        }
        let expected_kir =
            ExpectedTargetKirIdentityV1::from(replay.evidence().target_bound_kernel_ir_identity());
        let llvm = std::str::from_utf8(outer.module_handoff().module_bytes())
            .map_err(|_| ProductionSemanticAnchorErrorV1::InvalidLlvm)?;
        let replay_llvm = replay.evidence().pre_descriptor_llvm();
        let replay_absence = parse_llvm_absence_v1(replay_llvm, expected_kir, expected_target)?;
        let handoff_absence = parse_llvm_absence_v1(llvm, expected_kir, expected_target)?;
        if replay_absence != handoff_absence {
            return Err(ProductionSemanticAnchorErrorV1::KirToLlvmAnchorMismatch);
        }
        let replay_manifest = parse_llvm_manifest_v1(replay_llvm, expected_kir, expected_target)?;
        let Some(manifest) = parse_llvm_manifest_v1(llvm, expected_kir, expected_target)? else {
            let expected_absence = expected_absence_v1(replay.target_bound_module());
            if replay_absence != expected_absence {
                return Err(ProductionSemanticAnchorErrorV1::ContradictoryLlvm);
            }
            let unavailable = match expected_absence {
                Some(LlvmAnchorAbsenceV1::NoOperations) => {
                    ProductionSemanticAnchorUnavailableV1::NoOperations
                }
                Some(LlvmAnchorAbsenceV1::MultipleDefinedBodies) => {
                    ProductionSemanticAnchorUnavailableV1::MultipleDefinedBodies
                }
                None => ProductionSemanticAnchorUnavailableV1::CompilerInstrumentationAbsent,
            };
            return Ok(ProductionSemanticAnchorAdmissionV1::Unavailable(
                unavailable,
            ));
        };
        if replay_absence.is_some() {
            return Err(ProductionSemanticAnchorErrorV1::ContradictoryLlvm);
        }
        if replay_manifest.as_ref() != Some(&manifest) {
            return Err(ProductionSemanticAnchorErrorV1::KirToLlvmAnchorMismatch);
        }
        validate_manifest_against_kir(&manifest, replay.target_bound_module())?;
        let admitted = admit_final_artifact_v1(
            manifest,
            replay.target_bound_module(),
            self.exact_finalized_bytes(),
        )?;
        Ok(ProductionSemanticAnchorAdmissionV1::Admitted(Box::new(
            admitted,
        )))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct LlvmAnchorRecordV1 {
    probe_index: u64,
    function_ordinal: u64,
    block_ordinal: u64,
    operation_ordinal: u64,
    llvm_function_ordinal: u64,
    llvm_block_ordinal: u64,
    llvm_instruction_ordinal: u64,
}

#[derive(Debug, Eq, PartialEq)]
struct LlvmAnchorManifestV1 {
    target_bound_kir_version: ProductionReplayKernelIrVersionV1,
    target_bound_kir_sha256: [u8; 32],
    target_bound_kir_bytes: u64,
    target: String,
    guid: u64,
    function_hash: u64,
    records: Vec<LlvmAnchorRecordV1>,
}

#[derive(Clone, Copy)]
struct ExpectedTargetKirIdentityV1 {
    version: ProductionReplayKernelIrVersionV1,
    sha256: [u8; 32],
    byte_len: u64,
}

impl From<ProductionReplayKernelIrIdentityV1> for ExpectedTargetKirIdentityV1 {
    fn from(value: ProductionReplayKernelIrIdentityV1) -> Self {
        Self {
            version: value.version(),
            sha256: value.sha256(),
            byte_len: value.byte_len(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum LlvmAnchorAbsenceV1 {
    NoOperations,
    MultipleDefinedBodies,
}

fn expected_absence_v1(module: &Module) -> Option<LlvmAnchorAbsenceV1> {
    let defined_body_count = module
        .functions
        .iter()
        .filter(|function| function.body.is_some())
        .count();
    if defined_body_count > 1 {
        return Some(LlvmAnchorAbsenceV1::MultipleDefinedBodies);
    }
    let has_operations = module
        .functions
        .iter()
        .filter_map(|function| function.body.as_ref())
        .flat_map(|body| &body.blocks)
        .any(|block| !block.operations.is_empty());
    (!has_operations).then_some(LlvmAnchorAbsenceV1::NoOperations)
}

fn parse_llvm_absence_v1(
    llvm: &str,
    expected_kir: ExpectedTargetKirIdentityV1,
    expected_target: &str,
) -> Result<Option<LlvmAnchorAbsenceV1>, ProductionSemanticAnchorErrorV1> {
    let mut named = None;
    for line in llvm
        .lines()
        .filter(|line| line.starts_with("!fe2o3.semantic_anchor.absence.v1 = "))
    {
        if named.replace(line).is_some() {
            return Err(ProductionSemanticAnchorErrorV1::ContradictoryLlvm);
        }
    }
    let Some(named) = named else {
        if llvm.contains("fe2o3.semantic_anchor.absence.v1") {
            return Err(ProductionSemanticAnchorErrorV1::ContradictoryLlvm);
        }
        return Ok(None);
    };
    if llvm.contains("!fe2o3.semantic_anchor.v1 = ")
        || llvm.contains("@llvm.pseudoprobe")
        || llvm.contains("!llvm.pseudo_probe_desc")
    {
        return Err(ProductionSemanticAnchorErrorV1::ContradictoryLlvm);
    }
    let references = parse_metadata_references(
        named
            .strip_prefix("!fe2o3.semantic_anchor.absence.v1 = ")
            .ok_or(ProductionSemanticAnchorErrorV1::InvalidLlvm)?,
    )?;
    if references.len() != 1 {
        return Err(ProductionSemanticAnchorErrorV1::ContradictoryLlvm);
    }
    let definitions = metadata_definitions(llvm)?;
    let fields = metadata_fields(
        definitions
            .get(&references[0])
            .ok_or(ProductionSemanticAnchorErrorV1::InvalidLlvm)?,
    )?;
    if fields.len() != 5 {
        return Err(ProductionSemanticAnchorErrorV1::InvalidLlvm);
    }
    let reason = match parse_metadata_string(fields[0], "")?.as_str() {
        "no_operations" => LlvmAnchorAbsenceV1::NoOperations,
        "multiple_defined_bodies" => LlvmAnchorAbsenceV1::MultipleDefinedBodies,
        _ => return Err(ProductionSemanticAnchorErrorV1::ContradictoryLlvm),
    };
    let kir = parse_prefixed_sha256(fields[1], "sha256:")?;
    let version = parse_metadata_string(fields[2], "kir-version:")?;
    let version = match version.as_str() {
        "8" => ProductionReplayKernelIrVersionV1::V8,
        "9" => ProductionReplayKernelIrVersionV1::V9,
        _ => return Err(ProductionSemanticAnchorErrorV1::BindingMismatch),
    };
    let byte_len = parse_i64_field(fields[3])?;
    let target = parse_metadata_string(fields[4], "target:")?;
    if kir != expected_kir.sha256
        || version != expected_kir.version
        || byte_len != expected_kir.byte_len
        || target != expected_target
    {
        return Err(ProductionSemanticAnchorErrorV1::BindingMismatch);
    }
    Ok(Some(reason))
}

fn parse_llvm_manifest_v1(
    llvm: &str,
    expected_kir: ExpectedTargetKirIdentityV1,
    expected_target: &str,
) -> Result<Option<LlvmAnchorManifestV1>, ProductionSemanticAnchorErrorV1> {
    let mut named = None;
    for line in llvm
        .lines()
        .filter(|line| line.starts_with("!fe2o3.semantic_anchor.v1 = "))
    {
        if named.replace(line).is_some() {
            return Err(ProductionSemanticAnchorErrorV1::ContradictoryLlvm);
        }
    }
    let Some(named) = named else {
        if llvm.contains("@llvm.pseudoprobe") || llvm.contains("!llvm.pseudo_probe_desc") {
            return Err(ProductionSemanticAnchorErrorV1::ContradictoryLlvm);
        }
        return Ok(None);
    };

    let definitions = metadata_definitions(llvm)?;
    let references = parse_metadata_references(
        named
            .strip_prefix("!fe2o3.semantic_anchor.v1 = ")
            .ok_or(ProductionSemanticAnchorErrorV1::InvalidLlvm)?,
    )?;
    if references.len() < 2 {
        return Err(ProductionSemanticAnchorErrorV1::InvalidLlvm);
    }
    let mut referenced = Vec::new();
    referenced
        .try_reserve_exact(references.len())
        .map_err(|_| ProductionSemanticAnchorErrorV1::AllocationFailure)?;
    referenced.extend_from_slice(&references);
    referenced.sort_unstable();
    if referenced.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(ProductionSemanticAnchorErrorV1::ContradictoryLlvm);
    }
    let binding_reference = references[0];
    let binding = definitions
        .get(&binding_reference)
        .ok_or(ProductionSemanticAnchorErrorV1::InvalidLlvm)?;
    let fields = metadata_fields(binding)?;
    if fields.len() != 8 {
        return Err(ProductionSemanticAnchorErrorV1::InvalidLlvm);
    }
    let kir = parse_prefixed_sha256(fields[0], "sha256:")?;
    let kir_version = match parse_metadata_string(fields[1], "kir-version:")?.as_str() {
        "8" => ProductionReplayKernelIrVersionV1::V8,
        "9" => ProductionReplayKernelIrVersionV1::V9,
        _ => return Err(ProductionSemanticAnchorErrorV1::BindingMismatch),
    };
    let kir_bytes = parse_i64_field(fields[2])?;
    let target = parse_metadata_string(fields[3], "target:")?;
    let guid = parse_i64_field(fields[4])?;
    let function_hash = parse_i64_field(fields[5])?;
    let block_count = parse_i64_field(fields[6])?;
    let operation_count = parse_i64_field(fields[7])?;
    let block_count_usize =
        usize::try_from(block_count).map_err(|_| ProductionSemanticAnchorErrorV1::ResourceLimit)?;
    let operation_count_usize = usize::try_from(operation_count)
        .map_err(|_| ProductionSemanticAnchorErrorV1::ResourceLimit)?;
    if kir != expected_kir.sha256
        || kir_version != expected_kir.version
        || kir_bytes != expected_kir.byte_len
        || target != expected_target
        || guid == 0
        || function_hash == 0
        || operation_count == 0
        || operation_count_usize != references.len() - 1
        || operation_count_usize > MAX_ANCHORS_V1
        || block_count_usize > MAX_ANCHORS_V1
    {
        return Err(ProductionSemanticAnchorErrorV1::BindingMismatch);
    }
    let mut descriptor_line = None;
    for line in llvm
        .lines()
        .filter(|line| line.starts_with("!llvm.pseudo_probe_desc = "))
    {
        if descriptor_line.replace(line).is_some() {
            return Err(ProductionSemanticAnchorErrorV1::ContradictoryLlvm);
        }
    }
    let descriptor_line =
        descriptor_line.ok_or(ProductionSemanticAnchorErrorV1::ContradictoryLlvm)?;
    let descriptor_references = parse_metadata_references(
        descriptor_line
            .strip_prefix("!llvm.pseudo_probe_desc = ")
            .ok_or(ProductionSemanticAnchorErrorV1::InvalidLlvm)?,
    )?;
    if descriptor_references.len() != 1
        || referenced.binary_search(&descriptor_references[0]).is_ok()
    {
        return Err(ProductionSemanticAnchorErrorV1::ContradictoryLlvm);
    }
    let descriptor_reference = descriptor_references[0];
    let descriptor = metadata_fields(
        definitions
            .get(&descriptor_reference)
            .ok_or(ProductionSemanticAnchorErrorV1::InvalidLlvm)?,
    )?;
    if descriptor.len() != 3
        || parse_i64_field(descriptor[0])? != guid
        || parse_i64_field(descriptor[1])? != function_hash
    {
        return Err(ProductionSemanticAnchorErrorV1::BindingMismatch);
    }
    let _diagnostic_descriptor_name = parse_metadata_string(descriptor[2], "")?;

    let mut records = Vec::new();
    records
        .try_reserve_exact(references.len() - 1)
        .map_err(|_| ProductionSemanticAnchorErrorV1::AllocationFailure)?;
    for (expected_index, reference) in references[1..].iter().copied().enumerate() {
        let fields = metadata_fields(
            definitions
                .get(&reference)
                .ok_or(ProductionSemanticAnchorErrorV1::InvalidLlvm)?,
        )?;
        if fields.len() != 4 {
            return Err(ProductionSemanticAnchorErrorV1::InvalidLlvm);
        }
        let record = LlvmAnchorRecordV1 {
            probe_index: parse_i64_field(fields[0])?,
            function_ordinal: parse_i64_field(fields[1])?,
            block_ordinal: parse_i64_field(fields[2])?,
            operation_ordinal: parse_i64_field(fields[3])?,
            llvm_function_ordinal: 0,
            llvm_block_ordinal: 0,
            llvm_instruction_ordinal: 0,
        };
        if record.probe_index != (expected_index + 1) as u64 {
            return Err(ProductionSemanticAnchorErrorV1::InvalidLlvm);
        }
        records.push(record);
    }

    bind_exact_llvm_locations(llvm, guid, &mut records)?;
    let function_ordinal = records
        .first()
        .ok_or(ProductionSemanticAnchorErrorV1::InvalidLlvm)?
        .function_ordinal;
    let expected_guid = anchor_digest(
        GUID_DOMAIN_V1,
        AnchorDigestInputV1 {
            kir_version,
            kir,
            kir_bytes,
            target: &target,
            function: function_ordinal,
            blocks: block_count,
            operations: operation_count,
        },
    );
    let expected_hash = anchor_digest(
        HASH_DOMAIN_V1,
        AnchorDigestInputV1 {
            kir_version,
            kir,
            kir_bytes,
            target: &target,
            function: function_ordinal,
            blocks: block_count,
            operations: operation_count,
        },
    );
    if guid != expected_guid || function_hash != expected_hash {
        return Err(ProductionSemanticAnchorErrorV1::BindingMismatch);
    }
    Ok(Some(LlvmAnchorManifestV1 {
        target_bound_kir_version: kir_version,
        target_bound_kir_sha256: kir,
        target_bound_kir_bytes: kir_bytes,
        target,
        guid,
        function_hash,
        records,
    }))
}

struct MetadataDefinitionsV1<'a> {
    entries: Vec<(usize, &'a str)>,
}

impl<'a> MetadataDefinitionsV1<'a> {
    fn get(&self, number: &usize) -> Option<&&'a str> {
        self.entries
            .binary_search_by_key(number, |(number, _)| *number)
            .ok()
            .map(|index| &self.entries[index].1)
    }
}

fn metadata_definitions(
    llvm: &str,
) -> Result<MetadataDefinitionsV1<'_>, ProductionSemanticAnchorErrorV1> {
    let mut entries = Vec::new();
    for line in llvm.lines() {
        let Some(rest) = line.strip_prefix('!') else {
            continue;
        };
        let Some((number, value)) = rest.split_once(" = ") else {
            continue;
        };
        let Ok(number) = number.parse::<usize>() else {
            continue;
        };
        // Active manifests own one workgroup definition, one descriptor, one
        // binding, and one definition per semantic anchor record.
        if entries.len() >= MAX_ANCHOR_METADATA_DEFINITIONS_V1 {
            return Err(ProductionSemanticAnchorErrorV1::ResourceLimit);
        }
        entries
            .try_reserve(1)
            .map_err(|_| ProductionSemanticAnchorErrorV1::AllocationFailure)?;
        entries.push((number, value));
    }
    entries.sort_unstable_by_key(|(number, _)| *number);
    if entries.windows(2).any(|pair| pair[0].0 == pair[1].0) {
        return Err(ProductionSemanticAnchorErrorV1::ContradictoryLlvm);
    }
    Ok(MetadataDefinitionsV1 { entries })
}

fn parse_metadata_references(value: &str) -> Result<Vec<usize>, ProductionSemanticAnchorErrorV1> {
    let inner = value
        .strip_prefix("!{")
        .and_then(|value| value.strip_suffix('}'))
        .ok_or(ProductionSemanticAnchorErrorV1::InvalidLlvm)?;
    let mut result = Vec::new();
    for field in inner.split(", ") {
        if result.len() > MAX_ANCHORS_V1 {
            return Err(ProductionSemanticAnchorErrorV1::ResourceLimit);
        }
        result
            .try_reserve(1)
            .map_err(|_| ProductionSemanticAnchorErrorV1::AllocationFailure)?;
        result.push(
            field
                .strip_prefix('!')
                .ok_or(ProductionSemanticAnchorErrorV1::InvalidLlvm)?
                .parse()
                .map_err(|_| ProductionSemanticAnchorErrorV1::InvalidLlvm)?,
        );
    }
    Ok(result)
}

fn metadata_fields(value: &str) -> Result<Vec<&str>, ProductionSemanticAnchorErrorV1> {
    let inner = value
        .strip_prefix("!{")
        .and_then(|value| value.strip_suffix('}'))
        .ok_or(ProductionSemanticAnchorErrorV1::InvalidLlvm)?;
    let mut fields = Vec::new();
    for field in inner.split(", ") {
        if fields.len() >= 16 {
            return Err(ProductionSemanticAnchorErrorV1::InvalidLlvm);
        }
        fields
            .try_reserve(1)
            .map_err(|_| ProductionSemanticAnchorErrorV1::AllocationFailure)?;
        fields.push(field);
    }
    Ok(fields)
}

fn parse_i64_field(value: &str) -> Result<u64, ProductionSemanticAnchorErrorV1> {
    value
        .strip_prefix("i64 ")
        .ok_or(ProductionSemanticAnchorErrorV1::InvalidLlvm)?
        .parse()
        .map_err(|_| ProductionSemanticAnchorErrorV1::InvalidLlvm)
}

fn parse_metadata_string(
    value: &str,
    prefix: &str,
) -> Result<String, ProductionSemanticAnchorErrorV1> {
    let value = value
        .strip_prefix("!\"")
        .and_then(|value| value.strip_suffix('"'))
        .and_then(|value| value.strip_prefix(prefix))
        .ok_or(ProductionSemanticAnchorErrorV1::InvalidLlvm)?;
    if value.is_empty() || !value.is_ascii() || value.contains(['"', '\\']) {
        return Err(ProductionSemanticAnchorErrorV1::InvalidLlvm);
    }
    let mut owned = String::new();
    owned
        .try_reserve_exact(value.len())
        .map_err(|_| ProductionSemanticAnchorErrorV1::AllocationFailure)?;
    owned.push_str(value);
    Ok(owned)
}

fn parse_prefixed_sha256(
    value: &str,
    prefix: &str,
) -> Result<[u8; 32], ProductionSemanticAnchorErrorV1> {
    let text = parse_metadata_string(value, prefix)?;
    if text.len() != 64 {
        return Err(ProductionSemanticAnchorErrorV1::InvalidLlvm);
    }
    let mut result = [0_u8; 32];
    for (index, pair) in text.as_bytes().chunks_exact(2).enumerate() {
        result[index] = (hex_nibble(pair[0])? << 4) | hex_nibble(pair[1])?;
    }
    Ok(result)
}

fn hex_nibble(value: u8) -> Result<u8, ProductionSemanticAnchorErrorV1> {
    match value {
        b'0'..=b'9' => Ok(value - b'0'),
        b'a'..=b'f' => Ok(value - b'a' + 10),
        _ => Err(ProductionSemanticAnchorErrorV1::InvalidLlvm),
    }
}

fn bind_exact_llvm_locations(
    llvm: &str,
    guid: u64,
    records: &mut [LlvmAnchorRecordV1],
) -> Result<(), ProductionSemanticAnchorErrorV1> {
    let mut function = None::<u64>;
    let mut next_function = 0_u64;
    let mut block = None::<u64>;
    let mut instruction = 0_u64;
    let mut seen = Vec::new();
    seen.try_reserve_exact(records.len())
        .map_err(|_| ProductionSemanticAnchorErrorV1::AllocationFailure)?;
    seen.resize(records.len(), false);
    for line in llvm.lines() {
        if line.starts_with("define ") {
            function = Some(next_function);
            next_function += 1;
            block = None;
            instruction = 0;
            continue;
        }
        if line == "}" {
            function = None;
            block = None;
            continue;
        }
        if function.is_some() && !line.starts_with(' ') && line.ends_with(':') {
            block = Some(block.map_or(0, |value| value + 1));
            instruction = 0;
            continue;
        }
        let (Some(function), Some(block)) = (function, block) else {
            continue;
        };
        if !line.starts_with("  ") || line.trim_start().starts_with(';') || line.trim().is_empty() {
            continue;
        }
        if let Some(arguments) = line
            .trim()
            .strip_prefix("call void @llvm.pseudoprobe(")
            .and_then(|line| line.strip_suffix(')'))
        {
            let mut fields = arguments.split(", ");
            let guid_field = fields.next();
            let probe_field = fields.next();
            let type_field = fields.next();
            let discriminator_field = fields.next();
            if fields.next().is_some()
                || guid_field.map(parse_i64_field).transpose()? != Some(guid)
                || type_field != Some("i32 0")
                || discriminator_field != Some("i64 -1")
            {
                return Err(ProductionSemanticAnchorErrorV1::ContradictoryLlvm);
            }
            let probe_index = probe_field
                .ok_or(ProductionSemanticAnchorErrorV1::ContradictoryLlvm)
                .and_then(parse_i64_field)?;
            let record_index = probe_index
                .checked_sub(1)
                .and_then(|index| usize::try_from(index).ok())
                .ok_or(ProductionSemanticAnchorErrorV1::ContradictoryLlvm)?;
            let record = records
                .get_mut(record_index)
                .filter(|record| record.probe_index == probe_index)
                .ok_or(ProductionSemanticAnchorErrorV1::ContradictoryLlvm)?;
            let was_seen = seen
                .get_mut(record_index)
                .ok_or(ProductionSemanticAnchorErrorV1::ContradictoryLlvm)?;
            if *was_seen {
                return Err(ProductionSemanticAnchorErrorV1::ContradictoryLlvm);
            }
            *was_seen = true;
            record.llvm_function_ordinal = function;
            record.llvm_block_ordinal = block;
            record.llvm_instruction_ordinal = instruction;
        }
        instruction += 1;
    }
    if seen.iter().any(|seen| !seen) {
        return Err(ProductionSemanticAnchorErrorV1::ContradictoryLlvm);
    }
    Ok(())
}

fn validate_manifest_against_kir(
    manifest: &LlvmAnchorManifestV1,
    module: &Module,
) -> Result<(), ProductionSemanticAnchorErrorV1> {
    let expected = module
        .functions
        .iter()
        .enumerate()
        .filter_map(|(function, definition)| definition.body.as_ref().map(|body| (function, body)))
        .flat_map(|(function, body)| {
            body.blocks
                .iter()
                .enumerate()
                .map(move |(block, definition)| (function, block, definition))
        })
        .flat_map(|(function, block, definition)| {
            (0..definition.operations.len())
                .map(move |operation| (function as u64, block as u64, operation as u64))
        });
    if manifest
        .records
        .iter()
        .map(|record| {
            (
                record.function_ordinal,
                record.block_ordinal,
                record.operation_ordinal,
            )
        })
        .ne(expected)
    {
        return Err(ProductionSemanticAnchorErrorV1::KirCoordinateMismatch);
    }
    Ok(())
}

fn admit_final_artifact_v1(
    manifest: LlvmAnchorManifestV1,
    _module: &Module,
    artifact: &[u8],
) -> Result<AdmittedProductionSemanticAnchorsV1, ProductionSemanticAnchorErrorV1> {
    validate_amdgpu_et_dyn(artifact)?;
    let object = object::File::parse(artifact)
        .map_err(|_| ProductionSemanticAnchorErrorV1::InvalidArtifact)?;
    let desc = unique_section(&object, ".pseudo_probe_desc")?;
    let probes = unique_section(&object, ".pseudo_probe")?;
    if desc.len() > MAX_PROBE_SECTION_BYTES_V1 || probes.len() > MAX_PROBE_SECTION_BYTES_V1 {
        return Err(ProductionSemanticAnchorErrorV1::ResourceLimit);
    }
    let descriptors = decode_probe_descriptors(desc)?;
    if descriptors.len() != 1
        || descriptors[0].guid != manifest.guid
        || descriptors[0].function_hash != manifest.function_hash
    {
        return Err(ProductionSemanticAnchorErrorV1::ProbeDescriptorMismatch);
    }

    let inspected = crate::inspect_and_bind_kernel_descriptors(artifact)
        .map_err(|_| ProductionSemanticAnchorErrorV1::InvalidArtifact)?;
    let symbol = sole_metadata_entry_symbol(&object, &inspected)?;
    let decoded = decode_probe_records(probes, manifest.guid, symbol.entry_address)?;
    let mut addresses = Vec::<Vec<u64>>::new();
    addresses
        .try_reserve_exact(manifest.records.len())
        .map_err(|_| ProductionSemanticAnchorErrorV1::AllocationFailure)?;
    addresses.resize_with(manifest.records.len(), Vec::new);
    let mut raw_record_counts = Vec::new();
    raw_record_counts
        .try_reserve_exact(manifest.records.len())
        .map_err(|_| ProductionSemanticAnchorErrorV1::AllocationFailure)?;
    raw_record_counts.resize(manifest.records.len(), 0_usize);
    for record in decoded {
        let record_index = record
            .probe_index
            .checked_sub(1)
            .and_then(|index| usize::try_from(index).ok())
            .ok_or(ProductionSemanticAnchorErrorV1::UnexpectedProbe)?;
        if manifest
            .records
            .get(record_index)
            .is_none_or(|expected| expected.probe_index != record.probe_index)
        {
            return Err(ProductionSemanticAnchorErrorV1::UnexpectedProbe);
        }
        let count = raw_record_counts
            .get_mut(record_index)
            .ok_or(ProductionSemanticAnchorErrorV1::UnexpectedProbe)?;
        *count = count
            .checked_add(1)
            .ok_or(ProductionSemanticAnchorErrorV1::ResourceLimit)?;
        let record_addresses = addresses
            .get_mut(record_index)
            .ok_or(ProductionSemanticAnchorErrorV1::UnexpectedProbe)?;
        record_addresses
            .try_reserve(1)
            .map_err(|_| ProductionSemanticAnchorErrorV1::AllocationFailure)?;
        record_addresses.push(record.address);
    }
    for values in &mut addresses {
        values.sort_unstable();
        values.dedup();
    }

    let unique_address_count = addresses.iter().try_fold(0_usize, |total, values| {
        total
            .checked_add(values.len())
            .ok_or(ProductionSemanticAnchorErrorV1::ResourceLimit)
    })?;
    let mut address_owners = Vec::<(u64, usize)>::new();
    address_owners
        .try_reserve_exact(unique_address_count)
        .map_err(|_| ProductionSemanticAnchorErrorV1::AllocationFailure)?;
    for (record_index, values) in addresses.iter().enumerate() {
        for address in values {
            address_owners.push((*address, record_index));
        }
    }
    address_owners.sort_unstable();
    let mut coalesced = Vec::new();
    coalesced
        .try_reserve_exact(manifest.records.len())
        .map_err(|_| ProductionSemanticAnchorErrorV1::AllocationFailure)?;
    coalesced.resize(manifest.records.len(), false);
    let mut owner_start = 0;
    while owner_start < address_owners.len() {
        let mut owner_end = owner_start + 1;
        while owner_end < address_owners.len()
            && address_owners[owner_end].0 == address_owners[owner_start].0
        {
            owner_end += 1;
        }
        if owner_end - owner_start > 1 {
            for (_, record_index) in &address_owners[owner_start..owner_end] {
                *coalesced
                    .get_mut(*record_index)
                    .ok_or(ProductionSemanticAnchorErrorV1::UnexpectedProbe)? = true;
            }
        }
        owner_start = owner_end;
    }

    let entry_end = symbol
        .entry_address
        .checked_add(symbol.entry_size)
        .ok_or(ProductionSemanticAnchorErrorV1::InvalidArtifact)?;
    let mut admitted = Vec::new();
    admitted
        .try_reserve_exact(manifest.records.len())
        .map_err(|_| ProductionSemanticAnchorErrorV1::AllocationFailure)?;
    for (record_index, record) in manifest.records.into_iter().enumerate() {
        let values = std::mem::take(
            addresses
                .get_mut(record_index)
                .ok_or(ProductionSemanticAnchorErrorV1::UnexpectedProbe)?,
        );
        let transformation = classify_transformation(
            *raw_record_counts
                .get(record_index)
                .ok_or(ProductionSemanticAnchorErrorV1::UnexpectedProbe)?,
            *coalesced
                .get(record_index)
                .ok_or(ProductionSemanticAnchorErrorV1::UnexpectedProbe)?,
        );
        let mut isa = Vec::new();
        isa.try_reserve_exact(values.len())
            .map_err(|_| ProductionSemanticAnchorErrorV1::AllocationFailure)?;
        for address in values {
            let end = address
                .checked_add(4)
                .ok_or(ProductionSemanticAnchorErrorV1::ProbeOutsideKernel)?;
            if address < symbol.entry_address
                || end > entry_end
                || !address.is_multiple_of(4)
                || !is_backed_by_entry_bytes(artifact, &symbol, address, end)?
            {
                return Err(ProductionSemanticAnchorErrorV1::ProbeOutsideKernel);
            }
            isa.push(SemanticDebugLocationV1::Isa {
                kernel_ordinal: symbol.kernel_ordinal as u64,
                byte_start: address - symbol.entry_address,
                byte_end: end - symbol.entry_address,
            });
        }
        let semantic_operation_id = semantic_operation_identity(
            manifest.target_bound_kir_version,
            manifest.target_bound_kir_sha256,
            manifest.target_bound_kir_bytes,
            &manifest.target,
            record.function_ordinal,
            record.block_ordinal,
            record.operation_ordinal,
        );
        admitted.push(AdmittedProductionSemanticAnchorV1 {
            semantic_operation_id,
            kir: SemanticDebugLocationV1::Kir {
                function_ordinal: record.function_ordinal,
                block_ordinal: record.block_ordinal,
                operation_ordinal: record.operation_ordinal,
            },
            compiler_handoff_llvm: SemanticDebugLocationV1::Llvm {
                function_ordinal: record.llvm_function_ordinal,
                block_ordinal: record.llvm_block_ordinal,
                instruction_ordinal: record.llvm_instruction_ordinal,
            },
            isa,
            transformation,
        });
    }
    Ok(AdmittedProductionSemanticAnchorsV1 {
        artifact_identity: ContentIdentityV1::calculate(artifact),
        target_bound_kir_version: manifest.target_bound_kir_version,
        target_bound_kir_sha256: manifest.target_bound_kir_sha256,
        target_bound_kir_bytes: manifest.target_bound_kir_bytes,
        target: manifest.target,
        pseudo_probe_desc_bytes: desc.len() as u64,
        pseudo_probe_bytes: probes.len() as u64,
        anchors: admitted,
    })
}

fn classify_transformation(
    raw_record_count: usize,
    coalesced: bool,
) -> ProductionSemanticAnchorTransformationV1 {
    match (raw_record_count, coalesced) {
        (0, _) => ProductionSemanticAnchorTransformationV1::Eliminated,
        (1, false) => ProductionSemanticAnchorTransformationV1::Preserved,
        (1, true) => ProductionSemanticAnchorTransformationV1::Coalesced,
        (_, false) => ProductionSemanticAnchorTransformationV1::Duplicated,
        (_, true) => ProductionSemanticAnchorTransformationV1::DuplicatedAndCoalesced,
    }
}

struct ProbeDescriptorV1 {
    guid: u64,
    function_hash: u64,
}

fn decode_probe_descriptors(
    bytes: &[u8],
) -> Result<Vec<ProbeDescriptorV1>, ProductionSemanticAnchorErrorV1> {
    let mut cursor = Cursor::new(bytes);
    let mut result = Vec::new();
    while !cursor.is_empty() {
        if result.len() == MAX_ANCHORS_V1 {
            return Err(ProductionSemanticAnchorErrorV1::ResourceLimit);
        }
        let guid = cursor.u64()?;
        let function_hash = cursor.u64()?;
        let name_length = cursor.uleb()?;
        let name = cursor.bytes(
            usize::try_from(name_length)
                .map_err(|_| ProductionSemanticAnchorErrorV1::InvalidProbeEncoding)?,
        )?;
        let name = std::str::from_utf8(name)
            .map_err(|_| ProductionSemanticAnchorErrorV1::InvalidProbeEncoding)?;
        if guid == 0 || function_hash == 0 || name.is_empty() || !name.is_ascii() {
            return Err(ProductionSemanticAnchorErrorV1::InvalidProbeEncoding);
        }
        result
            .try_reserve(1)
            .map_err(|_| ProductionSemanticAnchorErrorV1::AllocationFailure)?;
        result.push(ProbeDescriptorV1 {
            guid,
            function_hash,
        });
    }
    Ok(result)
}

struct DecodedProbeV1 {
    probe_index: u64,
    address: u64,
}

fn decode_probe_records(
    bytes: &[u8],
    expected_guid: u64,
    function_start: u64,
) -> Result<Vec<DecodedProbeV1>, ProductionSemanticAnchorErrorV1> {
    let mut cursor = Cursor::new(bytes);
    let mut result = Vec::new();
    let mut group_count = 0_usize;
    let mut total_record_count = 0_usize;
    while !cursor.is_empty() {
        group_count = group_count
            .checked_add(1)
            .ok_or(ProductionSemanticAnchorErrorV1::ResourceLimit)?;
        if group_count > MAX_ANCHORS_V1 {
            return Err(ProductionSemanticAnchorErrorV1::ResourceLimit);
        }
        let guid = cursor.u64()?;
        let count = cursor.uleb()?;
        let inline_count = cursor.uleb()?;
        let count = usize::try_from(count)
            .map_err(|_| ProductionSemanticAnchorErrorV1::InvalidProbeEncoding)?;
        if guid != expected_guid || inline_count != 0 || count > MAX_ANCHORS_V1 {
            return Err(ProductionSemanticAnchorErrorV1::InvalidProbeEncoding);
        }
        if count > cursor.remaining() / 3 {
            return Err(ProductionSemanticAnchorErrorV1::InvalidProbeEncoding);
        }
        total_record_count = total_record_count
            .checked_add(count)
            .ok_or(ProductionSemanticAnchorErrorV1::ResourceLimit)?;
        if total_record_count > MAX_ANCHORS_V1 {
            return Err(ProductionSemanticAnchorErrorV1::ResourceLimit);
        }
        result
            .try_reserve(count)
            .map_err(|_| ProductionSemanticAnchorErrorV1::AllocationFailure)?;
        let mut last_address = function_start;
        for _ in 0..count {
            let probe_index = cursor.uleb()?;
            let packed = cursor.u8()?;
            let probe_type = packed & 0x0f;
            let attributes = (packed >> 4) & 0x07;
            let is_delta = packed & 0x80 != 0;
            let address = if is_delta {
                last_address
                    .checked_add(cursor.uleb()?)
                    .ok_or(ProductionSemanticAnchorErrorV1::InvalidProbeEncoding)?
            } else {
                cursor.u64()?
            };
            if attributes & 4 != 0 {
                let _ = cursor.uleb()?;
            }
            if attributes & 2 != 0 {
                if probe_index != 0 || probe_type != 0 {
                    return Err(ProductionSemanticAnchorErrorV1::InvalidProbeEncoding);
                }
                continue;
            }
            if probe_type != 0 || probe_index == 0 || attributes != 0 {
                return Err(ProductionSemanticAnchorErrorV1::InvalidProbeEncoding);
            }
            last_address = address;
            result.push(DecodedProbeV1 {
                probe_index,
                address,
            });
        }
    }
    Ok(result)
}

struct EntrySymbolV1 {
    kernel_ordinal: usize,
    entry_address: u64,
    entry_size: u64,
    entry_file_offset: u64,
}

#[derive(Clone, Copy)]
struct EntryTextSymbolFactV1 {
    defined_text: bool,
    address: u64,
    size: u64,
}

fn is_sole_metadata_ordinal(
    kernel_count: usize,
    binding_count: usize,
    kernel_index: usize,
) -> bool {
    kernel_count == 1 && binding_count == 1 && kernel_index == 0
}

fn has_unique_bound_entry_symbol(
    entry_address: u64,
    entry_size: u64,
    facts: impl Iterator<Item = EntryTextSymbolFactV1>,
) -> bool {
    if entry_size == 0 {
        return false;
    }
    let mut exact = 0_usize;
    let mut same_address = 0_usize;
    for fact in facts.filter(|fact| fact.defined_text && fact.address == entry_address) {
        same_address = match same_address.checked_add(1) {
            Some(count) => count,
            None => return false,
        };
        if fact.size == entry_size {
            exact = match exact.checked_add(1) {
                Some(count) => count,
                None => return false,
            };
        }
    }
    exact == 1 && same_address == 1
}

/// Selects the exact sole-kernel entry from the already reconciled AMDHSA metadata binding.
/// Symbol and metadata names never choose or validate the final join candidate.
fn sole_metadata_entry_symbol(
    object: &object::File<'_>,
    inspected: &fe2o3_hsaco::InspectedKernelBindings,
) -> Result<EntrySymbolV1, ProductionSemanticAnchorErrorV1> {
    let [binding] = inspected.bindings() else {
        return Err(ProductionSemanticAnchorErrorV1::AmbiguousEntrySymbol);
    };
    if !is_sole_metadata_ordinal(
        inspected.inspection().kernels().len(),
        inspected.bindings().len(),
        binding.kernel_index(),
    ) {
        return Err(ProductionSemanticAnchorErrorV1::AmbiguousEntrySymbol);
    }
    let _kernel = inspected
        .inspection()
        .kernels()
        .get(binding.kernel_index())
        .filter(|_| inspected.inspection().kernels().len() == 1)
        .ok_or(ProductionSemanticAnchorErrorV1::AmbiguousEntrySymbol)?;

    if !has_unique_bound_entry_symbol(
        binding.entry_address(),
        binding.entry_size(),
        object.symbols().map(|symbol| EntryTextSymbolFactV1 {
            defined_text: symbol.kind() == SymbolKind::Text && !symbol.is_undefined(),
            address: symbol.address(),
            size: symbol.size(),
        }),
    ) {
        return Err(ProductionSemanticAnchorErrorV1::AmbiguousEntrySymbol);
    }
    Ok(EntrySymbolV1 {
        kernel_ordinal: binding.kernel_index(),
        entry_address: binding.entry_address(),
        entry_size: binding.entry_size(),
        entry_file_offset: binding.entry_file_offset(),
    })
}

#[cfg(test)]
mod entry_selection_tests {
    use super::*;

    const fn fact(address: u64, size: u64) -> EntryTextSymbolFactV1 {
        EntryTextSymbolFactV1 {
            defined_text: true,
            address,
            size,
        }
    }

    #[test]
    fn sole_metadata_selection_rejects_multi_kernel_and_wrong_ordinal() {
        assert!(is_sole_metadata_ordinal(1, 1, 0));
        assert!(!is_sole_metadata_ordinal(2, 1, 0));
        assert!(!is_sole_metadata_ordinal(1, 2, 0));
        assert!(!is_sole_metadata_ordinal(1, 1, 1));
    }

    #[test]
    fn numeric_entry_selection_rejects_aliases_and_tolerates_decoys() {
        let exact = fact(0x1000, 64);
        let decoy = fact(0x2000, 64);
        assert!(has_unique_bound_entry_symbol(
            0x1000,
            64,
            [exact, decoy].into_iter()
        ));
        for alias in [fact(0x1000, 0), fact(0x1000, 32), fact(0x1000, 64)] {
            assert!(!has_unique_bound_entry_symbol(
                0x1000,
                64,
                [exact, alias].into_iter()
            ));
        }
        assert!(!has_unique_bound_entry_symbol(
            0x1000,
            0,
            [fact(0x1000, 0)].into_iter()
        ));
    }
}

fn unique_section<'a>(
    object: &'a object::File<'a>,
    name: &str,
) -> Result<&'a [u8], ProductionSemanticAnchorErrorV1> {
    let mut matching = object
        .sections()
        .filter(|section| section.name() == Ok(name));
    let section = matching
        .next()
        .ok_or(ProductionSemanticAnchorErrorV1::MissingProbeSection)?;
    if matching.next().is_some() {
        return Err(ProductionSemanticAnchorErrorV1::AmbiguousProbeSection);
    }
    section
        .data()
        .map_err(|_| ProductionSemanticAnchorErrorV1::InvalidArtifact)
}

fn is_backed_by_entry_bytes(
    artifact: &[u8],
    symbol: &EntrySymbolV1,
    start: u64,
    end: u64,
) -> Result<bool, ProductionSemanticAnchorErrorV1> {
    let relative_start = start
        .checked_sub(symbol.entry_address)
        .ok_or(ProductionSemanticAnchorErrorV1::ProbeOutsideKernel)?;
    let relative_end = end
        .checked_sub(symbol.entry_address)
        .ok_or(ProductionSemanticAnchorErrorV1::ProbeOutsideKernel)?;
    let file_start = symbol
        .entry_file_offset
        .checked_add(relative_start)
        .and_then(|value| usize::try_from(value).ok())
        .ok_or(ProductionSemanticAnchorErrorV1::InvalidArtifact)?;
    let file_end = symbol
        .entry_file_offset
        .checked_add(relative_end)
        .and_then(|value| usize::try_from(value).ok())
        .ok_or(ProductionSemanticAnchorErrorV1::InvalidArtifact)?;
    Ok(file_start < file_end && file_end <= artifact.len())
}

fn validate_amdgpu_et_dyn(bytes: &[u8]) -> Result<(), ProductionSemanticAnchorErrorV1> {
    if bytes.len() < 64
        || &bytes[..4] != b"\x7fELF"
        || bytes[4] != 2
        || bytes[5] != 1
        || u16::from_le_bytes([bytes[16], bytes[17]]) != 3
        || u16::from_le_bytes([bytes[18], bytes[19]]) != 224
    {
        return Err(ProductionSemanticAnchorErrorV1::InvalidArtifact);
    }
    Ok(())
}

struct AnchorDigestInputV1<'a> {
    kir_version: ProductionReplayKernelIrVersionV1,
    kir: [u8; 32],
    kir_bytes: u64,
    target: &'a str,
    function: u64,
    blocks: u64,
    operations: u64,
}

fn anchor_digest(domain: &[u8], input: AnchorDigestInputV1<'_>) -> u64 {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update([match input.kir_version {
        ProductionReplayKernelIrVersionV1::V8 => 8,
        ProductionReplayKernelIrVersionV1::V9 => 9,
    }]);
    hasher.update(input.kir);
    hasher.update(input.kir_bytes.to_le_bytes());
    hasher.update((input.target.len() as u64).to_le_bytes());
    hasher.update(input.target.as_bytes());
    hasher.update(input.function.to_le_bytes());
    hasher.update(input.blocks.to_le_bytes());
    hasher.update(input.operations.to_le_bytes());
    let digest = hasher.finalize();
    let mut prefix = [0_u8; 8];
    prefix.copy_from_slice(&digest[..8]);
    let value = u64::from_le_bytes(prefix);
    if value == 0 { 1 } else { value }
}

fn semantic_operation_identity(
    kir_version: ProductionReplayKernelIrVersionV1,
    kir: [u8; 32],
    kir_bytes: u64,
    target: &str,
    function: u64,
    block: u64,
    operation: u64,
) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(IDENTITY_DOMAIN_V1);
    hasher.update([match kir_version {
        ProductionReplayKernelIrVersionV1::V8 => 8,
        ProductionReplayKernelIrVersionV1::V9 => 9,
    }]);
    hasher.update(kir);
    hasher.update(kir_bytes.to_le_bytes());
    hasher.update((target.len() as u64).to_le_bytes());
    hasher.update(target.as_bytes());
    hasher.update(function.to_le_bytes());
    hasher.update(block.to_le_bytes());
    hasher.update(operation.to_le_bytes());
    hasher.finalize().into()
}

struct Cursor<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Cursor<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn is_empty(&self) -> bool {
        self.offset == self.bytes.len()
    }

    fn remaining(&self) -> usize {
        self.bytes.len() - self.offset
    }

    fn bytes(&mut self, length: usize) -> Result<&'a [u8], ProductionSemanticAnchorErrorV1> {
        let end = self
            .offset
            .checked_add(length)
            .filter(|end| *end <= self.bytes.len())
            .ok_or(ProductionSemanticAnchorErrorV1::InvalidProbeEncoding)?;
        let result = &self.bytes[self.offset..end];
        self.offset = end;
        Ok(result)
    }

    fn u8(&mut self) -> Result<u8, ProductionSemanticAnchorErrorV1> {
        Ok(self.bytes(1)?[0])
    }

    fn u64(&mut self) -> Result<u64, ProductionSemanticAnchorErrorV1> {
        let bytes: [u8; 8] = self
            .bytes(8)?
            .try_into()
            .map_err(|_| ProductionSemanticAnchorErrorV1::InvalidProbeEncoding)?;
        Ok(u64::from_le_bytes(bytes))
    }

    fn uleb(&mut self) -> Result<u64, ProductionSemanticAnchorErrorV1> {
        let mut value = 0_u64;
        for shift in (0..=63).step_by(7) {
            let byte = self.u8()?;
            let payload = u64::from(byte & 0x7f);
            if shift == 63 && payload > 1 {
                return Err(ProductionSemanticAnchorErrorV1::InvalidProbeEncoding);
            }
            value |= payload << shift;
            if byte & 0x80 == 0 {
                if shift != 0 && payload == 0 {
                    return Err(ProductionSemanticAnchorErrorV1::InvalidProbeEncoding);
                }
                return Ok(value);
            }
        }
        Err(ProductionSemanticAnchorErrorV1::InvalidProbeEncoding)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductionSemanticAnchorErrorV1 {
    InvalidCompilerAttachment,
    InvalidProductionAssociation,
    InvalidKirToLlvmReplay,
    TargetMismatch,
    InvalidLlvm,
    ContradictoryLlvm,
    BindingMismatch,
    KirCoordinateMismatch,
    KirToLlvmAnchorMismatch,
    InvalidArtifact,
    MissingProbeSection,
    AmbiguousProbeSection,
    InvalidProbeEncoding,
    ProbeDescriptorMismatch,
    AmbiguousEntrySymbol,
    UnexpectedProbe,
    ProbeOutsideKernel,
    ResourceLimit,
    AllocationFailure,
}

impl fmt::Display for ProductionSemanticAnchorErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "production semantic anchor admission failed: {self:?}"
        )
    }
}

impl Error for ProductionSemanticAnchorErrorV1 {}

#[cfg(test)]
mod tests {
    use super::*;
    use dialect_amdgcn::{
        ProductionSemanticAnchorKirIdentityV1,
        lower_kernel_to_gfx942_xnack_minus_llvm_ir_with_semantic_anchors_v1,
    };
    use fe2o3_kernel_ir::{
        BasicBlock, BlockId, Constant, Function, Kernel, KernelId, LaunchDomain, LaunchExtent,
        Module, Operation, OperationKind, ScalarType, Signature, Terminator, Type, ValueDef,
        ValueId, VerifiedCanonicalKernelIrV8, WorkgroupSize, gfx942_xnack_minus_target_capability,
    };

    const TARGET: &str = "gfx942:xnack-";

    fn llvm_fixture(ids: [usize; 4], swap_markers: bool) -> (String, ExpectedTargetKirIdentityV1) {
        let kir = [7_u8; 32];
        let kir_identity = ExpectedTargetKirIdentityV1 {
            version: ProductionReplayKernelIrVersionV1::V8,
            sha256: kir,
            byte_len: 123,
        };
        let guid = anchor_digest(
            GUID_DOMAIN_V1,
            AnchorDigestInputV1 {
                kir_version: kir_identity.version,
                kir,
                kir_bytes: kir_identity.byte_len,
                target: TARGET,
                function: 0,
                blocks: 2,
                operations: 2,
            },
        );
        let hash = anchor_digest(
            HASH_DOMAIN_V1,
            AnchorDigestInputV1 {
                kir_version: kir_identity.version,
                kir,
                kir_bytes: kir_identity.byte_len,
                target: TARGET,
                function: 0,
                blocks: 2,
                operations: 2,
            },
        );
        let (first, second) = if swap_markers { (2, 1) } else { (1, 2) };
        let [descriptor, header, record_one, record_two] = ids;
        (
            format!(
                concat!(
                    "declare void @llvm.pseudoprobe(i64, i64, i32, i64)\n",
                    "define amdgpu_kernel void @kernel() {{\n",
                    "bb0:\n",
                    "  call void @llvm.pseudoprobe(i64 {guid}, i64 {first}, i32 0, i64 -1)\n",
                    "  %value = add i32 1, 2\n",
                    "bb1:\n",
                    "  call void @llvm.pseudoprobe(i64 {guid}, i64 {second}, i32 0, i64 -1)\n",
                    "  ret void\n",
                    "}}\n",
                    "!llvm.pseudo_probe_desc = !{{!{descriptor}}}\n",
                    "!fe2o3.semantic_anchor.v1 = !{{!{header}, !{record_one}, !{record_two}}}\n",
                    "!{descriptor} = !{{i64 {guid}, i64 {hash}, !\"kernel\"}}\n",
                    "!{header} = !{{!\"sha256:{kir_hex}\", !\"kir-version:8\", i64 123, !\"target:{target}\", i64 {guid}, i64 {hash}, i64 2, i64 2}}\n",
                    "!{record_one} = !{{i64 1, i64 0, i64 0, i64 0}}\n",
                    "!{record_two} = !{{i64 2, i64 0, i64 1, i64 0}}\n",
                ),
                kir_hex = "07".repeat(32),
                target = TARGET,
                guid = guid,
                hash = hash,
                first = first,
                second = second,
                descriptor = descriptor,
                header = header,
                record_one = record_one,
                record_two = record_two,
            ),
            kir_identity,
        )
    }

    #[test]
    fn llvm_metadata_renumbering_has_no_authority() {
        let (canonical, kir) = llvm_fixture([1, 2, 3, 4], false);
        let (renumbered, _) = llvm_fixture([91, 17, 42, 8], false);
        let canonical = parse_llvm_manifest_v1(&canonical, kir, TARGET).unwrap();
        let renumbered = parse_llvm_manifest_v1(&renumbered, kir, TARGET).unwrap();
        assert_eq!(canonical, renumbered);
    }

    #[test]
    fn maximum_actual_anchor_lowering_fits_exact_metadata_definition_bound() {
        let mut block = BasicBlock::new(BlockId(0));
        block.operations = (0..MAX_ANCHORS_V1)
            .map(|index| {
                Operation::effect_free(
                    ValueDef::new(
                        ValueId(u32::try_from(index).unwrap()),
                        Type::Scalar(ScalarType::U32),
                    ),
                    OperationKind::Constant(Constant::U32(7)),
                )
            })
            .collect();
        block.terminator = Some(Terminator::Return { values: Vec::new() });
        let mut function = Function::kernel_entry(
            "anchor_limit_impl",
            Signature::new(Vec::new(), Vec::new()),
            Vec::new(),
            vec![block],
        );
        let target = gfx942_xnack_minus_target_capability();
        function.required_capabilities.insert(target.clone());
        let mut kernel = Kernel::new(
            "anchor_limit",
            "anchor_limit_impl",
            LaunchDomain::D1 {
                x: LaunchExtent::Static(1),
            },
        );
        kernel.workgroup_size = Some(WorkgroupSize::new(64, 1, 1));
        kernel.required_capabilities.insert(target.clone());
        let mut module = Module::new("tests::anchor_limit");
        module.required_capabilities.insert(target);
        module.functions.push(function);
        module.kernels.push(kernel);

        let owner = VerifiedCanonicalKernelIrV8::from_module(module.clone()).unwrap();
        let anchor_identity = ProductionSemanticAnchorKirIdentityV1::from_v8(&owner);
        let expected = ExpectedTargetKirIdentityV1 {
            version: ProductionReplayKernelIrVersionV1::V8,
            sha256: anchor_identity.sha256(),
            byte_len: anchor_identity.byte_len(),
        };
        let llvm = lower_kernel_to_gfx942_xnack_minus_llvm_ir_with_semantic_anchors_v1(
            &module,
            &KernelId::new("anchor_limit"),
            anchor_identity,
        )
        .unwrap();
        let manifest = parse_llvm_manifest_v1(&llvm, expected, TARGET)
            .unwrap()
            .unwrap();
        assert_eq!(manifest.records.len(), MAX_ANCHORS_V1);
        assert_eq!(
            metadata_definitions(&llvm).unwrap().entries.len(),
            MAX_ANCHOR_METADATA_DEFINITIONS_V1
        );

        let one_over = format!("{llvm}!999999 = !{{i64 0}}\n");
        assert_eq!(
            parse_llvm_manifest_v1(&one_over, expected, TARGET),
            Err(ProductionSemanticAnchorErrorV1::ResourceLimit)
        );
    }

    #[test]
    fn llvm_anchor_parser_rejects_cross_references_duplicates_and_moved_markers() {
        let (canonical, kir) = llvm_fixture([91, 17, 42, 8], false);
        let (moved, _) = llvm_fixture([91, 17, 42, 8], true);
        assert_ne!(
            parse_llvm_manifest_v1(&canonical, kir, TARGET).unwrap(),
            parse_llvm_manifest_v1(&moved, kir, TARGET).unwrap()
        );

        let cross_referenced = canonical.replace(
            "!llvm.pseudo_probe_desc = !{!91}",
            "!llvm.pseudo_probe_desc = !{!17}",
        );
        assert_eq!(
            parse_llvm_manifest_v1(&cross_referenced, kir, TARGET),
            Err(ProductionSemanticAnchorErrorV1::ContradictoryLlvm)
        );

        let duplicated = canonical.replace(
            "!fe2o3.semantic_anchor.v1 = !{!17, !42, !8}",
            "!fe2o3.semantic_anchor.v1 = !{!17, !42, !42}",
        );
        assert_eq!(
            parse_llvm_manifest_v1(&duplicated, kir, TARGET),
            Err(ProductionSemanticAnchorErrorV1::ContradictoryLlvm)
        );
    }

    #[test]
    fn llvm_absence_parser_is_exact_and_rejects_hostile_markers() {
        let (_, kir) = llvm_fixture([1, 2, 3, 4], false);
        let no_operations = format!(
            concat!(
                "define amdgpu_kernel void @kernel() {{\n",
                "bb0:\n",
                "  ret void\n",
                "}}\n",
                "!fe2o3.semantic_anchor.absence.v1 = !{{!9}}\n",
                "!9 = !{{!\"no_operations\", !\"sha256:{kir_hex}\", !\"kir-version:8\", i64 123, !\"target:{target}\"}}\n",
            ),
            kir_hex = "07".repeat(32),
            target = TARGET,
        );
        assert_eq!(
            parse_llvm_absence_v1(&no_operations, kir, TARGET).unwrap(),
            Some(LlvmAnchorAbsenceV1::NoOperations)
        );
        let multiple = no_operations.replace("no_operations", "multiple_defined_bodies");
        assert_eq!(
            parse_llvm_absence_v1(&multiple, kir, TARGET).unwrap(),
            Some(LlvmAnchorAbsenceV1::MultipleDefinedBodies)
        );

        let duplicated = format!("{no_operations}!fe2o3.semantic_anchor.absence.v1 = !{{!9}}\n");
        assert_eq!(
            parse_llvm_absence_v1(&duplicated, kir, TARGET),
            Err(ProductionSemanticAnchorErrorV1::ContradictoryLlvm)
        );
        let substituted = no_operations.replace("sha256:07", "sha256:08");
        assert_eq!(
            parse_llvm_absence_v1(&substituted, kir, TARGET),
            Err(ProductionSemanticAnchorErrorV1::BindingMismatch)
        );
        let contradictory = format!(
            "{no_operations}!llvm.pseudo_probe_desc = !{{!10}}\n!10 = !{{i64 1, i64 1, !\"kernel\"}}\n"
        );
        assert_eq!(
            parse_llvm_absence_v1(&contradictory, kir, TARGET),
            Err(ProductionSemanticAnchorErrorV1::ContradictoryLlvm)
        );
    }

    fn push_uleb(bytes: &mut Vec<u8>, mut value: u64) {
        loop {
            let mut byte = (value & 0x7f) as u8;
            value >>= 7;
            if value != 0 {
                byte |= 0x80;
            }
            bytes.push(byte);
            if value == 0 {
                break;
            }
        }
    }

    fn probes(guid: u64, records: &[(u64, u64)]) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&guid.to_le_bytes());
        push_uleb(&mut bytes, records.len() as u64);
        push_uleb(&mut bytes, 0);
        for (index, address) in records {
            push_uleb(&mut bytes, *index);
            bytes.push(0);
            bytes.extend_from_slice(&address.to_le_bytes());
        }
        bytes
    }

    #[test]
    fn probe_decoder_preserves_duplicate_and_coalesced_cardinality() {
        let decoded = decode_probe_records(
            &probes(11, &[(1, 0x100), (1, 0x108), (2, 0x108)]),
            11,
            0x100,
        )
        .unwrap();
        assert_eq!(decoded.len(), 3);
        assert_eq!(decoded[0].probe_index, 1);
        assert_eq!(decoded[1].address, 0x108);

        assert_eq!(
            classify_transformation(2, true),
            ProductionSemanticAnchorTransformationV1::DuplicatedAndCoalesced
        );
        assert_eq!(
            classify_transformation(1, true),
            ProductionSemanticAnchorTransformationV1::Coalesced
        );
        assert_eq!(
            classify_transformation(0, false),
            ProductionSemanticAnchorTransformationV1::Eliminated
        );
        assert_eq!(
            classify_transformation(2, false),
            ProductionSemanticAnchorTransformationV1::Duplicated
        );
        assert!(matches!(
            decode_probe_records(&probes(12, &[(1, 0x100)]), 11, 0x100),
            Err(ProductionSemanticAnchorErrorV1::InvalidProbeEncoding)
        ));
    }

    #[test]
    fn probe_decoder_rejects_declared_records_larger_than_remaining_payload() {
        let mut truncated = Vec::new();
        truncated.extend_from_slice(&11_u64.to_le_bytes());
        push_uleb(&mut truncated, 4);
        push_uleb(&mut truncated, 0);

        assert!(matches!(
            decode_probe_records(&truncated, 11, 0x100),
            Err(ProductionSemanticAnchorErrorV1::InvalidProbeEncoding)
        ));
    }
}
