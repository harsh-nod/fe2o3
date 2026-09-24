//! Closed inert physical-entry vocabulary for allocated KIR20.
//! Source observations are not authentication; FunctionBody is the only CFG.
use crate::{ScalarType, ValueId};

#[path = "gfx942_physical_entry_instruction_v20.rs"]
mod instruction;
pub use instruction::{Gfx942PhysicalEntryInstructionVNext, Gfx942PhysicalEntryOpcodeV20};

pub const AMDGPU_GFX942_PHYSICAL_ENTRY_CAPABILITY_NAMESPACE_V20: &str = "amd.gfx942";
pub const AMDGPU_GFX942_PHYSICAL_ENTRY_CAPABILITY_NAME_V20: &str = "physical_entry_u32_out_v1";
pub const GFX942_PHYSICAL_ENTRY_MAX_BLOCKS_V20: usize = 4;
pub const GFX942_PHYSICAL_ENTRY_MAX_INSTRUCTIONS_V20: usize = 64;
pub const GFX942_PHYSICAL_ENTRY_MAX_SOURCE_SITES_V20: usize = 73;
pub const GFX942_PHYSICAL_ENTRY_MAX_DEFINITIONS_V20: usize = 768;
pub const GFX942_PHYSICAL_ENTRY_DECLARATION_BYTES_V20: usize = 756;
pub const GFX942_PHYSICAL_ENTRY_STEP_BYTES_V20: usize = 85;

/// Physical unit, not observed hardware contents or runtime pointer authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Gfx942PhysicalEntryRegisterV20 {
    Sgpr(u8),
    Vgpr(u8),
    Scc,
    Vcc,
    Exec,
}
impl Gfx942PhysicalEntryRegisterV20 {
    pub const fn scalar_type(self) -> ScalarType {
        match self {
            Self::Sgpr(_) | Self::Vgpr(_) => ScalarType::U32,
            Self::Scc => ScalarType::Bool,
            Self::Vcc | Self::Exec => ScalarType::U64,
        }
    }
    pub const fn state_index(self) -> Option<usize> {
        match self {
            Self::Sgpr(index) if index < 64 => Some(index as usize),
            Self::Vgpr(index) if index < 64 => Some(64 + index as usize),
            Self::Scc => Some(128),
            Self::Vcc => Some(129),
            Self::Exec => Some(130),
            _ => None,
        }
    }
    pub const fn from_state_index(index: usize) -> Option<Self> {
        match index {
            0..64 => Some(Self::Sgpr(index as u8)),
            64..128 => Some(Self::Vgpr((index - 64) as u8)),
            128 => Some(Self::Scc),
            129 => Some(Self::Vcc),
            130 => Some(Self::Exec),
            _ => None,
        }
    }
}
pub const GFX942_PHYSICAL_ENTRY_REGISTERS_V20: [Gfx942PhysicalEntryRegisterV20; 5] = [
    Gfx942PhysicalEntryRegisterV20::Sgpr(0),
    Gfx942PhysicalEntryRegisterV20::Sgpr(1),
    Gfx942PhysicalEntryRegisterV20::Sgpr(2),
    Gfx942PhysicalEntryRegisterV20::Vgpr(0),
    Gfx942PhysicalEntryRegisterV20::Exec,
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Gfx942PhysicalEntryOriginVNext {
    pub root_axes: [[u8; 32]; 5],
    pub mir_body: [u8; 32],
    pub source_signature: [u8; 32],
    pub rustc_fn_abi: [u8; 32],
    pub frontend_bytes_sha256: [u8; 32],
}
impl Gfx942PhysicalEntryOriginVNext {
    pub fn is_complete(&self) -> bool {
        self.root_axes
            .iter()
            .chain([
                &self.mir_body,
                &self.source_signature,
                &self.rustc_fn_abi,
                &self.frontend_bytes_sha256,
            ])
            .all(|value| *value != [0; 32])
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Gfx942PhysicalEntrySourceSiteVNext {
    pub occurrence: u8,
    pub raw_block: u32,
    pub semantic_block_index: u32,
    pub semantic_block_identity: [u8; 32],
    pub semantic_callable_index: u32,
}
impl Gfx942PhysicalEntrySourceSiteVNext {
    pub const ZERO: Self = Self {
        occurrence: 0,
        raw_block: 0,
        semantic_block_index: 0,
        semantic_block_identity: [0; 32],
        semantic_callable_index: 0,
    };
    pub fn is_complete(&self) -> bool {
        usize::from(self.occurrence) < GFX942_PHYSICAL_ENTRY_MAX_SOURCE_SITES_V20
            && self.semantic_block_identity != [0; 32]
    }
}

/// Instruction selection only: actual targets/conditions belong to the CFG.
#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Gfx942PhysicalEntryBranchEncodingVNext {
    Inactive = 0,
    Scc1 = 1,
    Jump = 2,
    Fallthrough = 3,
    Endpgm0 = 4,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Gfx942PhysicalEntryBlockContractVNext {
    pub label: u8,
    pub label_site: Gfx942PhysicalEntrySourceSiteVNext,
    pub encoding: Gfx942PhysicalEntryBranchEncodingVNext,
    pub terminator_site: Gfx942PhysicalEntrySourceSiteVNext,
    pub native_ordinal: Option<u8>,
}
impl Gfx942PhysicalEntryBlockContractVNext {
    pub const ZERO: Self = Self {
        label: 0,
        label_site: Gfx942PhysicalEntrySourceSiteVNext::ZERO,
        encoding: Gfx942PhysicalEntryBranchEncodingVNext::Inactive,
        terminator_site: Gfx942PhysicalEntrySourceSiteVNext::ZERO,
        native_ordinal: None,
    };
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Gfx942PhysicalEntryDeclarationVNext {
    pub origin: Gfx942PhysicalEntryOriginVNext,
    pub begin_site: Gfx942PhysicalEntrySourceSiteVNext,
    pub parameters: [ValueId; 5],
    pub block_count: u8,
    pub native_instruction_count: u8,
    pub workgroup: [u32; 3],
    pub maximum_workgroups: [u32; 3],
    pub blocks: [Gfx942PhysicalEntryBlockContractVNext; 4],
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Gfx942PhysicalEntryStepVNext {
    pub site: Gfx942PhysicalEntrySourceSiteVNext,
    pub native_ordinal: u8,
    pub instruction: Gfx942PhysicalEntryInstructionVNext,
    pub operands: [Option<ValueId>; 6],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Gfx942PhysicalEntryShapeErrorV20 {
    Origin,
    Site,
    Count,
    Parameter,
    Block,
    Ordinal,
    Instruction,
    OperandArity,
}
impl Gfx942PhysicalEntryDeclarationVNext {
    pub fn validate_shape(&self) -> Result<(), Gfx942PhysicalEntryShapeErrorV20> {
        use Gfx942PhysicalEntryBranchEncodingVNext as B;
        use Gfx942PhysicalEntryShapeErrorV20 as E;
        if !self.origin.is_complete() {
            return Err(E::Origin);
        }
        if !self.begin_site.is_complete() || self.begin_site.occurrence != 0 {
            return Err(E::Site);
        }
        if !matches!(self.block_count, 1 | 4)
            || !(1..=64).contains(&self.native_instruction_count)
            || self.workgroup != [64, 1, 1]
            || self.maximum_workgroups != [2, 1, 1]
        {
            return Err(E::Count);
        }
        for (index, value) in self.parameters.iter().enumerate() {
            if self.parameters[..index].contains(value) {
                return Err(E::Parameter);
            }
        }
        let count = usize::from(self.block_count);
        for (index, block) in self.blocks.iter().enumerate() {
            if index >= count {
                if *block != Gfx942PhysicalEntryBlockContractVNext::ZERO {
                    return Err(E::Block);
                }
                continue;
            }
            if !block.label_site.is_complete()
                || !block.terminator_site.is_complete()
                || self.blocks[..index]
                    .iter()
                    .any(|other| other.label == block.label)
                || block.encoding == B::Inactive
            {
                return Err(E::Block);
            }
            match (block.encoding, block.native_ordinal) {
                (B::Fallthrough, None) => {}
                (B::Scc1 | B::Jump | B::Endpgm0, Some(value))
                    if value < self.native_instruction_count => {}
                _ => return Err(E::Ordinal),
            }
        }
        Ok(())
    }
}
impl Gfx942PhysicalEntryStepVNext {
    pub fn validate_shape(&self) -> Result<(), Gfx942PhysicalEntryShapeErrorV20> {
        use Gfx942PhysicalEntryShapeErrorV20 as E;
        if !self.site.is_complete() {
            return Err(E::Site);
        }
        if self.native_ordinal >= 64 {
            return Err(E::Ordinal);
        }
        self.instruction.validate_shape()?;
        if self.instruction.opcode.is_control() {
            return Err(E::Instruction);
        }
        let expected = self.instruction.operand_registers();
        if self
            .operands
            .iter()
            .zip(expected)
            .any(|(value, register)| value.is_some() != register.is_some())
        {
            return Err(E::OperandArity);
        }
        Ok(())
    }
}
