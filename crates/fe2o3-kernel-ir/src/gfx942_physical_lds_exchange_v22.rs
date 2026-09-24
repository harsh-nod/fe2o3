//! Closed inert authored LDS-exchange vocabulary for allocated KIR22.
//! FunctionBody remains the sole executable. These fields carry no source authority.
use crate::{
    Gfx942PhysicalEntryBlockContractVNext, Gfx942PhysicalEntryBranchEncodingVNext,
    Gfx942PhysicalEntryOriginVNext, Gfx942PhysicalEntrySourceSiteVNext, ValueId,
};
#[path = "gfx942_physical_lds_exchange_instruction_v22.rs"]
mod instruction;
pub use instruction::{Gfx942PhysicalLdsExchangeInstructionV1, Gfx942PhysicalLdsExchangeOpcodeV1};

pub const AMDGPU_GFX942_PHYSICAL_LDS_EXCHANGE_CAPABILITY_NAMESPACE_V22: &str = "amd.gfx942";
pub const AMDGPU_GFX942_PHYSICAL_LDS_EXCHANGE_CAPABILITY_NAME_V22: &str =
    "physical_lds_exchange_u32_v1";
pub const GFX942_PHYSICAL_LDS_EXCHANGE_MAX_BLOCKS_V22: usize = 1;
pub const GFX942_PHYSICAL_LDS_EXCHANGE_MAX_INSTRUCTIONS_V22: usize = 40;
pub const GFX942_PHYSICAL_LDS_EXCHANGE_MAX_SOURCE_SITES_V22: usize = 42;
pub const GFX942_PHYSICAL_LDS_EXCHANGE_MAX_DEFINITIONS_V22: usize = 192;
pub const GFX942_PHYSICAL_LDS_EXCHANGE_DECLARATION_BYTES_V22: usize = 474;
pub const GFX942_PHYSICAL_LDS_EXCHANGE_STEP_BYTES_V22: usize = 80;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Gfx942PhysicalLdsExchangeFrameV1 {
    pub byte_offset: u32,
    pub byte_length: u32,
    pub alignment: u32,
    pub publication_epoch: u8,
}
impl Gfx942PhysicalLdsExchangeFrameV1 {
    pub fn validate_shape(&self) -> Result<(), Gfx942PhysicalLdsExchangeShapeErrorV22> {
        if self.byte_offset == 0
            && self.byte_length == 512
            && self.alignment == 4
            && self.publication_epoch == 1
        {
            Ok(())
        } else {
            Err(Gfx942PhysicalLdsExchangeShapeErrorV22::LdsFrame)
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Gfx942PhysicalLdsExchangeDeclarationV1 {
    pub origin: Gfx942PhysicalEntryOriginVNext,
    pub begin_site: Gfx942PhysicalEntrySourceSiteVNext,
    /// Actual logical input ReadOnly slice, then output ReadWrite slice.
    pub parameters: [ValueId; 2],
    pub native_instruction_count: u8,
    pub workgroup: [u32; 3],
    pub maximum_workgroups: [u32; 3],
    pub lds_frame: Gfx942PhysicalLdsExchangeFrameV1,
    pub block: Gfx942PhysicalEntryBlockContractVNext,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Gfx942PhysicalLdsExchangeStepV1 {
    pub site: Gfx942PhysicalEntrySourceSiteVNext,
    pub native_ordinal: u8,
    pub instruction: Gfx942PhysicalLdsExchangeInstructionV1,
    pub operands: [Option<ValueId>; 5],
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Gfx942PhysicalLdsExchangeShapeErrorV22 {
    Origin,
    Site,
    Count,
    Parameter,
    Block,
    Ordinal,
    Instruction,
    OperandArity,
    LdsFrame,
}
pub(crate) fn lds_exchange_site_v22(site: Gfx942PhysicalEntrySourceSiteVNext) -> bool {
    site.is_complete()
        && usize::from(site.occurrence) < GFX942_PHYSICAL_LDS_EXCHANGE_MAX_SOURCE_SITES_V22
}
impl Gfx942PhysicalLdsExchangeDeclarationV1 {
    pub fn validate_shape(&self) -> Result<(), Gfx942PhysicalLdsExchangeShapeErrorV22> {
        use Gfx942PhysicalLdsExchangeShapeErrorV22 as E;
        if !self.origin.is_complete() {
            return Err(E::Origin);
        }
        if !lds_exchange_site_v22(self.begin_site) || self.begin_site.occurrence != 0 {
            return Err(E::Site);
        }
        if !(1..=40).contains(&self.native_instruction_count)
            || self.workgroup != [128, 1, 1]
            || self.maximum_workgroups != [1, 1, 1]
        {
            return Err(E::Count);
        }
        self.lds_frame.validate_shape()?;
        if self.parameters[0] == self.parameters[1] {
            return Err(E::Parameter);
        }
        if self.block.label != 0
            || !lds_exchange_site_v22(self.block.label_site)
            || !lds_exchange_site_v22(self.block.terminator_site)
            || self.block.encoding != Gfx942PhysicalEntryBranchEncodingVNext::Endpgm0
        {
            return Err(E::Block);
        }
        if self.block.native_ordinal != Some(self.native_instruction_count - 1) {
            return Err(E::Ordinal);
        }
        Ok(())
    }
}
impl Gfx942PhysicalLdsExchangeStepV1 {
    pub fn validate_shape(&self) -> Result<(), Gfx942PhysicalLdsExchangeShapeErrorV22> {
        use Gfx942PhysicalLdsExchangeShapeErrorV22 as E;
        if !lds_exchange_site_v22(self.site) {
            return Err(E::Site);
        }
        if usize::from(self.native_ordinal) >= GFX942_PHYSICAL_LDS_EXCHANGE_MAX_INSTRUCTIONS_V22 {
            return Err(E::Ordinal);
        }
        self.instruction.validate_shape()?;
        if self.instruction.opcode.is_control() {
            return Err(E::Instruction);
        }
        if self
            .operands
            .iter()
            .zip(self.instruction.operand_registers())
            .any(|(value, register)| value.is_some() != register.is_some())
        {
            return Err(E::OperandArity);
        }
        Ok(())
    }
}
