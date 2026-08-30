//! Exact production KIR-to-LLVM-to-final-HSACO semantic anchors.
//!
//! This additive contract describes compiler-inserted LLVM pseudo-probe anchors. An anchor is a
//! correspondence point, not complete machine-instruction coverage, a schedule, or a refinement
//! proof.

use std::collections::{BTreeMap, BTreeSet};
use std::{error::Error, fmt};

use dialect_amdgcn::CanonicalProductionKirToLlvmReplayEvidenceV1;
use fe2o3_compiler_lineage::InertSemanticToLlvmAssociationV3;
use fe2o3_kernel_ir::{
    Module, ProductionSemanticDebugAvailabilityV1, ProductionSemanticDebugProducerGapV1,
    ProductionSemanticDebugReceiptExtensionV1, SemanticDebugLocationV1,
};
use object::{Object, ObjectSection, ObjectSymbol, SymbolKind};
use sha2::{Digest, Sha256};

use crate::{ContentIdentityV1, PreparedFinalizedProtectedWorkerV3HsacoV1};

const GUID_DOMAIN_V1: &[u8] = b"FE2O3/PRODUCTION-KIR-LLVM-ISA-ANCHOR-GUID/V1\0";
const HASH_DOMAIN_V1: &[u8] = b"FE2O3/PRODUCTION-KIR-LLVM-ISA-ANCHOR-HASH/V1\0";
const IDENTITY_DOMAIN_V1: &[u8] = b"FE2O3/PRODUCTION-KIR-LLVM-ISA-ANCHOR-ID/V1\0";
const MAX_ANCHORS_V1: usize = 1 << 20;
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
    Producer(ProductionSemanticDebugProducerGapV1),
    LegacyBareAssociationNoAttachment,
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
    llvm: SemanticDebugLocationV1,
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

    pub const fn llvm(&self) -> SemanticDebugLocationV1 {
        self.llvm
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
}

#[derive(Debug)]
pub struct AdmittedProductionSemanticAnchorsV1 {
    artifact_identity: ContentIdentityV1,
    target_bound_kir_sha256: [u8; 32],
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
}

impl PreparedFinalizedProtectedWorkerV3HsacoV1 {
    /// Replays exact target-bound KIR custody, parses compiler anchors from exact final LLVM, and
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
                        ProductionSemanticAnchorUnavailableV1::LegacyBareAssociationNoAttachment,
                    ));
                }
                Err(_) => return Err(ProductionSemanticAnchorErrorV1::InvalidCompilerAttachment),
            };
        let producer_gap = match extension.carrier_v1().availability() {
            ProductionSemanticDebugAvailabilityV1::Available(_) => None,
            ProductionSemanticDebugAvailabilityV1::Unavailable(gap) => Some(*gap),
        };

        let replay = CanonicalProductionKirToLlvmReplayEvidenceV1::decode(
            receipts.amdgpu_lowering().canonical_preimage(),
        )
        .and_then(|evidence| {
            evidence.validate_against_neutral_kernel_ir(receipts.kernel_ir().canonical_preimage())
        })
        .map_err(|_| ProductionSemanticAnchorErrorV1::InvalidKirToLlvmReplay)?;
        let expected_target = replay.evidence().profile().device_target();
        if expected_target != self.target().to_string()
            || expected_target != outer.module_handoff().target().to_string()
        {
            return Err(ProductionSemanticAnchorErrorV1::TargetMismatch);
        }
        let expected_kir = replay.evidence().target_bound_kernel_ir_identity().sha256();
        let llvm = std::str::from_utf8(outer.module_handoff().module_bytes())
            .map_err(|_| ProductionSemanticAnchorErrorV1::InvalidLlvm)?;
        let replay_llvm = replay.evidence().pre_descriptor_llvm();
        let replay_manifest = parse_llvm_manifest_v1(replay_llvm, expected_kir, &expected_target)?;
        let Some(manifest) = parse_llvm_manifest_v1(llvm, expected_kir, &expected_target)? else {
            let unavailable = producer_gap.map_or(
                ProductionSemanticAnchorUnavailableV1::CompilerInstrumentationAbsent,
                ProductionSemanticAnchorUnavailableV1::Producer,
            );
            return Ok(ProductionSemanticAnchorAdmissionV1::Unavailable(
                unavailable,
            ));
        };
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
    target_bound_kir_sha256: [u8; 32],
    target: String,
    guid: u64,
    function_hash: u64,
    descriptor_name: String,
    records: Vec<LlvmAnchorRecordV1>,
}

fn parse_llvm_manifest_v1(
    llvm: &str,
    expected_kir: [u8; 32],
    expected_target: &str,
) -> Result<Option<LlvmAnchorManifestV1>, ProductionSemanticAnchorErrorV1> {
    let named = llvm
        .lines()
        .filter(|line| line.starts_with("!fe2o3.semantic_anchor.v1 = "))
        .collect::<Vec<_>>();
    if named.is_empty() {
        if llvm.contains("@llvm.pseudoprobe") || llvm.contains("!llvm.pseudo_probe_desc") {
            return Err(ProductionSemanticAnchorErrorV1::ContradictoryLlvm);
        }
        return Ok(None);
    }
    if named.len() != 1 {
        return Err(ProductionSemanticAnchorErrorV1::ContradictoryLlvm);
    }

    let definitions = metadata_definitions(llvm)?;
    let references = parse_metadata_references(
        named[0]
            .strip_prefix("!fe2o3.semantic_anchor.v1 = ")
            .ok_or(ProductionSemanticAnchorErrorV1::InvalidLlvm)?,
    )?;
    if references.len() < 2 {
        return Err(ProductionSemanticAnchorErrorV1::InvalidLlvm);
    }
    let referenced = references.iter().copied().collect::<BTreeSet<_>>();
    if referenced.len() != references.len() {
        return Err(ProductionSemanticAnchorErrorV1::ContradictoryLlvm);
    }
    let binding_reference = references[0];
    let binding = definitions
        .get(&binding_reference)
        .ok_or(ProductionSemanticAnchorErrorV1::InvalidLlvm)?;
    let fields = metadata_fields(binding)?;
    if fields.len() != 6 {
        return Err(ProductionSemanticAnchorErrorV1::InvalidLlvm);
    }
    let kir = parse_prefixed_sha256(fields[0], "sha256:")?;
    let target = parse_metadata_string(fields[1], "target:")?;
    let guid = parse_i64_field(fields[2])?;
    let function_hash = parse_i64_field(fields[3])?;
    let block_count = parse_i64_field(fields[4])?;
    let operation_count = parse_i64_field(fields[5])?;
    if kir != expected_kir
        || target != expected_target
        || guid == 0
        || function_hash == 0
        || operation_count == 0
        || usize::try_from(operation_count).ok() != Some(references.len() - 1)
        || operation_count as usize > MAX_ANCHORS_V1
    {
        return Err(ProductionSemanticAnchorErrorV1::BindingMismatch);
    }
    let descriptor_line = llvm
        .lines()
        .filter(|line| line.starts_with("!llvm.pseudo_probe_desc = "))
        .collect::<Vec<_>>();
    if descriptor_line.len() != 1 {
        return Err(ProductionSemanticAnchorErrorV1::ContradictoryLlvm);
    }
    let descriptor_references = parse_metadata_references(
        descriptor_line[0]
            .strip_prefix("!llvm.pseudo_probe_desc = ")
            .ok_or(ProductionSemanticAnchorErrorV1::InvalidLlvm)?,
    )?;
    if descriptor_references.len() != 1 || referenced.contains(&descriptor_references[0]) {
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
    let descriptor_name = parse_metadata_string(descriptor[2], "")?;

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
    let function_ordinal = records[0].function_ordinal;
    let expected_guid = anchor_digest(
        GUID_DOMAIN_V1,
        kir,
        &target,
        function_ordinal,
        block_count,
        operation_count,
    );
    let expected_hash = anchor_digest(
        HASH_DOMAIN_V1,
        kir,
        &target,
        function_ordinal,
        block_count,
        operation_count,
    );
    if guid != expected_guid || function_hash != expected_hash {
        return Err(ProductionSemanticAnchorErrorV1::BindingMismatch);
    }
    Ok(Some(LlvmAnchorManifestV1 {
        target_bound_kir_sha256: kir,
        target,
        guid,
        function_hash,
        descriptor_name,
        records,
    }))
}

fn metadata_definitions(
    llvm: &str,
) -> Result<BTreeMap<usize, &str>, ProductionSemanticAnchorErrorV1> {
    let mut definitions = BTreeMap::new();
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
        if definitions.insert(number, value).is_some() {
            return Err(ProductionSemanticAnchorErrorV1::ContradictoryLlvm);
        }
    }
    Ok(definitions)
}

fn parse_metadata_references(value: &str) -> Result<Vec<usize>, ProductionSemanticAnchorErrorV1> {
    let inner = value
        .strip_prefix("!{")
        .and_then(|value| value.strip_suffix('}'))
        .ok_or(ProductionSemanticAnchorErrorV1::InvalidLlvm)?;
    let mut result = Vec::new();
    for field in inner.split(", ") {
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
    Ok(inner.split(", ").collect())
}

fn parse_i64_field(value: &str) -> Result<u64, ProductionSemanticAnchorErrorV1> {
    value
        .strip_prefix("i64 ")
        .ok_or(ProductionSemanticAnchorErrorV1::InvalidLlvm)?
        .parse()
        .map_err(|_| ProductionSemanticAnchorErrorV1::InvalidLlvm)
}

fn parse_metadata_string<'a>(
    value: &'a str,
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
    Ok(value.to_owned())
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
    let mut seen = BTreeSet::new();
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
            let fields = arguments.split(", ").collect::<Vec<_>>();
            if fields.len() != 4
                || parse_i64_field(fields[0])? != guid
                || fields[2] != "i32 0"
                || fields[3] != "i64 -1"
            {
                return Err(ProductionSemanticAnchorErrorV1::ContradictoryLlvm);
            }
            let probe_index = parse_i64_field(fields[1])?;
            let record = records
                .iter_mut()
                .find(|record| record.probe_index == probe_index)
                .ok_or(ProductionSemanticAnchorErrorV1::ContradictoryLlvm)?;
            if !seen.insert(probe_index) {
                return Err(ProductionSemanticAnchorErrorV1::ContradictoryLlvm);
            }
            record.llvm_function_ordinal = function;
            record.llvm_block_ordinal = block;
            record.llvm_instruction_ordinal = instruction;
        }
        instruction += 1;
    }
    if seen.len() != records.len() {
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
        || descriptors[0].name != manifest.descriptor_name
    {
        return Err(ProductionSemanticAnchorErrorV1::ProbeDescriptorMismatch);
    }

    let inspected = crate::inspect_and_bind_kernel_descriptors(artifact)
        .map_err(|_| ProductionSemanticAnchorErrorV1::InvalidArtifact)?;
    let symbol = unique_entry_symbol(&object, &manifest.descriptor_name, &inspected)?;
    let decoded = decode_probe_records(probes, manifest.guid, symbol.entry_address)?;
    let mut addresses = BTreeMap::<u64, Vec<u64>>::new();
    for record in decoded {
        if !manifest
            .records
            .iter()
            .any(|expected| expected.probe_index == record.probe_index)
        {
            return Err(ProductionSemanticAnchorErrorV1::UnexpectedProbe);
        }
        addresses
            .entry(record.probe_index)
            .or_default()
            .push(record.address);
    }
    for values in addresses.values_mut() {
        values.sort_unstable();
        values.dedup();
    }
    let mut address_owners = BTreeMap::<u64, usize>::new();
    for values in addresses.values() {
        for address in values {
            *address_owners.entry(*address).or_default() += 1;
        }
    }

    let entry_end = symbol
        .entry_address
        .checked_add(symbol.entry_size)
        .ok_or(ProductionSemanticAnchorErrorV1::InvalidArtifact)?;
    let mut admitted = Vec::new();
    admitted
        .try_reserve_exact(manifest.records.len())
        .map_err(|_| ProductionSemanticAnchorErrorV1::AllocationFailure)?;
    for record in manifest.records {
        let values = addresses.remove(&record.probe_index).unwrap_or_default();
        let transformation = classify_transformation(&values, &address_owners);
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
            manifest.target_bound_kir_sha256,
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
            llvm: SemanticDebugLocationV1::Llvm {
                function_ordinal: record.llvm_function_ordinal,
                block_ordinal: record.llvm_block_ordinal,
                instruction_ordinal: record.llvm_instruction_ordinal,
            },
            isa,
            transformation,
        });
    }
    if !addresses.is_empty() {
        return Err(ProductionSemanticAnchorErrorV1::UnexpectedProbe);
    }
    Ok(AdmittedProductionSemanticAnchorsV1 {
        artifact_identity: ContentIdentityV1::calculate(artifact),
        target_bound_kir_sha256: manifest.target_bound_kir_sha256,
        target: manifest.target,
        pseudo_probe_desc_bytes: desc.len() as u64,
        pseudo_probe_bytes: probes.len() as u64,
        anchors: admitted,
    })
}

fn classify_transformation(
    addresses: &[u64],
    address_owners: &BTreeMap<u64, usize>,
) -> ProductionSemanticAnchorTransformationV1 {
    let coalesced = addresses
        .iter()
        .any(|address| address_owners.get(address).copied().unwrap_or(0) > 1);
    match (addresses.len(), coalesced) {
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
    name: String,
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
        result.push(ProbeDescriptorV1 {
            guid,
            function_hash,
            name: name.to_owned(),
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
    while !cursor.is_empty() {
        let guid = cursor.u64()?;
        let count = cursor.uleb()?;
        let inline_count = cursor.uleb()?;
        let count = usize::try_from(count)
            .map_err(|_| ProductionSemanticAnchorErrorV1::InvalidProbeEncoding)?;
        if guid != expected_guid || inline_count != 0 || count > MAX_ANCHORS_V1 + 1 {
            return Err(ProductionSemanticAnchorErrorV1::InvalidProbeEncoding);
        }
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

fn unique_entry_symbol(
    object: &object::File<'_>,
    expected_name: &str,
    inspected: &fe2o3_hsaco::InspectedKernelBindings,
) -> Result<EntrySymbolV1, ProductionSemanticAnchorErrorV1> {
    let candidates = object
        .symbols()
        .filter(|symbol| {
            symbol.kind() == SymbolKind::Text
                && symbol.name() == Ok(expected_name)
                && symbol.size() != 0
                && !symbol.is_undefined()
        })
        .collect::<Vec<_>>();
    if candidates.len() != 1 {
        return Err(ProductionSemanticAnchorErrorV1::AmbiguousEntrySymbol);
    }
    let candidate = &candidates[0];
    let matches = inspected
        .bindings()
        .iter()
        .filter(|binding| {
            binding.entry_address() == candidate.address()
                && binding.entry_size() == candidate.size()
        })
        .collect::<Vec<_>>();
    if matches.len() != 1 {
        return Err(ProductionSemanticAnchorErrorV1::AmbiguousEntrySymbol);
    }
    let aliases = object
        .symbols()
        .filter(|symbol| {
            symbol.kind() == SymbolKind::Text
                && !symbol.is_undefined()
                && symbol.address() == candidate.address()
        })
        .count();
    if aliases != 1 {
        return Err(ProductionSemanticAnchorErrorV1::AmbiguousEntrySymbol);
    }
    let binding = matches[0];
    let kernel = inspected
        .inspection()
        .kernels()
        .get(binding.kernel_index())
        .ok_or(ProductionSemanticAnchorErrorV1::AmbiguousEntrySymbol)?;
    if kernel.name() != expected_name {
        return Err(ProductionSemanticAnchorErrorV1::AmbiguousEntrySymbol);
    }
    Ok(EntrySymbolV1 {
        kernel_ordinal: binding.kernel_index(),
        entry_address: binding.entry_address(),
        entry_size: binding.entry_size(),
        entry_file_offset: binding.entry_file_offset(),
    })
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

fn anchor_digest(
    domain: &[u8],
    kir: [u8; 32],
    target: &str,
    function: u64,
    blocks: u64,
    operations: u64,
) -> u64 {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update(kir);
    hasher.update((target.len() as u64).to_le_bytes());
    hasher.update(target.as_bytes());
    hasher.update(function.to_le_bytes());
    hasher.update(blocks.to_le_bytes());
    hasher.update(operations.to_le_bytes());
    let digest = hasher.finalize();
    let mut prefix = [0_u8; 8];
    prefix.copy_from_slice(&digest[..8]);
    let value = u64::from_le_bytes(prefix);
    if value == 0 { 1 } else { value }
}

fn semantic_operation_identity(
    kir: [u8; 32],
    target: &str,
    function: u64,
    block: u64,
    operation: u64,
) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(IDENTITY_DOMAIN_V1);
    hasher.update(kir);
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

    const TARGET: &str = "gfx942:xnack-";

    fn llvm_fixture(ids: [usize; 4], swap_markers: bool) -> (String, [u8; 32]) {
        let kir = [7_u8; 32];
        let guid = anchor_digest(GUID_DOMAIN_V1, kir, TARGET, 0, 2, 2);
        let hash = anchor_digest(HASH_DOMAIN_V1, kir, TARGET, 0, 2, 2);
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
                    "!{header} = !{{!\"sha256:{kir_hex}\", !\"target:{target}\", i64 {guid}, i64 {hash}, i64 2, i64 2}}\n",
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
            kir,
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

        let owners = BTreeMap::from([(0x100, 1), (0x108, 2)]);
        assert_eq!(
            classify_transformation(&[0x100, 0x108], &owners),
            ProductionSemanticAnchorTransformationV1::DuplicatedAndCoalesced
        );
        assert_eq!(
            classify_transformation(&[0x108], &owners),
            ProductionSemanticAnchorTransformationV1::Coalesced
        );
        assert_eq!(
            classify_transformation(&[], &owners),
            ProductionSemanticAnchorTransformationV1::Eliminated
        );
        assert!(matches!(
            decode_probe_records(&probes(12, &[(1, 0x100)]), 11, 0x100),
            Err(ProductionSemanticAnchorErrorV1::InvalidProbeEncoding)
        ));
    }
}
