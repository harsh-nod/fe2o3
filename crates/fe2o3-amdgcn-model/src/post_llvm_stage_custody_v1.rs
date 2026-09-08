//! Target-specific envelope checks for exact gfx942/gfx950 post-LLVM stage custody.

use core::fmt;
use fe2o3_compiler_lineage::CheckedPostLlvmStageContentsV1;

const ELF64_HEADER_BYTES: usize = 64;
const ELF64_SECTION_HEADER_BYTES: usize = 64;
const MAX_ELF_SECTIONS: usize = 256;
const MAX_ELF_SYMBOLS: usize = 4_096;
const MAX_ELF_RELOCATIONS: usize = 16_384;
const MAX_ELF_NAME_BYTES: usize = 256;
const ELF_MAGIC: &[u8; 4] = b"\x7fELF";
const ELFCLASS64: u8 = 2;
const ELFDATA2LSB: u8 = 1;
const EV_CURRENT: u8 = 1;
const ELFOSABI_NONE: u8 = 0;
const ELFOSABI_AMDGPU_HSA: u8 = 64;
const ET_REL: u16 = 1;
const ET_DYN: u16 = 3;
const EM_AMDGPU: u16 = 224;
const SHT_NOTE: u32 = 7;
const SHT_PROGBITS: u32 = 1;
const SHT_SYMTAB: u32 = 2;
const SHT_STRTAB: u32 = 3;
const SHT_RELA: u32 = 4;
const SHT_NOBITS: u32 = 8;
const SHT_REL: u32 = 9;
const SHT_DYNSYM: u32 = 11;
const SHF_WRITE: u64 = 1;
const SHF_ALLOC: u64 = 2;
const SHF_EXECINSTR: u64 = 4;
const NT_AMDGPU_METADATA: u32 = 32;
const COV6_ABI_VERSION: u8 = 4;
const GFX942_XNACK_MINUS_ELF_FLAGS: u32 = 0x64c;
const GFX950_XNACK_MINUS_SRAM_ECC_MINUS_ELF_FLAGS: u32 = 0x0a4f;
const AMDHSA_KERNEL_DESCRIPTOR_BYTES: u64 = 64;
const AMDHSA_COMPUTE_PGM_RSRC1_OFFSET: usize = 48;
const RAW_LLVM_BITCODE_MAGIC: &[u8; 4] = b"BC\xc0\xde";
const LLVM_BITCODE_WRAPPER_MAGIC: &[u8; 4] = b"\xde\xc0\x17\x0b";
const LLVM_BITCODE_WRAPPER_HEADER_BYTES: usize = 20;

/// Move-only exact stage bodies after target-specific binary-envelope checks.
#[must_use = "dropping checked gfx942 stage custody abandons machine-refinement inputs"]
pub struct CheckedGfx942PostLlvmStageEnvelopesV1 {
    contents: CheckedPostLlvmStageContentsV1,
}

/// Move-only exact gfx950 stage bodies after target-specific binary-envelope checks.
#[must_use = "dropping checked gfx950 stage custody abandons machine-refinement inputs"]
pub struct CheckedGfx950PostLlvmStageEnvelopesV1 {
    contents: CheckedPostLlvmStageContentsV1,
}

/// One named allocatable object section retained by the preservation premise.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Gfx942ObjectSectionCustodyV1 {
    name: String,
    section_type: u32,
    flags: u64,
    byte_len: u64,
    alignment: u64,
    has_relocations: bool,
}

impl Gfx942ObjectSectionCustodyV1 {
    /// Returns the exact ELF section name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the ELF section type.
    pub const fn section_type(&self) -> u32 {
        self.section_type
    }

    /// Returns the ELF section flags.
    pub const fn flags(&self) -> u64 {
        self.flags
    }

    /// Returns the object section byte length.
    pub const fn byte_len(&self) -> u64 {
        self.byte_len
    }

    /// Returns the required section alignment.
    pub const fn alignment(&self) -> u64 {
        self.alignment
    }

    /// Reports whether the input section is targeted by a retained relocation.
    pub const fn has_relocations(&self) -> bool {
        self.has_relocations
    }
}

/// One defined global or weak object symbol retained by the preservation premise.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Gfx942ObjectSymbolCustodyV1 {
    name: String,
    binding: u8,
    symbol_type: u8,
    visibility: u8,
    section_name: String,
    value: u64,
    byte_len: u64,
}

impl Gfx942ObjectSymbolCustodyV1 {
    /// Returns the exact symbol name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the ELF symbol binding.
    pub const fn binding(&self) -> u8 {
        self.binding
    }

    /// Returns the ELF symbol type.
    pub const fn symbol_type(&self) -> u8 {
        self.symbol_type
    }

    /// Returns the ELF symbol visibility.
    pub const fn visibility(&self) -> u8 {
        self.visibility
    }

    /// Returns the defining section name.
    pub fn section_name(&self) -> &str {
        &self.section_name
    }

    /// Returns the ELF symbol value in this artifact's address model.
    pub const fn value(&self) -> u64 {
        self.value
    }

    /// Returns the declared symbol byte length.
    pub const fn byte_len(&self) -> u64 {
        self.byte_len
    }
}

/// One bounded input relocation retained for exact linker replay.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Gfx942ObjectRelocationCustodyV1 {
    target_section_name: String,
    symbol_name: String,
    offset: u64,
    relocation_type: u32,
    addend: Option<i64>,
}

impl Gfx942ObjectRelocationCustodyV1 {
    /// Returns the section targeted by the relocation.
    pub fn target_section_name(&self) -> &str {
        &self.target_section_name
    }

    /// Returns the referenced symbol name, which may be empty for symbol zero.
    pub fn symbol_name(&self) -> &str {
        &self.symbol_name
    }

    /// Returns the section-relative relocation offset.
    pub const fn offset(&self) -> u64 {
        self.offset
    }

    /// Returns the AMDGPU ELF relocation type number.
    pub const fn relocation_type(&self) -> u32 {
        self.relocation_type
    }

    /// Returns the explicit RELA addend, or `None` for REL.
    pub const fn addend(&self) -> Option<i64> {
        self.addend
    }
}

/// Move-only object-to-HSACO structural custody plus exact linker replay.
#[must_use = "dropping object-to-HSACO custody abandons a machine-refinement premise"]
pub struct CheckedGfx942ObjectToHsacoPreservationV1 {
    envelopes: CheckedGfx942PostLlvmStageEnvelopesV1,
    sections: Box<[Gfx942ObjectSectionCustodyV1]>,
    symbols: Box<[Gfx942ObjectSymbolCustodyV1]>,
    relocations: Box<[Gfx942ObjectRelocationCustodyV1]>,
}

/// Exact gfx950 object-to-HSACO structural custody plus exact linker replay.
///
/// This is a distinct target owner, not an alias of the gfx942 proof input. The shared custody
/// row types describe target-independent ELF facts; construction is guarded by gfx950 ELF flags.
#[must_use = "dropping gfx950 object-to-HSACO custody abandons a machine-refinement premise"]
pub struct CheckedGfx950ObjectToHsacoPreservationV1 {
    envelopes: CheckedGfx950PostLlvmStageEnvelopesV1,
    sections: Box<[Gfx942ObjectSectionCustodyV1]>,
    symbols: Box<[Gfx942ObjectSymbolCustodyV1]>,
    relocations: Box<[Gfx942ObjectRelocationCustodyV1]>,
}

impl fmt::Debug for CheckedGfx942ObjectToHsacoPreservationV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CheckedGfx942ObjectToHsacoPreservationV1")
            .field("sections", &self.sections.len())
            .field("symbols", &self.symbols.len())
            .field("relocations", &self.relocations.len())
            .finish_non_exhaustive()
    }
}

impl CheckedGfx942ObjectToHsacoPreservationV1 {
    /// Returns the exact post-LLVM stage bodies underlying this preservation premise.
    pub const fn contents(&self) -> &CheckedPostLlvmStageContentsV1 {
        self.envelopes.contents()
    }

    /// Returns every retained allocatable input section.
    pub fn sections(&self) -> &[Gfx942ObjectSectionCustodyV1] {
        &self.sections
    }

    /// Returns every retained defined global or weak input symbol.
    pub fn symbols(&self) -> &[Gfx942ObjectSymbolCustodyV1] {
        &self.symbols
    }

    /// Returns the complete bounded input relocation roster.
    pub fn relocations(&self) -> &[Gfx942ObjectRelocationCustodyV1] {
        &self.relocations
    }

    /// Reports exact replay-output custody and checked section/symbol preservation premises.
    pub const fn retains_object_to_hsaco_structural_and_replay_premises(&self) -> bool {
        true
    }

    /// Reports independent evaluation of every admitted input relocation against final bytes.
    pub const fn proves_relocation_semantics(&self) -> bool {
        true
    }

    /// Structural matching and exact replay do not prove linker semantic refinement.
    pub const fn proves_linker_semantic_refinement(&self) -> bool {
        false
    }

    /// This owner grants no compiler, publication, load, or launch authority.
    pub const fn grants_authority(&self) -> bool {
        false
    }

    /// Returns the checked envelope owner.
    pub fn into_envelopes(self) -> CheckedGfx942PostLlvmStageEnvelopesV1 {
        self.envelopes
    }
}

impl fmt::Debug for CheckedGfx950ObjectToHsacoPreservationV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CheckedGfx950ObjectToHsacoPreservationV1")
            .field("sections", &self.sections.len())
            .field("symbols", &self.symbols.len())
            .field("relocations", &self.relocations.len())
            .finish_non_exhaustive()
    }
}

impl CheckedGfx950ObjectToHsacoPreservationV1 {
    /// Returns the exact post-LLVM stage bodies underlying this preservation premise.
    pub const fn contents(&self) -> &CheckedPostLlvmStageContentsV1 {
        self.envelopes.contents()
    }

    /// Returns every retained allocatable input section.
    pub fn sections(&self) -> &[Gfx942ObjectSectionCustodyV1] {
        &self.sections
    }

    /// Returns every retained defined global or weak input symbol.
    pub fn symbols(&self) -> &[Gfx942ObjectSymbolCustodyV1] {
        &self.symbols
    }

    /// Returns the complete bounded input relocation roster.
    pub fn relocations(&self) -> &[Gfx942ObjectRelocationCustodyV1] {
        &self.relocations
    }

    /// Reports exact replay-output custody and checked section/symbol preservation premises.
    pub const fn retains_object_to_hsaco_structural_and_replay_premises(&self) -> bool {
        true
    }

    /// Reports independent evaluation of every admitted input relocation against final bytes.
    pub const fn proves_relocation_semantics(&self) -> bool {
        true
    }

    /// Structural matching and exact replay do not prove linker semantic refinement.
    pub const fn proves_linker_semantic_refinement(&self) -> bool {
        false
    }

    /// This owner grants no compiler, publication, load, or launch authority.
    pub const fn grants_authority(&self) -> bool {
        false
    }

    /// Returns the checked gfx950 envelope owner.
    pub fn into_envelopes(self) -> CheckedGfx950PostLlvmStageEnvelopesV1 {
        self.envelopes
    }
}

impl fmt::Debug for CheckedGfx942PostLlvmStageEnvelopesV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CheckedGfx942PostLlvmStageEnvelopesV1")
            .field("record", &self.contents.record().identity())
            .finish_non_exhaustive()
    }
}

impl CheckedGfx942PostLlvmStageEnvelopesV1 {
    /// Returns the independently matched exact stage bodies.
    pub const fn contents(&self) -> &CheckedPostLlvmStageContentsV1 {
        &self.contents
    }

    /// Reports exact pre/post bitcode and recorded pass-declaration custody.
    pub const fn retains_exact_llvm_stage_contents_and_recorded_passes(&self) -> bool {
        true
    }

    /// Reports an exact structurally valid gfx942 relocatable-object envelope.
    pub const fn retains_exact_gfx942_relocatable_object(&self) -> bool {
        true
    }

    /// Reports an exact structurally valid gfx942 AMDHSA ELF envelope.
    pub const fn retains_exact_gfx942_amdhsa_elf_envelope(&self) -> bool {
        true
    }

    /// Reports the COV6 ABI marker checked on both retained ELF artifacts.
    pub const fn proves_code_object_version(&self) -> bool {
        true
    }

    /// Returns false because retaining a metadata note does not validate its semantics.
    pub const fn proves_hsaco_metadata(&self) -> bool {
        false
    }

    /// Reports exact preservation of the binary AMDGPU COV6 metadata descriptor.
    pub const fn retains_exact_cov6_metadata_descriptor_across_link(&self) -> bool {
        true
    }

    /// Envelope checks do not replay LLVM optimization.
    pub const fn proves_llvm_optimization_semantics(&self) -> bool {
        false
    }

    /// Envelope checks do not establish LLVM instruction selection.
    pub const fn proves_instruction_selection(&self) -> bool {
        false
    }

    /// Envelope checks do not establish relocation or symbol preservation.
    pub const fn proves_object_to_hsaco_preservation(&self) -> bool {
        false
    }

    /// This evidence grants no compiler or runtime authority.
    pub const fn grants_authority(&self) -> bool {
        false
    }

    /// Returns the target-neutral exact-stage owner.
    pub fn into_contents(self) -> CheckedPostLlvmStageContentsV1 {
        self.contents
    }
}

impl fmt::Debug for CheckedGfx950PostLlvmStageEnvelopesV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CheckedGfx950PostLlvmStageEnvelopesV1")
            .field("record", &self.contents.record().identity())
            .finish_non_exhaustive()
    }
}

impl CheckedGfx950PostLlvmStageEnvelopesV1 {
    /// Returns the independently matched exact stage bodies.
    pub const fn contents(&self) -> &CheckedPostLlvmStageContentsV1 {
        &self.contents
    }

    /// Reports exact pre/post bitcode and recorded pass-declaration custody.
    pub const fn retains_exact_llvm_stage_contents_and_recorded_passes(&self) -> bool {
        true
    }

    /// Reports an exact structurally valid gfx950 relocatable-object envelope.
    pub const fn retains_exact_gfx950_relocatable_object(&self) -> bool {
        true
    }

    /// Reports an exact structurally valid gfx950 AMDHSA ELF envelope.
    pub const fn retains_exact_gfx950_amdhsa_elf_envelope(&self) -> bool {
        true
    }

    /// Reports the COV6 ABI marker checked on both retained ELF artifacts.
    pub const fn proves_code_object_version(&self) -> bool {
        true
    }

    /// Returns false because retaining a metadata note does not validate its semantics.
    pub const fn proves_hsaco_metadata(&self) -> bool {
        false
    }

    /// Reports exact preservation of the binary AMDGPU COV6 metadata descriptor.
    pub const fn retains_exact_cov6_metadata_descriptor_across_link(&self) -> bool {
        true
    }

    /// Envelope checks do not establish optimization, instruction selection, or linking semantics.
    pub const fn grants_authority(&self) -> bool {
        false
    }

    /// Returns the target-neutral exact-stage owner.
    pub fn into_contents(self) -> CheckedPostLlvmStageContentsV1 {
        self.contents
    }
}

/// Checks raw LLVM-bitcode and AMDGPU ELF envelopes without interpreting display text.
pub fn check_gfx942_post_llvm_stage_envelopes_v1(
    contents: CheckedPostLlvmStageContentsV1,
) -> Result<CheckedGfx942PostLlvmStageEnvelopesV1, Gfx942PostLlvmStageEnvelopeErrorV1> {
    check_llvm_bitcode(contents.pre_optimization_bitcode())
        .map_err(|_| Gfx942PostLlvmStageEnvelopeErrorV1::InvalidPreOptimizationBitcode)?;
    check_llvm_bitcode(contents.post_optimization_bitcode())
        .map_err(|_| Gfx942PostLlvmStageEnvelopeErrorV1::InvalidPostOptimizationBitcode)?;
    let object = check_amdgpu_elf(
        contents.generated_object(),
        ET_REL,
        false,
        GFX942_XNACK_MINUS_ELF_FLAGS,
    )
    .map_err(|_| Gfx942PostLlvmStageEnvelopeErrorV1::InvalidRelocatableObject)?;
    let hsaco = check_amdgpu_elf(
        contents.final_code_object(),
        ET_DYN,
        true,
        GFX942_XNACK_MINUS_ELF_FLAGS,
    )
    .map_err(|_| Gfx942PostLlvmStageEnvelopeErrorV1::InvalidFinalAmdhsaElfEnvelope)?;
    if object.metadata != hsaco.metadata {
        return Err(Gfx942PostLlvmStageEnvelopeErrorV1::Cov6MetadataNotPreserved);
    }
    Ok(CheckedGfx942PostLlvmStageEnvelopesV1 { contents })
}

/// Checks raw LLVM-bitcode and exact gfx950:sramecc-:xnack- COV6 ELF envelopes.
pub fn check_gfx950_post_llvm_stage_envelopes_v1(
    contents: CheckedPostLlvmStageContentsV1,
) -> Result<CheckedGfx950PostLlvmStageEnvelopesV1, Gfx950PostLlvmStageEnvelopeErrorV1> {
    check_llvm_bitcode(contents.pre_optimization_bitcode())
        .map_err(|_| Gfx950PostLlvmStageEnvelopeErrorV1::InvalidPreOptimizationBitcode)?;
    check_llvm_bitcode(contents.post_optimization_bitcode())
        .map_err(|_| Gfx950PostLlvmStageEnvelopeErrorV1::InvalidPostOptimizationBitcode)?;
    let object = check_amdgpu_elf(
        contents.generated_object(),
        ET_REL,
        false,
        GFX950_XNACK_MINUS_SRAM_ECC_MINUS_ELF_FLAGS,
    )
    .map_err(|_| Gfx950PostLlvmStageEnvelopeErrorV1::InvalidRelocatableObject)?;
    let hsaco = check_amdgpu_elf(
        contents.final_code_object(),
        ET_DYN,
        true,
        GFX950_XNACK_MINUS_SRAM_ECC_MINUS_ELF_FLAGS,
    )
    .map_err(|_| Gfx950PostLlvmStageEnvelopeErrorV1::InvalidFinalAmdhsaElfEnvelope)?;
    if object.metadata != hsaco.metadata {
        return Err(Gfx950PostLlvmStageEnvelopeErrorV1::Cov6MetadataNotPreserved);
    }
    Ok(CheckedGfx950PostLlvmStageEnvelopesV1 { contents })
}

/// Checks bounded named section/symbol custody and exact object-to-HSACO replay output.
///
/// The replayed output must be byte-identical to the retained final HSACO. This validates a
/// deterministic custody premise for the supplied linker occurrence, not relocation application
/// or linker semantics.
pub fn check_gfx942_object_to_hsaco_preservation_v1(
    envelopes: CheckedGfx942PostLlvmStageEnvelopesV1,
    independently_replayed_hsaco: &[u8],
) -> Result<CheckedGfx942ObjectToHsacoPreservationV1, Gfx942ObjectToHsacoPreservationErrorV1> {
    if !envelopes
        .contents()
        .retains_complete_occurrence_and_assembly_replay_custody()
    {
        return Err(Gfx942ObjectToHsacoPreservationErrorV1::MissingPipelineOccurrenceCustody);
    }
    if independently_replayed_hsaco != envelopes.contents().final_code_object() {
        return Err(Gfx942ObjectToHsacoPreservationErrorV1::HsacoReplayMismatch);
    }
    let object_bytes = envelopes.contents().generated_object();
    let hsaco_bytes = envelopes.contents().final_code_object();
    let object = parse_elf_inventory(object_bytes)
        .map_err(|_| Gfx942ObjectToHsacoPreservationErrorV1::InvalidObjectInventory)?;
    let hsaco = parse_elf_inventory(hsaco_bytes)
        .map_err(|_| Gfx942ObjectToHsacoPreservationErrorV1::InvalidHsacoInventory)?;

    let mut retained_sections = Vec::new();
    for input in object.sections.iter().filter(|section| {
        section.flags & SHF_ALLOC != 0 && matches!(section.section_type, SHT_PROGBITS | SHT_NOBITS)
    }) {
        let output = unique_named_section(&hsaco.sections, &input.name)
            .ok_or(Gfx942ObjectToHsacoPreservationErrorV1::SectionNotPreserved)?;
        let relevant_flags = SHF_WRITE | SHF_ALLOC | SHF_EXECINSTR;
        if output.section_type != input.section_type
            || output.flags & relevant_flags != input.flags & relevant_flags
            || output.size < input.size
            || output.alignment < input.alignment
            || (input.alignment != 0 && output.alignment % input.alignment != 0)
        {
            return Err(Gfx942ObjectToHsacoPreservationErrorV1::SectionNotPreserved);
        }
        let has_relocations = object
            .relocations
            .iter()
            .any(|relocation| relocation.target_section_name == input.name);
        if input.section_type == SHT_PROGBITS && !has_relocations {
            let input_data = section_data(object_bytes, input)
                .ok_or(Gfx942ObjectToHsacoPreservationErrorV1::InvalidObjectInventory)?;
            let output_data = section_data(hsaco_bytes, output)
                .ok_or(Gfx942ObjectToHsacoPreservationErrorV1::InvalidHsacoInventory)?;
            if !contains_exact_bytes(output_data, input_data) {
                return Err(Gfx942ObjectToHsacoPreservationErrorV1::SectionNotPreserved);
            }
        }
        retained_sections.push(Gfx942ObjectSectionCustodyV1 {
            name: input.name.clone(),
            section_type: input.section_type,
            flags: input.flags,
            byte_len: input.size,
            alignment: input.alignment,
            has_relocations,
        });
    }

    for input in &object.symbols {
        if !hsaco.symbols.iter().any(|output| {
            output.name == input.name
                && output.binding == input.binding
                && output.symbol_type == input.symbol_type
                && output.visibility == input.visibility
                && output.section_name == input.section_name
                && output.byte_len == input.byte_len
        }) {
            return Err(Gfx942ObjectToHsacoPreservationErrorV1::SymbolNotPreserved);
        }
    }
    if !hsaco.relocations.is_empty() && hsaco.relocations != object.relocations {
        return Err(Gfx942ObjectToHsacoPreservationErrorV1::RelocationRosterNotPreserved);
    }
    validate_relocation_applications(&object, &hsaco, hsaco_bytes).map_err(
        |error| match error {
            RelocationApplicationErrorV1::UnsupportedType => {
                Gfx942ObjectToHsacoPreservationErrorV1::UnsupportedRelocationType
            }
            RelocationApplicationErrorV1::InvalidCoordinates
            | RelocationApplicationErrorV1::ValueMismatch => {
                Gfx942ObjectToHsacoPreservationErrorV1::RelocationApplicationMismatch
            }
        },
    )?;
    Ok(CheckedGfx942ObjectToHsacoPreservationV1 {
        envelopes,
        sections: retained_sections.into_boxed_slice(),
        symbols: object.symbols.into_boxed_slice(),
        relocations: object.relocations.into_boxed_slice(),
    })
}

/// Checks gfx950 section/symbol custody and exact object-to-HSACO replay output.
///
/// Relocation application remains an explicit unavailable semantic premise. Relocation rosters
/// are retained and compared, but construction does not claim that LLD applied each expression.
pub fn check_gfx950_object_to_hsaco_preservation_v1(
    envelopes: CheckedGfx950PostLlvmStageEnvelopesV1,
    independently_replayed_hsaco: &[u8],
) -> Result<CheckedGfx950ObjectToHsacoPreservationV1, Gfx950ObjectToHsacoPreservationErrorV1> {
    if !envelopes
        .contents()
        .retains_complete_occurrence_and_assembly_replay_custody()
    {
        return Err(Gfx950ObjectToHsacoPreservationErrorV1::MissingPipelineOccurrenceCustody);
    }
    if independently_replayed_hsaco != envelopes.contents().final_code_object() {
        return Err(Gfx950ObjectToHsacoPreservationErrorV1::HsacoReplayMismatch);
    }
    let object_bytes = envelopes.contents().generated_object();
    let hsaco_bytes = envelopes.contents().final_code_object();
    let object = parse_elf_inventory(object_bytes)
        .map_err(|_| Gfx950ObjectToHsacoPreservationErrorV1::InvalidObjectInventory)?;
    let hsaco = parse_elf_inventory(hsaco_bytes)
        .map_err(|_| Gfx950ObjectToHsacoPreservationErrorV1::InvalidHsacoInventory)?;

    let mut retained_sections = Vec::new();
    for input in object.sections.iter().filter(|section| {
        section.flags & SHF_ALLOC != 0 && matches!(section.section_type, SHT_PROGBITS | SHT_NOBITS)
    }) {
        let output = unique_named_section(&hsaco.sections, &input.name)
            .ok_or(Gfx950ObjectToHsacoPreservationErrorV1::SectionNotPreserved)?;
        let relevant_flags = SHF_WRITE | SHF_ALLOC | SHF_EXECINSTR;
        if output.section_type != input.section_type
            || output.flags & relevant_flags != input.flags & relevant_flags
            || output.size < input.size
            || output.alignment < input.alignment
            || (input.alignment != 0 && output.alignment % input.alignment != 0)
        {
            return Err(Gfx950ObjectToHsacoPreservationErrorV1::SectionNotPreserved);
        }
        let has_relocations = object
            .relocations
            .iter()
            .any(|relocation| relocation.target_section_name == input.name);
        if input.section_type == SHT_PROGBITS && !has_relocations {
            let input_data = section_data(object_bytes, input)
                .ok_or(Gfx950ObjectToHsacoPreservationErrorV1::InvalidObjectInventory)?;
            let output_data = section_data(hsaco_bytes, output)
                .ok_or(Gfx950ObjectToHsacoPreservationErrorV1::InvalidHsacoInventory)?;
            if !contains_exact_bytes(output_data, input_data) {
                return Err(Gfx950ObjectToHsacoPreservationErrorV1::SectionNotPreserved);
            }
        }
        retained_sections.push(Gfx942ObjectSectionCustodyV1 {
            name: input.name.clone(),
            section_type: input.section_type,
            flags: input.flags,
            byte_len: input.size,
            alignment: input.alignment,
            has_relocations,
        });
    }

    for input in &object.symbols {
        if !hsaco.symbols.iter().any(|output| {
            output.name == input.name
                && output.binding == input.binding
                && output.symbol_type == input.symbol_type
                && output.visibility == input.visibility
                && output.section_name == input.section_name
                && output.byte_len == input.byte_len
        }) {
            return Err(Gfx950ObjectToHsacoPreservationErrorV1::SymbolNotPreserved);
        }
    }
    if !hsaco.relocations.is_empty() && hsaco.relocations != object.relocations {
        return Err(Gfx950ObjectToHsacoPreservationErrorV1::RelocationRosterNotPreserved);
    }
    validate_relocation_applications(&object, &hsaco, hsaco_bytes).map_err(
        |error| match error {
            RelocationApplicationErrorV1::UnsupportedType => {
                Gfx950ObjectToHsacoPreservationErrorV1::UnsupportedRelocationType
            }
            RelocationApplicationErrorV1::InvalidCoordinates
            | RelocationApplicationErrorV1::ValueMismatch => {
                Gfx950ObjectToHsacoPreservationErrorV1::RelocationApplicationMismatch
            }
        },
    )?;
    Ok(CheckedGfx950ObjectToHsacoPreservationV1 {
        envelopes,
        sections: retained_sections.into_boxed_slice(),
        symbols: object.symbols.into_boxed_slice(),
        relocations: object.relocations.into_boxed_slice(),
    })
}

/// Stable failures for bounded object-to-HSACO structural custody.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Gfx942ObjectToHsacoPreservationErrorV1 {
    /// Complete post-LLVM occurrence custody was not attached.
    MissingPipelineOccurrenceCustody,
    /// Independent linker replay did not reproduce the exact retained HSACO.
    HsacoReplayMismatch,
    /// The relocatable object's bounded named ELF inventory was malformed.
    InvalidObjectInventory,
    /// The final HSACO's bounded named ELF inventory was malformed.
    InvalidHsacoInventory,
    /// An allocatable input section or its exact unrelocated bytes were absent in the HSACO.
    SectionNotPreserved,
    /// A defined global or weak input symbol was absent or structurally substituted.
    SymbolNotPreserved,
    /// Relocations retained in the final ELF were omitted, reordered, or substituted.
    RelocationRosterNotPreserved,
    /// An input relocation uses an expression outside the admitted AMDGPU subset.
    UnsupportedRelocationType,
    /// Independently evaluated relocation bytes differ from the final HSACO.
    RelocationApplicationMismatch,
}

impl fmt::Display for Gfx942ObjectToHsacoPreservationErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "gfx942 object-to-HSACO custody rejected: {self:?}"
        )
    }
}

impl std::error::Error for Gfx942ObjectToHsacoPreservationErrorV1 {}

/// Stable failures for bounded gfx950 object-to-HSACO structural custody.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Gfx950ObjectToHsacoPreservationErrorV1 {
    /// Complete post-LLVM occurrence custody was not attached.
    MissingPipelineOccurrenceCustody,
    /// Independent linker replay did not reproduce the exact retained HSACO.
    HsacoReplayMismatch,
    /// The relocatable object's bounded named ELF inventory was malformed.
    InvalidObjectInventory,
    /// The final HSACO's bounded named ELF inventory was malformed.
    InvalidHsacoInventory,
    /// An allocatable input section or its exact unrelocated bytes were absent in the HSACO.
    SectionNotPreserved,
    /// A defined global or weak input symbol was absent or structurally substituted.
    SymbolNotPreserved,
    /// Relocations retained in the final ELF were omitted, reordered, or substituted.
    RelocationRosterNotPreserved,
    /// An input relocation uses an expression outside the admitted AMDGPU subset.
    UnsupportedRelocationType,
    /// Independently evaluated relocation bytes differ from the final HSACO.
    RelocationApplicationMismatch,
}

impl fmt::Display for Gfx950ObjectToHsacoPreservationErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "gfx950 object-to-HSACO custody rejected: {self:?}"
        )
    }
}

impl std::error::Error for Gfx950ObjectToHsacoPreservationErrorV1 {}

/// Stable failures for gfx942 post-LLVM envelope checking.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Gfx942PostLlvmStageEnvelopeErrorV1 {
    /// The pre-optimization bytes are not a bounded LLVM bitcode envelope.
    InvalidPreOptimizationBitcode,
    /// The post-optimization bytes are not a bounded LLVM bitcode envelope.
    InvalidPostOptimizationBitcode,
    /// The object is not an ELF64 little-endian AMDGPU gfx942:xnack- relocatable.
    InvalidRelocatableObject,
    /// The final artifact is not an ELF64 little-endian AMDHSA gfx942:xnack- shared object.
    InvalidFinalAmdhsaElfEnvelope,
    /// The object and final HSACO do not retain the same exact AMDGPU metadata descriptor.
    Cov6MetadataNotPreserved,
}

impl fmt::Display for Gfx942PostLlvmStageEnvelopeErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "gfx942 post-LLVM envelope rejected: {self:?}")
    }
}

impl std::error::Error for Gfx942PostLlvmStageEnvelopeErrorV1 {}

/// Stable failures for gfx950 post-LLVM envelope checking.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Gfx950PostLlvmStageEnvelopeErrorV1 {
    /// The pre-optimization bytes are not a bounded LLVM bitcode envelope.
    InvalidPreOptimizationBitcode,
    /// The post-optimization bytes are not a bounded LLVM bitcode envelope.
    InvalidPostOptimizationBitcode,
    /// The object is not an ELF64 little-endian AMDGPU gfx950:sramecc-:xnack- relocatable.
    InvalidRelocatableObject,
    /// The final artifact is not an ELF64 little-endian AMDHSA gfx950:sramecc-:xnack- shared object.
    InvalidFinalAmdhsaElfEnvelope,
    /// The object and final HSACO do not retain the same exact AMDGPU metadata descriptor.
    Cov6MetadataNotPreserved,
}

impl fmt::Display for Gfx950PostLlvmStageEnvelopeErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "gfx950 post-LLVM envelope rejected: {self:?}")
    }
}

impl std::error::Error for Gfx950PostLlvmStageEnvelopeErrorV1 {}

fn check_llvm_bitcode(bytes: &[u8]) -> Result<(), ()> {
    if bytes.starts_with(RAW_LLVM_BITCODE_MAGIC) {
        return Ok(());
    }
    if bytes.len() < LLVM_BITCODE_WRAPPER_HEADER_BYTES
        || !bytes.starts_with(LLVM_BITCODE_WRAPPER_MAGIC)
    {
        return Err(());
    }
    let offset = read_u32(bytes, 8)? as usize;
    let length = read_u32(bytes, 12)? as usize;
    let end = offset.checked_add(length).ok_or(())?;
    if offset < LLVM_BITCODE_WRAPPER_HEADER_BYTES
        || end > bytes.len()
        || length < RAW_LLVM_BITCODE_MAGIC.len()
        || &bytes[offset..offset + RAW_LLVM_BITCODE_MAGIC.len()] != RAW_LLVM_BITCODE_MAGIC
    {
        return Err(());
    }
    Ok(())
}

struct CheckedElfFacts<'a> {
    metadata: &'a [u8],
}

fn check_amdgpu_elf(
    bytes: &[u8],
    expected_type: u16,
    require_hsa_osabi: bool,
    expected_flags: u32,
) -> Result<CheckedElfFacts<'_>, ()> {
    if bytes.len() < ELF64_HEADER_BYTES
        || &bytes[..4] != ELF_MAGIC
        || bytes[4] != ELFCLASS64
        || bytes[5] != ELFDATA2LSB
        || bytes[6] != EV_CURRENT
        || (require_hsa_osabi && bytes[7] != ELFOSABI_AMDGPU_HSA)
        || (!require_hsa_osabi && !matches!(bytes[7], ELFOSABI_NONE | ELFOSABI_AMDGPU_HSA))
        || bytes[8] != COV6_ABI_VERSION
        || read_u16(bytes, 16)? != expected_type
        || read_u16(bytes, 18)? != EM_AMDGPU
        || read_u32(bytes, 20)? != u32::from(EV_CURRENT)
        || read_u32(bytes, 48)? != expected_flags
        || read_u16(bytes, 52)? != ELF64_HEADER_BYTES as u16
    {
        return Err(());
    }
    let section_offset = usize::try_from(read_u64(bytes, 40)?).map_err(|_| ())?;
    let section_entry_size = usize::from(read_u16(bytes, 58)?);
    let section_count = usize::from(read_u16(bytes, 60)?);
    if section_count == 0
        || section_count > MAX_ELF_SECTIONS
        || section_entry_size != ELF64_SECTION_HEADER_BYTES
        || checked_table_end(section_offset, section_entry_size, section_count)? > bytes.len()
    {
        return Err(());
    }
    let mut metadata = None;
    for ordinal in 0..section_count {
        let section = section_offset
            .checked_add(ordinal.checked_mul(section_entry_size).ok_or(())?)
            .ok_or(())?;
        if read_u32(bytes, section + 4)? != SHT_NOTE {
            continue;
        }
        let offset = usize::try_from(read_u64(bytes, section + 24)?).map_err(|_| ())?;
        let size = usize::try_from(read_u64(bytes, section + 32)?).map_err(|_| ())?;
        let alignment = usize::try_from(read_u64(bytes, section + 48)?).map_err(|_| ())?;
        if alignment != 4 {
            return Err(());
        }
        let notes = bytes
            .get(offset..offset.checked_add(size).ok_or(())?)
            .ok_or(())?;
        let mut cursor = 0;
        while cursor < notes.len() {
            let header_end = cursor.checked_add(12).ok_or(())?;
            if header_end > notes.len() {
                return Err(());
            }
            let name_size = usize::try_from(read_u32(notes, cursor)?).map_err(|_| ())?;
            let description_size = usize::try_from(read_u32(notes, cursor + 4)?).map_err(|_| ())?;
            let note_type = read_u32(notes, cursor + 8)?;
            let name_start = header_end;
            let name_end = name_start.checked_add(name_size).ok_or(())?;
            let description_start = align4(name_end)?;
            let description_end = description_start.checked_add(description_size).ok_or(())?;
            let next = align4(description_end)?;
            if next > notes.len() {
                return Err(());
            }
            let name = notes.get(name_start..name_end).ok_or(())?;
            if (name == b"AMDGPU" || name == b"AMDGPU\0") && note_type == NT_AMDGPU_METADATA {
                let description = notes.get(description_start..description_end).ok_or(())?;
                if description.is_empty() || metadata.replace(description).is_some() {
                    return Err(());
                }
            }
            cursor = next;
        }
    }
    Ok(CheckedElfFacts {
        metadata: metadata.ok_or(())?,
    })
}

#[derive(Clone)]
struct ParsedElfSection {
    name: String,
    section_type: u32,
    flags: u64,
    address: u64,
    offset: u64,
    size: u64,
    link: u32,
    info: u32,
    alignment: u64,
    entry_size: u64,
}

struct ParsedElfInventory {
    sections: Vec<ParsedElfSection>,
    symbols: Vec<Gfx942ObjectSymbolCustodyV1>,
    relocations: Vec<Gfx942ObjectRelocationCustodyV1>,
}

pub(crate) struct Gfx942ExecutableSectionV1<'a> {
    pub(crate) file_offset: u64,
    pub(crate) bytes: &'a [u8],
}

fn parse_elf_inventory(bytes: &[u8]) -> Result<ParsedElfInventory, ()> {
    let section_offset = usize::try_from(read_u64(bytes, 40)?).map_err(|_| ())?;
    let section_entry_size = usize::from(read_u16(bytes, 58)?);
    let section_count = usize::from(read_u16(bytes, 60)?);
    let names_index = usize::from(read_u16(bytes, 62)?);
    if section_count == 0
        || section_count > MAX_ELF_SECTIONS
        || section_entry_size != ELF64_SECTION_HEADER_BYTES
        || names_index >= section_count
        || checked_table_end(section_offset, section_entry_size, section_count)? > bytes.len()
    {
        return Err(());
    }
    let raw_sections = (0..section_count)
        .map(|index| parse_section_header(bytes, section_offset, section_entry_size, index))
        .collect::<Result<Vec<_>, _>>()?;
    if raw_sections[names_index].section_type != SHT_STRTAB {
        return Err(());
    }
    let names = section_data(bytes, &raw_sections[names_index]).ok_or(())?;
    let mut sections = Vec::with_capacity(section_count);
    for (index, mut section) in raw_sections.into_iter().enumerate() {
        let header = section_offset
            .checked_add(index.checked_mul(section_entry_size).ok_or(())?)
            .ok_or(())?;
        section.name = read_elf_name(names, read_u32(bytes, header)?)?.to_owned();
        if section.section_type != SHT_NOBITS {
            section_data(bytes, &section).ok_or(())?;
        }
        sections.push(section);
    }

    let mut symbols = Vec::new();
    let mut total_symbols = 0_usize;
    for section in &sections {
        if !matches!(section.section_type, SHT_SYMTAB | SHT_DYNSYM) {
            continue;
        }
        let count = symbol_count(section)?;
        total_symbols = total_symbols.checked_add(count).ok_or(())?;
        if total_symbols > MAX_ELF_SYMBOLS {
            return Err(());
        }
        let string_table = sections.get(section.link as usize).ok_or(())?;
        if string_table.section_type != SHT_STRTAB {
            return Err(());
        }
        let strings = section_data(bytes, string_table).ok_or(())?;
        let table = section_data(bytes, section).ok_or(())?;
        for ordinal in 0..count {
            let offset = ordinal.checked_mul(24).ok_or(())?;
            let name = read_elf_name(strings, read_u32(table, offset)?)?;
            let info = *table.get(offset + 4).ok_or(())?;
            let visibility = *table.get(offset + 5).ok_or(())? & 0x3;
            let defining_index = usize::from(read_u16(table, offset + 6)?);
            let binding = info >> 4;
            if matches!(binding, 1 | 2) && defining_index != 0 && defining_index < 0xff00 {
                let defining_section = sections.get(defining_index).ok_or(())?;
                if name.is_empty() || defining_section.name.is_empty() {
                    return Err(());
                }
                symbols.push(Gfx942ObjectSymbolCustodyV1 {
                    name: name.to_owned(),
                    binding,
                    symbol_type: info & 0xf,
                    visibility,
                    section_name: defining_section.name.clone(),
                    value: read_u64(table, offset + 8)?,
                    byte_len: read_u64(table, offset + 16)?,
                });
            }
        }
    }

    let mut relocations = Vec::new();
    for section in &sections {
        if !matches!(section.section_type, SHT_RELA | SHT_REL) {
            continue;
        }
        let expected_entry_size = if section.section_type == SHT_RELA {
            24
        } else {
            16
        };
        if section.entry_size != expected_entry_size || section.size % expected_entry_size != 0 {
            return Err(());
        }
        let count = usize::try_from(section.size / expected_entry_size).map_err(|_| ())?;
        if relocations.len().checked_add(count).ok_or(())? > MAX_ELF_RELOCATIONS {
            return Err(());
        }
        let target = sections.get(section.info as usize).ok_or(())?;
        let symbol_table = sections.get(section.link as usize).ok_or(())?;
        if target.name.is_empty() || !matches!(symbol_table.section_type, SHT_SYMTAB | SHT_DYNSYM) {
            return Err(());
        }
        let table = section_data(bytes, section).ok_or(())?;
        for ordinal in 0..count {
            let offset = ordinal
                .checked_mul(expected_entry_size as usize)
                .ok_or(())?;
            let relocation_info = read_u64(table, offset + 8)?;
            let symbol_index = usize::try_from(relocation_info >> 32).map_err(|_| ())?;
            relocations.push(Gfx942ObjectRelocationCustodyV1 {
                target_section_name: target.name.clone(),
                symbol_name: symbol_name(bytes, &sections, symbol_table, symbol_index)?.to_owned(),
                offset: read_u64(table, offset)?,
                relocation_type: relocation_info as u32,
                addend: if section.section_type == SHT_RELA {
                    Some(read_i64(table, offset + 16)?)
                } else {
                    None
                },
            });
        }
    }
    Ok(ParsedElfInventory {
        sections,
        symbols,
        relocations,
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RelocationApplicationErrorV1 {
    UnsupportedType,
    InvalidCoordinates,
    ValueMismatch,
}

const R_AMDGPU_REL64: u32 = 5;

fn validate_relocation_applications(
    object: &ParsedElfInventory,
    hsaco: &ParsedElfInventory,
    hsaco_bytes: &[u8],
) -> Result<(), RelocationApplicationErrorV1> {
    for relocation in &object.relocations {
        if relocation.relocation_type != R_AMDGPU_REL64 || relocation.addend.is_none() {
            return Err(RelocationApplicationErrorV1::UnsupportedType);
        }
        let input_section = unique_named_section(&object.sections, &relocation.target_section_name)
            .ok_or(RelocationApplicationErrorV1::InvalidCoordinates)?;
        let output_section = unique_named_section(&hsaco.sections, &relocation.target_section_name)
            .ok_or(RelocationApplicationErrorV1::InvalidCoordinates)?;
        let output_symbol = hsaco
            .symbols
            .iter()
            .find(|symbol| symbol.name == relocation.symbol_name)
            .ok_or(RelocationApplicationErrorV1::InvalidCoordinates)?;
        let placement = output_section_placement(object, hsaco, input_section, output_section)?;
        let place = output_section
            .address
            .checked_add(placement)
            .and_then(|value| value.checked_add(relocation.offset))
            .ok_or(RelocationApplicationErrorV1::InvalidCoordinates)?;
        let destination = placement
            .checked_add(relocation.offset)
            .ok_or(RelocationApplicationErrorV1::InvalidCoordinates)?;
        let destination = usize::try_from(destination)
            .map_err(|_| RelocationApplicationErrorV1::InvalidCoordinates)?;
        let output_data = section_data(hsaco_bytes, output_section)
            .ok_or(RelocationApplicationErrorV1::InvalidCoordinates)?;
        let actual = output_data
            .get(
                destination
                    ..destination
                        .checked_add(8)
                        .ok_or(RelocationApplicationErrorV1::InvalidCoordinates)?,
            )
            .ok_or(RelocationApplicationErrorV1::InvalidCoordinates)?;
        let expected = output_symbol
            .value
            .wrapping_add_signed(relocation.addend.expect("RELA checked above"))
            .wrapping_sub(place)
            .to_le_bytes();
        if actual != expected {
            return Err(RelocationApplicationErrorV1::ValueMismatch);
        }
    }
    Ok(())
}

fn output_section_placement(
    object: &ParsedElfInventory,
    hsaco: &ParsedElfInventory,
    input_section: &ParsedElfSection,
    output_section: &ParsedElfSection,
) -> Result<u64, RelocationApplicationErrorV1> {
    let mut placement = None;
    for input_symbol in object
        .symbols
        .iter()
        .filter(|symbol| symbol.section_name == input_section.name)
    {
        let Some(output_symbol) = hsaco.symbols.iter().find(|candidate| {
            candidate.name == input_symbol.name && candidate.section_name == output_section.name
        }) else {
            continue;
        };
        let input_offset = input_symbol
            .value
            .checked_sub(input_section.address)
            .ok_or(RelocationApplicationErrorV1::InvalidCoordinates)?;
        let output_offset = output_symbol
            .value
            .checked_sub(output_section.address)
            .ok_or(RelocationApplicationErrorV1::InvalidCoordinates)?;
        let candidate = output_offset
            .checked_sub(input_offset)
            .ok_or(RelocationApplicationErrorV1::InvalidCoordinates)?;
        if placement
            .replace(candidate)
            .is_some_and(|prior| prior != candidate)
        {
            return Err(RelocationApplicationErrorV1::InvalidCoordinates);
        }
    }
    match placement {
        Some(value) => Ok(value),
        None if input_section.size == output_section.size => Ok(0),
        None => Err(RelocationApplicationErrorV1::InvalidCoordinates),
    }
}

fn parse_section_header(
    bytes: &[u8],
    table_offset: usize,
    entry_size: usize,
    index: usize,
) -> Result<ParsedElfSection, ()> {
    let offset = table_offset
        .checked_add(index.checked_mul(entry_size).ok_or(())?)
        .ok_or(())?;
    Ok(ParsedElfSection {
        name: String::new(),
        section_type: read_u32(bytes, offset + 4)?,
        flags: read_u64(bytes, offset + 8)?,
        address: read_u64(bytes, offset + 16)?,
        offset: read_u64(bytes, offset + 24)?,
        size: read_u64(bytes, offset + 32)?,
        link: read_u32(bytes, offset + 40)?,
        info: read_u32(bytes, offset + 44)?,
        alignment: read_u64(bytes, offset + 48)?,
        entry_size: read_u64(bytes, offset + 56)?,
    })
}

pub(crate) fn decode_gfx942_hsaco_kernel_mode_words_v1(bytes: &[u8]) -> Result<Box<[u32]>, ()> {
    decode_hsaco_kernel_mode_words_v1(bytes)
}

pub(crate) fn decode_gfx950_hsaco_kernel_mode_words_v1(bytes: &[u8]) -> Result<Box<[u32]>, ()> {
    decode_hsaco_kernel_mode_words_v1(bytes)
}

fn decode_hsaco_kernel_mode_words_v1(bytes: &[u8]) -> Result<Box<[u32]>, ()> {
    let inventory = parse_elf_inventory(bytes)?;
    let mut descriptors = Vec::<(String, u32)>::new();
    for symbol_table in &inventory.sections {
        if !matches!(symbol_table.section_type, SHT_SYMTAB | SHT_DYNSYM) {
            continue;
        }
        let count = symbol_count(symbol_table)?;
        if count > MAX_ELF_SYMBOLS {
            return Err(());
        }
        let string_table = inventory
            .sections
            .get(symbol_table.link as usize)
            .ok_or(())?;
        if string_table.section_type != SHT_STRTAB {
            return Err(());
        }
        let strings = section_data(bytes, string_table).ok_or(())?;
        let table = section_data(bytes, symbol_table).ok_or(())?;
        for ordinal in 0..count {
            let offset = ordinal.checked_mul(24).ok_or(())?;
            let name = read_elf_name(strings, read_u32(table, offset)?)?;
            let info = *table.get(offset + 4).ok_or(())?;
            let section_index = usize::from(read_u16(table, offset + 6)?);
            let value = read_u64(table, offset + 8)?;
            let size = read_u64(table, offset + 16)?;
            if info & 0xf != 1
                || size != AMDHSA_KERNEL_DESCRIPTOR_BYTES
                || !name.ends_with(".kd")
                || section_index == 0
                || section_index >= 0xff00
            {
                continue;
            }
            let section = inventory.sections.get(section_index).ok_or(())?;
            if section.section_type != SHT_PROGBITS || section.flags & SHF_ALLOC == 0 {
                return Err(());
            }
            let relative = value.checked_sub(section.address).ok_or(())?;
            if relative
                .checked_add(AMDHSA_KERNEL_DESCRIPTOR_BYTES)
                .is_none_or(|end| end > section.size)
            {
                return Err(());
            }
            let descriptor =
                usize::try_from(section.offset.checked_add(relative).ok_or(())?).map_err(|_| ())?;
            let mode_word = read_u32(
                bytes,
                descriptor
                    .checked_add(AMDHSA_COMPUTE_PGM_RSRC1_OFFSET)
                    .ok_or(())?,
            )?;
            descriptors.push((name.to_owned(), mode_word));
        }
    }
    descriptors.sort_unstable();
    descriptors.dedup();
    if descriptors.is_empty()
        || descriptors
            .windows(2)
            .any(|pair| pair[0].0 == pair[1].0 && pair[0].1 != pair[1].1)
    {
        return Err(());
    }
    Ok(descriptors
        .into_iter()
        .map(|(_, mode)| mode)
        .collect::<Vec<_>>()
        .into_boxed_slice())
}

pub(crate) fn decode_gfx942_executable_sections_v1(
    bytes: &[u8],
) -> Result<Box<[Gfx942ExecutableSectionV1<'_>]>, ()> {
    decode_executable_sections_v1(bytes)
}

pub(crate) fn decode_gfx950_executable_sections_v1(
    bytes: &[u8],
) -> Result<Box<[Gfx942ExecutableSectionV1<'_>]>, ()> {
    decode_executable_sections_v1(bytes)
}

fn decode_executable_sections_v1(bytes: &[u8]) -> Result<Box<[Gfx942ExecutableSectionV1<'_>]>, ()> {
    let inventory = parse_elf_inventory(bytes)?;
    let mut sections = inventory
        .sections
        .iter()
        .filter(|section| {
            section.section_type == SHT_PROGBITS && section.flags & SHF_EXECINSTR != 0
        })
        .map(|section| {
            if section.name.is_empty() || section.size == 0 || section.offset % 4 != 0 {
                return Err(());
            }
            Ok(Gfx942ExecutableSectionV1 {
                file_offset: section.offset,
                bytes: section_data(bytes, section).ok_or(())?,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    sections.sort_unstable_by_key(|section| section.file_offset);
    if sections.is_empty()
        || sections.windows(2).any(|pair| {
            pair[0]
                .file_offset
                .checked_add(pair[0].bytes.len() as u64)
                .is_none_or(|end| end > pair[1].file_offset)
        })
    {
        return Err(());
    }
    Ok(sections.into_boxed_slice())
}

fn symbol_count(section: &ParsedElfSection) -> Result<usize, ()> {
    if section.entry_size != 24 || section.size % 24 != 0 {
        return Err(());
    }
    usize::try_from(section.size / 24).map_err(|_| ())
}

fn symbol_name<'a>(
    bytes: &'a [u8],
    sections: &[ParsedElfSection],
    symbol_table: &ParsedElfSection,
    symbol_index: usize,
) -> Result<&'a str, ()> {
    let count = symbol_count(symbol_table)?;
    if symbol_index >= count {
        return Err(());
    }
    let strings = sections.get(symbol_table.link as usize).ok_or(())?;
    if strings.section_type != SHT_STRTAB {
        return Err(());
    }
    let table = section_data(bytes, symbol_table).ok_or(())?;
    let entry = symbol_index.checked_mul(24).ok_or(())?;
    read_elf_name(
        section_data(bytes, strings).ok_or(())?,
        read_u32(table, entry)?,
    )
}

fn section_data<'a>(bytes: &'a [u8], section: &ParsedElfSection) -> Option<&'a [u8]> {
    if section.section_type == SHT_NOBITS {
        return Some(&[]);
    }
    let start = usize::try_from(section.offset).ok()?;
    let size = usize::try_from(section.size).ok()?;
    bytes.get(start..start.checked_add(size)?)
}

fn unique_named_section<'a>(
    sections: &'a [ParsedElfSection],
    name: &str,
) -> Option<&'a ParsedElfSection> {
    let mut matches = sections.iter().filter(|section| section.name == name);
    let first = matches.next()?;
    matches.next().is_none().then_some(first)
}

fn contains_exact_bytes(container: &[u8], exact: &[u8]) -> bool {
    exact.is_empty()
        || container
            .windows(exact.len())
            .any(|candidate| candidate == exact)
}

fn read_elf_name(table: &[u8], offset: u32) -> Result<&str, ()> {
    let start = usize::try_from(offset).map_err(|_| ())?;
    let tail = table.get(start..).ok_or(())?;
    let length = tail.iter().position(|byte| *byte == 0).ok_or(())?;
    if length > MAX_ELF_NAME_BYTES {
        return Err(());
    }
    let name = core::str::from_utf8(&tail[..length]).map_err(|_| ())?;
    if !name.bytes().all(|byte| byte.is_ascii_graphic()) {
        return Err(());
    }
    Ok(name)
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16, ()> {
    let value = bytes.get(offset..offset + 2).ok_or(())?;
    Ok(u16::from_le_bytes(value.try_into().map_err(|_| ())?))
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, ()> {
    let value = bytes.get(offset..offset + 4).ok_or(())?;
    Ok(u32::from_le_bytes(value.try_into().map_err(|_| ())?))
}

fn read_u64(bytes: &[u8], offset: usize) -> Result<u64, ()> {
    let value = bytes.get(offset..offset + 8).ok_or(())?;
    Ok(u64::from_le_bytes(value.try_into().map_err(|_| ())?))
}

fn read_i64(bytes: &[u8], offset: usize) -> Result<i64, ()> {
    let value = bytes.get(offset..offset + 8).ok_or(())?;
    Ok(i64::from_le_bytes(value.try_into().map_err(|_| ())?))
}

fn checked_table_end(offset: usize, entry_size: usize, count: usize) -> Result<usize, ()> {
    offset
        .checked_add(entry_size.checked_mul(count).ok_or(())?)
        .ok_or(())
}

fn align4(value: usize) -> Result<usize, ()> {
    value.checked_add(3).map(|value| value & !3).ok_or(())
}
