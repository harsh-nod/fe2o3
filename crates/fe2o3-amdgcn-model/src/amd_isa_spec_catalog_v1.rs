//! Immutable queries over pinned public AMD specification metadata.
//!
//! No encoding selection, expression evaluation, register legality, executable
//! instruction admission, proof, artifact, or hardware authority is provided.
//! Description prose is omitted; exact source bytes remain identified by SHA256.

use std::fmt;

#[rustfmt::skip]
#[path = "amd_isa_spec_catalog_v1_cdna3_generated.rs"]
mod cdna3;
#[rustfmt::skip]
#[path = "amd_isa_spec_catalog_v1_cdna4_generated.rs"]
mod cdna4;
#[rustfmt::skip]
#[path = "amd_isa_spec_catalog_v1_coverage_generated.rs"]
mod coverage;

pub const AMD_ISA_SPEC_CATALOG_FORMAT_V1: &str = "fe2o3-amd-isa-spec-catalog-v1";
pub const AMD_ISA_SPEC_ARCHIVE_SHA256_V1: &str =
    "82404f1126761b7877595b622afa7e1f311f2f41e89a3abe9aaf8ad045c082e2";
pub const AMD_ISA_SPEC_REVIEWED_COVERAGE_JSON_V1: &str =
    include_str!("../../../tools/amd-isa-catalog-v1/reviewed-coverage.json");

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AmdIsaArchitectureV1 {
    Cdna3,
    Cdna4,
}

/// Reviewed metadata selection only, not a qualified silicon or feature profile.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AmdIsaCatalogTargetV1 {
    Gfx942,
    Gfx950,
}
impl AmdIsaCatalogTargetV1 {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Gfx942 => "gfx942",
            Self::Gfx950 => "gfx950",
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UnsupportedAmdIsaCatalogTargetV1;
impl fmt::Display for UnsupportedAmdIsaCatalogTargetV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("catalog accepts only exact gfx942 or gfx950 metadata profile names")
    }
}
impl std::error::Error for UnsupportedAmdIsaCatalogTargetV1 {}

/// Unknown targets, suffixes, feature strings, case folding and whitespace fail.
pub fn amd_isa_catalog_for_target_v1(
    name: &str,
) -> Result<AmdIsaSpecCatalogV1, UnsupportedAmdIsaCatalogTargetV1> {
    let (architecture, target) = match name {
        "gfx942" => (AmdIsaArchitectureV1::Cdna3, AmdIsaCatalogTargetV1::Gfx942),
        "gfx950" => (AmdIsaArchitectureV1::Cdna4, AmdIsaCatalogTargetV1::Gfx950),
        _ => return Err(UnsupportedAmdIsaCatalogTargetV1),
    };
    let mut catalog = amd_isa_spec_catalog_v1(architecture);
    catalog.target = Some(target);
    Ok(catalog)
}

/// Architecture generation inventory without a target-profile association.
pub fn amd_isa_spec_catalog_v1(architecture: AmdIsaArchitectureV1) -> AmdIsaSpecCatalogV1 {
    AmdIsaSpecCatalogV1 {
        data: match architecture {
            AmdIsaArchitectureV1::Cdna3 => &cdna3::DATA,
            AmdIsaArchitectureV1::Cdna4 => &cdna4::DATA,
        },
        target: None,
    }
}

/// Catalog identity is content-scoped and named, never an ordinal instruction ID.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AmdIsaCatalogIdentityV1 {
    pub format: &'static str,
    pub architecture: AmdIsaArchitectureV1,
    pub target_profile: Option<AmdIsaCatalogTargetV1>,
    pub archive_sha256: &'static str,
    pub member: &'static str,
    pub member_sha256: &'static str,
    /// Exact generated metadata bytes, independent of the upstream XML digest.
    pub metadata_sha256: &'static str,
    /// Exact separately reviewed overlay bytes, not an executable capability.
    pub reviewed_coverage_sha256: &'static str,
}

#[derive(Clone, Copy)]
pub struct AmdIsaSpecCatalogV1 {
    data: &'static CatalogData,
    target: Option<AmdIsaCatalogTargetV1>,
}
impl AmdIsaSpecCatalogV1 {
    pub fn identity(self) -> AmdIsaCatalogIdentityV1 {
        AmdIsaCatalogIdentityV1 {
            format: AMD_ISA_SPEC_CATALOG_FORMAT_V1,
            architecture: self.data.architecture,
            target_profile: self.target,
            archive_sha256: AMD_ISA_SPEC_ARCHIVE_SHA256_V1,
            member: self.data.member,
            member_sha256: self.data.member_sha256,
            metadata_sha256: self.data.metadata_sha256,
            reviewed_coverage_sha256: coverage::SHA256,
        }
    }
    pub fn instruction_count(self) -> usize {
        self.data.instructions.iter().map(|part| part.len()).sum()
    }
    pub fn instructions(self) -> impl Iterator<Item = AmdIsaInstructionV1> {
        self.data
            .instructions
            .iter()
            .flat_map(|part| part.iter())
            .map(move |data| AmdIsaInstructionV1 {
                catalog: self,
                data,
                queried_name: data.0,
            })
    }
    /// Exact canonical name only, with bounded comparisons and no allocations.
    pub fn instruction(self, name: &str) -> Option<AmdIsaInstructionV1> {
        if name.len() > 128 {
            return None;
        }
        for part in self.data.instructions {
            if let Ok(index) = part.binary_search_by_key(&name, |row| row.0) {
                return Some(AmdIsaInstructionV1 {
                    catalog: self,
                    data: &part[index],
                    queried_name: part[index].0,
                });
            }
        }
        None
    }
    /// Exact canonical name or an explicit upstream alias; no inferred spelling.
    pub fn lookup(self, name: &str) -> Option<AmdIsaInstructionV1> {
        if let Some(found) = self.instruction(name) {
            return Some(found);
        }
        if name.len() > 128 {
            return None;
        }
        let index = self
            .data
            .aliases
            .binary_search_by_key(&name, |row| row.0)
            .ok()?;
        let (alias, canonical) = self.data.aliases[index];
        let mut result = self.instruction(canonical)?;
        result.queried_name = alias;
        Some(result)
    }
    pub fn entries(
        self,
        kind: AmdIsaMetadataKindV1,
    ) -> impl ExactSizeIterator<Item = AmdIsaNamedMetadataV1> {
        self.named(kind)
            .iter()
            .map(move |data| AmdIsaNamedMetadataV1 {
                catalog: self,
                kind,
                data,
            })
    }
    pub fn entry(self, kind: AmdIsaMetadataKindV1, name: &str) -> Option<AmdIsaNamedMetadataV1> {
        if name.len() > 128 {
            return None;
        }
        let rows = self.named(kind);
        let index = rows.binary_search_by_key(&name, |row| row.0).ok()?;
        Some(AmdIsaNamedMetadataV1 {
            catalog: self,
            kind,
            data: &rows[index],
        })
    }
    fn named(self, kind: AmdIsaMetadataKindV1) -> &'static [NamedData] {
        match kind {
            AmdIsaMetadataKindV1::Encoding => self.data.encodings,
            AmdIsaMetadataKindV1::OperandType => self.data.operand_types,
            AmdIsaMetadataKindV1::DataFormat => self.data.data_formats,
        }
    }
    pub const fn metadata_json(self) -> &'static str {
        self.data.metadata
    }
    pub const fn metadata_sha256(self) -> &'static str {
        self.data.metadata_sha256
    }
    pub const fn grants_authority(self) -> bool {
        false
    }
    fn tree(self, range: (usize, usize)) -> AmdIsaMetadataTreeV1 {
        AmdIsaMetadataTreeV1 {
            catalog: self,
            json: &self.data.metadata[range.0..range.1],
        }
    }
}

/// Dictionary-coded inert XML tree. Conditions are data, never evaluated.
/// Each node is [tag string ID, flat attribute string IDs, text string ID, children].
/// The dictionary and source identity must accompany any detached JSON fragment.
#[derive(Clone, Copy)]
pub struct AmdIsaMetadataTreeV1 {
    catalog: AmdIsaSpecCatalogV1,
    json: &'static str,
}
impl AmdIsaMetadataTreeV1 {
    pub fn identity(self) -> AmdIsaCatalogIdentityV1 {
        self.catalog.identity()
    }
    pub const fn dictionary(self) -> &'static [&'static str] {
        self.catalog.data.strings
    }
    pub const fn tree_json(self) -> &'static str {
        self.json
    }
}

/// A named instruction family; its alternatives do not authorize an encoding.
#[derive(Clone, Copy)]
pub struct AmdIsaInstructionV1 {
    catalog: AmdIsaSpecCatalogV1,
    data: &'static InstructionData,
    queried_name: &'static str,
}
impl AmdIsaInstructionV1 {
    pub fn catalog_identity(self) -> AmdIsaCatalogIdentityV1 {
        self.catalog.identity()
    }
    pub const fn name(self) -> &'static str {
        self.data.0
    }
    pub const fn queried_name(self) -> &'static str {
        self.queried_name
    }
    pub const fn aliases(self) -> &'static [&'static str] {
        self.data.1
    }
    pub const fn flags(self) -> AmdIsaInstructionFlagsV1 {
        AmdIsaInstructionFlagsV1(self.data.2)
    }
    pub fn encodings(self) -> impl ExactSizeIterator<Item = AmdIsaInstructionEncodingV1> {
        self.data
            .3
            .iter()
            .map(move |data| AmdIsaInstructionEncodingV1 {
                instruction: self,
                data,
            })
    }
    pub fn metadata(self) -> AmdIsaMetadataTreeV1 {
        self.catalog.tree(self.data.4)
    }
    /// Matching the separately reviewed source-marker overlay is not executable
    /// catalog support. It does not establish encoding-level stages or EXEC.
    pub fn reviewed_source_marker_family(self) -> bool {
        self.catalog.target == Some(AmdIsaCatalogTargetV1::Gfx942)
            && coverage::NAMES.binary_search(&self.name()).is_ok()
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AmdIsaInstructionFlagsV1(u8);
impl AmdIsaInstructionFlagsV1 {
    pub const fn is_branch(self) -> bool {
        self.0 & 1 != 0
    }
    pub const fn is_conditional_branch(self) -> bool {
        self.0 & 2 != 0
    }
    pub const fn is_indirect_branch(self) -> bool {
        self.0 & 4 != 0
    }
    pub const fn is_program_terminator(self) -> bool {
        self.0 & 8 != 0
    }
    pub const fn is_immediately_executed(self) -> bool {
        self.0 & 16 != 0
    }
}

/// The source instruction, target profile, encoding name and condition stay joined.
#[derive(Clone, Copy)]
pub struct AmdIsaInstructionEncodingV1 {
    instruction: AmdIsaInstructionV1,
    data: &'static EncodingData,
}
impl AmdIsaInstructionEncodingV1 {
    pub const fn instruction(self) -> AmdIsaInstructionV1 {
        self.instruction
    }
    pub const fn name(self) -> &'static str {
        self.data.0
    }
    pub const fn condition(self) -> &'static str {
        self.data.1
    }
    pub const fn opcode(self) -> u32 {
        self.data.2
    }
    /// Zero means an explicitly retained upstream missing declaration; three
    /// means the reviewed identical ENC_FLAT/default triple, not unique selection.
    pub const fn condition_declaration_count(self) -> u8 {
        self.data.3
    }
    pub const fn condition_definition_available(self) -> bool {
        self.data.3 != 0
    }
    pub fn operands(self) -> impl ExactSizeIterator<Item = AmdIsaOperandV1> {
        self.data.4.iter().map(move |data| AmdIsaOperandV1 {
            encoding: self,
            data,
        })
    }
    pub fn definition(self) -> Option<AmdIsaNamedMetadataV1> {
        self.instruction
            .catalog
            .entry(AmdIsaMetadataKindV1::Encoding, self.name())
    }
}

#[derive(Clone, Copy)]
pub struct AmdIsaOperandV1 {
    encoding: AmdIsaInstructionEncodingV1,
    data: &'static OperandData,
}
impl AmdIsaOperandV1 {
    pub const fn encoding(self) -> AmdIsaInstructionEncodingV1 {
        self.encoding
    }
    pub const fn order(self) -> u8 {
        self.data.0
    }
    pub const fn is_input(self) -> bool {
        self.data.1 & 1 != 0
    }
    pub const fn is_output(self) -> bool {
        self.data.1 & 2 != 0
    }
    pub const fn is_implicit(self) -> bool {
        self.data.1 & 4 != 0
    }
    pub const fn binary_microcode_required(self) -> bool {
        self.data.1 & 8 != 0
    }
    pub const fn field_name(self) -> Option<&'static str> {
        self.data.2
    }
    pub const fn operand_type_name(self) -> &'static str {
        self.data.3
    }
    pub const fn data_format_name(self) -> &'static str {
        self.data.4
    }
    pub const fn bit_count(self) -> u16 {
        self.data.5
    }
    pub fn operand_type(self) -> Option<AmdIsaNamedMetadataV1> {
        self.encoding
            .instruction
            .catalog
            .entry(AmdIsaMetadataKindV1::OperandType, self.data.3)
    }
    pub fn data_format(self) -> Option<AmdIsaNamedMetadataV1> {
        self.encoding
            .instruction
            .catalog
            .entry(AmdIsaMetadataKindV1::DataFormat, self.data.4)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AmdIsaMetadataKindV1 {
    Encoding,
    OperandType,
    DataFormat,
}
#[derive(Clone, Copy)]
pub struct AmdIsaNamedMetadataV1 {
    catalog: AmdIsaSpecCatalogV1,
    kind: AmdIsaMetadataKindV1,
    data: &'static NamedData,
}
impl AmdIsaNamedMetadataV1 {
    pub fn catalog_identity(self) -> AmdIsaCatalogIdentityV1 {
        self.catalog.identity()
    }
    pub const fn kind(self) -> AmdIsaMetadataKindV1 {
        self.kind
    }
    pub const fn name(self) -> &'static str {
        self.data.0
    }
    /// Width in the source specification, not a legal allocation/register range.
    pub const fn bit_count(self) -> Option<u16> {
        self.data.1
    }
    pub fn metadata(self) -> AmdIsaMetadataTreeV1 {
        self.catalog.tree(self.data.2)
    }
}

// Generated storage is private. Public references cannot be constructed or
// retargeted from user-supplied names, IDs, metadata, or ordinal positions.
struct CatalogData {
    architecture: AmdIsaArchitectureV1,
    member: &'static str,
    member_sha256: &'static str,
    metadata_sha256: &'static str,
    metadata: &'static str,
    strings: &'static [&'static str],
    instructions: &'static [&'static [InstructionData]],
    aliases: &'static [(&'static str, &'static str)],
    encodings: &'static [NamedData],
    operand_types: &'static [NamedData],
    data_formats: &'static [NamedData],
}
struct InstructionData(
    &'static str,
    &'static [&'static str],
    u8,
    &'static [EncodingData],
    (usize, usize),
);
struct EncodingData(&'static str, &'static str, u32, u8, &'static [OperandData]);
struct OperandData(
    u8,
    u8,
    Option<&'static str>,
    &'static str,
    &'static str,
    u16,
);
struct NamedData(&'static str, Option<u16>, (usize, usize));

#[cfg(test)]
#[path = "amd_isa_spec_catalog_v1_tests.rs"]
mod tests;
