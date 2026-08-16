//! Exact logical memory/effect contract for causal FlashAttention B1/H1/N8/D16.
//!
//! The first-stage API is an exhaustive checker for the fixed source model,
//! not proof evidence. Only `join_flash_attention_memory_verus_v1` attaches the
//! separately executed, identity-bound Verus receipt. Neither stage connects
//! logical indices to LLVM or ISA addresses or grants publication, load,
//! launch, or execution authority.

use core::fmt;

pub const FLASH_ATTENTION_MEMORY_SEQUENCE_V1: usize = 8;
pub const FLASH_ATTENTION_MEMORY_DIMENSION_V1: usize = 16;
pub const FLASH_ATTENTION_MEMORY_LANES_V1: usize = 64;
pub const FLASH_ATTENTION_MEMORY_ELEMENTS_V1: usize = 128;
pub const FLASH_ATTENTION_MEMORY_REGION_BYTES_V1: u64 = 512;
pub const FLASH_ATTENTION_MEMORY_GLOBAL_ADDRESS_SPACE_V1: u8 = 1;
pub const FLASH_ATTENTION_MEMORY_ACCESS_WIDTH_V1: u8 = 4;

pub const FLASH_ATTENTION_MEMORY_SOURCE_IDENTITY_V1: [u8; 32] = [
    0x2b, 0x00, 0xa6, 0x4e, 0x43, 0xe6, 0x9c, 0x41, 0x6e, 0x70, 0x08, 0x0e, 0x01, 0x3e, 0xdf, 0x90,
    0xe8, 0x61, 0xfe, 0xf9, 0x4e, 0xe6, 0x64, 0x41, 0xda, 0x93, 0xd2, 0xc1, 0x1b, 0x3e, 0x8f, 0x17,
];
pub const FLASH_ATTENTION_MEMORY_PROFILE_IDENTITY_V1: [u8; 32] = [
    0x4d, 0xfe, 0x87, 0x0b, 0xb7, 0x6d, 0xd3, 0x2b, 0x49, 0x14, 0x4e, 0xe7, 0x0e, 0xc4, 0x92, 0x5e,
    0xab, 0x86, 0x77, 0xb7, 0xcb, 0xd1, 0xa1, 0xbf, 0xe9, 0x9f, 0xa2, 0x29, 0x4f, 0x85, 0xfe, 0xc8,
];
pub const FLASH_ATTENTION_MEMORY_KIR_IDENTITY_V1: [u8; 32] = [
    0x48, 0xbd, 0x8d, 0xe9, 0x11, 0xeb, 0xec, 0x55, 0x81, 0x70, 0x97, 0x61, 0xa8, 0x62, 0xc8, 0x89,
    0xf4, 0x70, 0x57, 0xc8, 0x08, 0x6e, 0x0f, 0xec, 0x4c, 0x79, 0xd5, 0xcd, 0xb7, 0x0b, 0xcf, 0xe9,
];
pub const FLASH_ATTENTION_MEMORY_DESCRIPTOR_IDENTITY_V1: [u8; 32] = [
    0x03, 0xae, 0x02, 0xa7, 0xbc, 0xe0, 0x60, 0x43, 0xaa, 0xdf, 0x54, 0x6d, 0x75, 0x04, 0xb2, 0x0c,
    0xf9, 0xa2, 0xb1, 0xc7, 0x72, 0xd9, 0x8c, 0xa1, 0x20, 0xb9, 0xbe, 0x3e, 0xc2, 0xa0, 0xa7, 0x9a,
];
pub const FLASH_ATTENTION_MEMORY_LAUNCH_IDENTITY_V1: [u8; 32] = [
    0x10, 0x0b, 0xc4, 0x9f, 0x34, 0x62, 0x74, 0x85, 0xa9, 0x59, 0xb7, 0x20, 0x1a, 0x23, 0x8b, 0xbf,
    0x84, 0x21, 0xdf, 0x80, 0x0d, 0x7f, 0x10, 0x28, 0xbb, 0xff, 0xf6, 0xbd, 0x8c, 0x51, 0xed, 0xd1,
];
pub const FLASH_ATTENTION_MEMORY_EFFECTS_IDENTITY_V1: [u8; 32] = [
    0xf9, 0x93, 0xef, 0x69, 0x52, 0xda, 0x81, 0xe5, 0x63, 0x10, 0x05, 0x77, 0xb2, 0x39, 0x77, 0x0e,
    0x91, 0x2c, 0xc5, 0xb5, 0x6b, 0xf8, 0x03, 0xbf, 0xce, 0x4e, 0x47, 0x43, 0x6f, 0x72, 0x61, 0x72,
];
pub const FLASH_ATTENTION_MEMORY_VERUS_PROOF_IDENTITY_V1: [u8; 32] = [
    0x29, 0x8a, 0x8d, 0x19, 0x9e, 0xf5, 0x7b, 0xa6, 0x58, 0x1d, 0x82, 0xea, 0x10, 0xb1, 0x1e, 0xe6,
    0x18, 0x9e, 0x53, 0x0c, 0xce, 0x8f, 0x5e, 0x0c, 0x0e, 0x69, 0x97, 0x3e, 0x89, 0x23, 0x8e, 0x6f,
];
pub const FLASH_ATTENTION_MEMORY_ARTIFACT_IDENTITY_V1: [u8; 32] = [
    0xf4, 0xb3, 0xaf, 0x45, 0xa4, 0x81, 0x51, 0xfb, 0x2e, 0x24, 0xfe, 0xa0, 0x04, 0xa7, 0x7d, 0x21,
    0x9f, 0x64, 0x94, 0x4e, 0xa1, 0x55, 0xc2, 0x76, 0x71, 0x0d, 0xe0, 0x5b, 0x25, 0xad, 0x96, 0x51,
];
pub const FLASH_ATTENTION_MEMORY_ANALYZER_PROFILE_IDENTITY_V1: [u8; 32] = [
    0xa4, 0xec, 0x22, 0x4c, 0x4c, 0xd4, 0x22, 0xa7, 0xf5, 0x5a, 0x26, 0xa0, 0xea, 0x7a, 0xc6, 0xf1,
    0x35, 0x0d, 0xe9, 0xb2, 0xff, 0x0a, 0x02, 0xc8, 0xfc, 0x88, 0xce, 0x9b, 0xa1, 0xd2, 0x12, 0xb8,
];
pub const FLASH_ATTENTION_MEMORY_VERUS_TRANSCRIPT_IDENTITY_V1: [u8; 32] = [
    0x6f, 0xd2, 0xd8, 0x17, 0x84, 0xa6, 0x42, 0xbe, 0x8b, 0x2a, 0x6b, 0xf1, 0x98, 0x5f, 0xf9, 0x9c,
    0x02, 0xe4, 0x6d, 0x72, 0x81, 0x6a, 0xa5, 0x29, 0x42, 0x13, 0x02, 0x21, 0x88, 0x73, 0x27, 0x06,
];
pub const FLASH_ATTENTION_MEMORY_VERUS_EXECUTABLE_IDENTITY_V1: [u8; 32] = [
    0xd9, 0x75, 0x01, 0xa8, 0x83, 0x93, 0x1d, 0x1d, 0x17, 0x3b, 0x1b, 0xf4, 0xb6, 0xcf, 0x4d, 0x97,
    0x3f, 0x16, 0xd1, 0x05, 0xdb, 0xcb, 0x46, 0x8e, 0x17, 0x7b, 0x52, 0xb2, 0x33, 0x16, 0x12, 0xd2,
];
pub const FLASH_ATTENTION_MEMORY_VERUS_CLOSURE_IDENTITY_V1: [u8; 32] = [
    0xf0, 0x68, 0x83, 0xe4, 0xce, 0x46, 0x3b, 0xcb, 0x9a, 0x3c, 0x8f, 0x91, 0x10, 0x64, 0xac, 0x85,
    0x05, 0x4c, 0x78, 0x22, 0xdc, 0x33, 0x1d, 0xb1, 0xa7, 0x9f, 0x75, 0xf9, 0xe8, 0x87, 0x8b, 0x01,
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FlashAttentionMemoryBufferV1 {
    Query,
    Key,
    Value,
    Output,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FlashAttentionMemoryEffectKindV1 {
    Read,
    Write,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum FlashAttentionMemoryPhaseV1 {
    InputValidation,
    CausalRecurrence,
    OwnedOutputCommit,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FlashAttentionLogicalAccessV1 {
    pub lane: usize,
    pub query_row: usize,
    pub key_row: Option<usize>,
    pub buffer: FlashAttentionMemoryBufferV1,
    pub element_index: usize,
    pub address_space: u8,
    pub byte_width: u8,
    pub phase: FlashAttentionMemoryPhaseV1,
    pub kind: FlashAttentionMemoryEffectKindV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FlashAttentionMemoryRegionsV1 {
    pub query_base: u64,
    pub key_base: u64,
    pub value_base: u64,
    pub output_base: u64,
    pub query_bytes: u64,
    pub key_bytes: u64,
    pub value_bytes: u64,
    pub output_bytes: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FlashAttentionMemoryIdentitiesV1 {
    pub source: [u8; 32],
    pub profile: [u8; 32],
    pub kernel_ir: [u8; 32],
    pub descriptor: [u8; 32],
    pub launch: [u8; 32],
    pub effects: [u8; 32],
}

impl FlashAttentionMemoryIdentitiesV1 {
    pub const fn exact() -> Self {
        Self {
            source: FLASH_ATTENTION_MEMORY_SOURCE_IDENTITY_V1,
            profile: FLASH_ATTENTION_MEMORY_PROFILE_IDENTITY_V1,
            kernel_ir: FLASH_ATTENTION_MEMORY_KIR_IDENTITY_V1,
            descriptor: FLASH_ATTENTION_MEMORY_DESCRIPTOR_IDENTITY_V1,
            launch: FLASH_ATTENTION_MEMORY_LAUNCH_IDENTITY_V1,
            effects: FLASH_ATTENTION_MEMORY_EFFECTS_IDENTITY_V1,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FlashAttentionMemoryContractErrorV1 {
    Identity,
    Extent,
    Alignment,
    AddressOverflow,
    OutputAliasesInput,
    Lane,
    KeyOutsideCausalPrefix,
    Feature,
    OutputSlot,
    AddressSpace,
    AccessWidth,
    EffectKind,
    EffectOrdering,
    OutputOwnership,
}

impl fmt::Display for FlashAttentionMemoryContractErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid exact FlashAttention memory contract: {self:?}"
        )
    }
}

impl std::error::Error for FlashAttentionMemoryContractErrorV1 {}

/// Inert exhaustive-check result for the complete fixed source model.
///
/// Its `exhaustively_checks_*` methods report checker coverage only. They are
/// intentionally not named `proves_*`; proof claims live on the joined Verus
/// receipt type below.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CheckedFlashAttentionMemoryContractV1 {
    identities: FlashAttentionMemoryIdentitiesV1,
}

/// Measurements emitted by the fail-closed `run-memory-verus.sh` boundary.
///
/// This record authenticates exact inputs and a canonical successful result;
/// it is not an authenticated operating-system runtime-closure observation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FlashAttentionMemoryVerusObservationV1 {
    pub proof_source: [u8; 32],
    pub artifact: [u8; 32],
    pub analyzer_profile: [u8; 32],
    pub verus_executable: [u8; 32],
    pub verus_closure_manifest: [u8; 32],
    pub transcript: [u8; 32],
}

impl FlashAttentionMemoryVerusObservationV1 {
    pub const fn exact() -> Self {
        Self {
            proof_source: FLASH_ATTENTION_MEMORY_VERUS_PROOF_IDENTITY_V1,
            artifact: FLASH_ATTENTION_MEMORY_ARTIFACT_IDENTITY_V1,
            analyzer_profile: FLASH_ATTENTION_MEMORY_ANALYZER_PROFILE_IDENTITY_V1,
            verus_executable: FLASH_ATTENTION_MEMORY_VERUS_EXECUTABLE_IDENTITY_V1,
            verus_closure_manifest: FLASH_ATTENTION_MEMORY_VERUS_CLOSURE_IDENTITY_V1,
            transcript: FLASH_ATTENTION_MEMORY_VERUS_TRANSCRIPT_IDENTITY_V1,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FlashAttentionMemoryVerusJoinErrorV1 {
    ProofSource,
    Artifact,
    AnalyzerProfile,
    VerusExecutable,
    VerusClosure,
    Transcript,
}

impl fmt::Display for FlashAttentionMemoryVerusJoinErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid exact FlashAttention Verus receipt: {self:?}"
        )
    }
}

impl std::error::Error for FlashAttentionMemoryVerusJoinErrorV1 {}

/// Exact source-level proof receipt, joined to the exhaustive source checker.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProvedFlashAttentionMemoryContractV1 {
    checked: CheckedFlashAttentionMemoryContractV1,
    observation: FlashAttentionMemoryVerusObservationV1,
}

impl ProvedFlashAttentionMemoryContractV1 {
    pub const fn checked_source_model(self) -> CheckedFlashAttentionMemoryContractV1 {
        self.checked
    }

    pub const fn observation(self) -> FlashAttentionMemoryVerusObservationV1 {
        self.observation
    }

    pub const fn has_identity_bound_verus_receipt(self) -> bool {
        true
    }

    pub const fn proves_fixed_source_index_bounds_under_contract_preconditions(self) -> bool {
        true
    }

    pub const fn proves_fixed_source_output_disjointness(self) -> bool {
        true
    }

    pub const fn proves_compiler_refinement(self) -> bool {
        false
    }

    pub const fn proves_isa_refinement(self) -> bool {
        false
    }

    pub const fn proves_logical_to_machine_address_refinement(self) -> bool {
        false
    }

    pub const fn proves_machine_memory_safety(self) -> bool {
        false
    }

    pub const fn proves_generalized_race_freedom(self) -> bool {
        false
    }

    pub const fn proves_gpu_execution(self) -> bool {
        false
    }
}

pub fn join_flash_attention_memory_verus_v1(
    checked: CheckedFlashAttentionMemoryContractV1,
    observation: FlashAttentionMemoryVerusObservationV1,
) -> Result<ProvedFlashAttentionMemoryContractV1, FlashAttentionMemoryVerusJoinErrorV1> {
    let exact = FlashAttentionMemoryVerusObservationV1::exact();
    if observation.proof_source != exact.proof_source {
        return Err(FlashAttentionMemoryVerusJoinErrorV1::ProofSource);
    }
    if observation.artifact != exact.artifact {
        return Err(FlashAttentionMemoryVerusJoinErrorV1::Artifact);
    }
    if observation.analyzer_profile != exact.analyzer_profile {
        return Err(FlashAttentionMemoryVerusJoinErrorV1::AnalyzerProfile);
    }
    if observation.verus_executable != exact.verus_executable {
        return Err(FlashAttentionMemoryVerusJoinErrorV1::VerusExecutable);
    }
    if observation.verus_closure_manifest != exact.verus_closure_manifest {
        return Err(FlashAttentionMemoryVerusJoinErrorV1::VerusClosure);
    }
    if observation.transcript != exact.transcript {
        return Err(FlashAttentionMemoryVerusJoinErrorV1::Transcript);
    }
    Ok(ProvedFlashAttentionMemoryContractV1 {
        checked,
        observation,
    })
}

impl CheckedFlashAttentionMemoryContractV1 {
    pub const fn identities(self) -> FlashAttentionMemoryIdentitiesV1 {
        self.identities
    }

    pub const fn exhaustively_checks_fixed_source_index_bounds(self) -> bool {
        true
    }

    pub const fn exhaustively_checks_fixed_source_output_disjointness(self) -> bool {
        true
    }

    pub const fn has_identity_bound_verus_receipt(self) -> bool {
        false
    }

    pub const fn proves_compiler_refinement(self) -> bool {
        false
    }

    pub const fn proves_isa_refinement(self) -> bool {
        false
    }

    pub const fn proves_logical_to_machine_address_refinement(self) -> bool {
        false
    }

    pub const fn proves_machine_memory_safety(self) -> bool {
        false
    }

    pub const fn proves_generalized_race_freedom(self) -> bool {
        false
    }

    pub const fn proves_gpu_execution(self) -> bool {
        false
    }
}

pub fn check_flash_attention_memory_contract_v1(
    identities: FlashAttentionMemoryIdentitiesV1,
    regions: FlashAttentionMemoryRegionsV1,
) -> Result<CheckedFlashAttentionMemoryContractV1, FlashAttentionMemoryContractErrorV1> {
    if identities != FlashAttentionMemoryIdentitiesV1::exact() {
        return Err(FlashAttentionMemoryContractErrorV1::Identity);
    }
    validate_regions(regions)?;
    for lane in 0..FLASH_ATTENTION_MEMORY_LANES_V1 {
        for index in 0..FLASH_ATTENTION_MEMORY_ELEMENTS_V1 {
            for buffer in [
                FlashAttentionMemoryBufferV1::Query,
                FlashAttentionMemoryBufferV1::Key,
                FlashAttentionMemoryBufferV1::Value,
            ] {
                validate_access(FlashAttentionLogicalAccessV1 {
                    lane,
                    query_row: lane / 8,
                    key_row: None,
                    buffer,
                    element_index: index,
                    address_space: FLASH_ATTENTION_MEMORY_GLOBAL_ADDRESS_SPACE_V1,
                    byte_width: FLASH_ATTENTION_MEMORY_ACCESS_WIDTH_V1,
                    phase: FlashAttentionMemoryPhaseV1::InputValidation,
                    kind: FlashAttentionMemoryEffectKindV1::Read,
                })?;
            }
        }
        let query_row = lane / 8;
        let first_output = lane * 2;
        for key_row in 0..=query_row {
            for feature in 0..FLASH_ATTENTION_MEMORY_DIMENSION_V1 {
                for (buffer, row) in [
                    (FlashAttentionMemoryBufferV1::Query, query_row),
                    (FlashAttentionMemoryBufferV1::Key, key_row),
                ] {
                    validate_access(FlashAttentionLogicalAccessV1 {
                        lane,
                        query_row,
                        key_row: Some(key_row),
                        buffer,
                        element_index: row * FLASH_ATTENTION_MEMORY_DIMENSION_V1 + feature,
                        address_space: FLASH_ATTENTION_MEMORY_GLOBAL_ADDRESS_SPACE_V1,
                        byte_width: FLASH_ATTENTION_MEMORY_ACCESS_WIDTH_V1,
                        phase: FlashAttentionMemoryPhaseV1::CausalRecurrence,
                        kind: FlashAttentionMemoryEffectKindV1::Read,
                    })?;
                }
            }
            for slot in 0..2 {
                validate_access(FlashAttentionLogicalAccessV1 {
                    lane,
                    query_row,
                    key_row: Some(key_row),
                    buffer: FlashAttentionMemoryBufferV1::Value,
                    element_index: key_row * FLASH_ATTENTION_MEMORY_DIMENSION_V1
                        + first_output % FLASH_ATTENTION_MEMORY_DIMENSION_V1
                        + slot,
                    address_space: FLASH_ATTENTION_MEMORY_GLOBAL_ADDRESS_SPACE_V1,
                    byte_width: FLASH_ATTENTION_MEMORY_ACCESS_WIDTH_V1,
                    phase: FlashAttentionMemoryPhaseV1::CausalRecurrence,
                    kind: FlashAttentionMemoryEffectKindV1::Read,
                })?;
            }
        }
        for slot in 0..2 {
            validate_access(FlashAttentionLogicalAccessV1 {
                lane,
                query_row,
                key_row: None,
                buffer: FlashAttentionMemoryBufferV1::Output,
                element_index: first_output + slot,
                address_space: FLASH_ATTENTION_MEMORY_GLOBAL_ADDRESS_SPACE_V1,
                byte_width: FLASH_ATTENTION_MEMORY_ACCESS_WIDTH_V1,
                phase: FlashAttentionMemoryPhaseV1::OwnedOutputCommit,
                kind: FlashAttentionMemoryEffectKindV1::Write,
            })?;
        }
    }
    for left_lane in 0..FLASH_ATTENTION_MEMORY_LANES_V1 {
        for right_lane in 0..FLASH_ATTENTION_MEMORY_LANES_V1 {
            for left_slot in 0..2 {
                for right_slot in 0..2 {
                    if (left_lane, left_slot) != (right_lane, right_slot)
                        && left_lane * 2 + left_slot == right_lane * 2 + right_slot
                    {
                        return Err(FlashAttentionMemoryContractErrorV1::OutputOwnership);
                    }
                }
            }
        }
    }
    Ok(CheckedFlashAttentionMemoryContractV1 { identities })
}

pub fn validate_flash_attention_logical_access_v1(
    access: FlashAttentionLogicalAccessV1,
) -> Result<(), FlashAttentionMemoryContractErrorV1> {
    validate_access(access)
}

fn validate_regions(
    regions: FlashAttentionMemoryRegionsV1,
) -> Result<(), FlashAttentionMemoryContractErrorV1> {
    if [
        regions.query_bytes,
        regions.key_bytes,
        regions.value_bytes,
        regions.output_bytes,
    ] != [FLASH_ATTENTION_MEMORY_REGION_BYTES_V1; 4]
    {
        return Err(FlashAttentionMemoryContractErrorV1::Extent);
    }
    let bases = [
        regions.query_base,
        regions.key_base,
        regions.value_base,
        regions.output_base,
    ];
    if bases.into_iter().any(|base| base % 4 != 0) {
        return Err(FlashAttentionMemoryContractErrorV1::Alignment);
    }
    let ends = [
        regions.query_base.checked_add(regions.query_bytes),
        regions.key_base.checked_add(regions.key_bytes),
        regions.value_base.checked_add(regions.value_bytes),
        regions.output_base.checked_add(regions.output_bytes),
    ];
    if ends.iter().any(Option::is_none) {
        return Err(FlashAttentionMemoryContractErrorV1::AddressOverflow);
    }
    let output_end = ends[3].expect("checked above");
    for (input_base, input_end) in bases[..3].iter().zip(ends[..3].iter()) {
        if regions.output_base < input_end.expect("checked above") && *input_base < output_end {
            return Err(FlashAttentionMemoryContractErrorV1::OutputAliasesInput);
        }
    }
    Ok(())
}

fn validate_access(
    access: FlashAttentionLogicalAccessV1,
) -> Result<(), FlashAttentionMemoryContractErrorV1> {
    if access.lane >= FLASH_ATTENTION_MEMORY_LANES_V1 {
        return Err(FlashAttentionMemoryContractErrorV1::Lane);
    }
    if access.query_row != access.lane / 8 || access.query_row >= FLASH_ATTENTION_MEMORY_SEQUENCE_V1
    {
        return Err(FlashAttentionMemoryContractErrorV1::OutputOwnership);
    }
    if access.address_space != FLASH_ATTENTION_MEMORY_GLOBAL_ADDRESS_SPACE_V1 {
        return Err(FlashAttentionMemoryContractErrorV1::AddressSpace);
    }
    if access.byte_width != FLASH_ATTENTION_MEMORY_ACCESS_WIDTH_V1 {
        return Err(FlashAttentionMemoryContractErrorV1::AccessWidth);
    }
    if access.element_index >= FLASH_ATTENTION_MEMORY_ELEMENTS_V1 {
        return Err(FlashAttentionMemoryContractErrorV1::Extent);
    }
    match (access.buffer, access.kind, access.phase) {
        (
            FlashAttentionMemoryBufferV1::Output,
            FlashAttentionMemoryEffectKindV1::Write,
            FlashAttentionMemoryPhaseV1::OwnedOutputCommit,
        ) => {
            if access.key_row.is_some()
                || (access.element_index != access.lane * 2
                    && access.element_index != access.lane * 2 + 1)
            {
                return Err(FlashAttentionMemoryContractErrorV1::OutputOwnership);
            }
        }
        (
            FlashAttentionMemoryBufferV1::Query
            | FlashAttentionMemoryBufferV1::Key
            | FlashAttentionMemoryBufferV1::Value,
            FlashAttentionMemoryEffectKindV1::Read,
            FlashAttentionMemoryPhaseV1::InputValidation,
        ) => {
            if access.key_row.is_some() {
                return Err(FlashAttentionMemoryContractErrorV1::EffectOrdering);
            }
        }
        (
            FlashAttentionMemoryBufferV1::Query
            | FlashAttentionMemoryBufferV1::Key
            | FlashAttentionMemoryBufferV1::Value,
            FlashAttentionMemoryEffectKindV1::Read,
            FlashAttentionMemoryPhaseV1::CausalRecurrence,
        ) => {
            if access.key_row.is_none_or(|key| key > access.query_row) {
                return Err(FlashAttentionMemoryContractErrorV1::KeyOutsideCausalPrefix);
            }
        }
        (FlashAttentionMemoryBufferV1::Output, _, _) => {
            return Err(FlashAttentionMemoryContractErrorV1::EffectKind);
        }
        (_, FlashAttentionMemoryEffectKindV1::Write, _) => {
            return Err(FlashAttentionMemoryContractErrorV1::EffectKind);
        }
        _ => return Err(FlashAttentionMemoryContractErrorV1::EffectOrdering),
    }
    Ok(())
}
