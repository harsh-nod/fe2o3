//! Closed inert authored global-copy vocabulary for allocated KIR21.
//! FunctionBody remains the sole executable. These fields carry no source authority.
use crate::{
    Gfx942PhysicalEntryBlockContractVNext, Gfx942PhysicalEntryBranchEncodingVNext,
    Gfx942PhysicalEntryOriginVNext, Gfx942PhysicalEntrySourceSiteVNext, ValueId,
};
#[path = "gfx942_physical_global_copy_instruction_v21.rs"]
mod instruction;
pub use instruction::{Gfx942PhysicalGlobalCopyInstructionV1, Gfx942PhysicalGlobalCopyOpcodeV1};

pub const AMDGPU_GFX942_PHYSICAL_GLOBAL_COPY_CAPABILITY_NAMESPACE_V21: &str = "amd.gfx942";
pub const AMDGPU_GFX942_PHYSICAL_GLOBAL_COPY_CAPABILITY_NAME_V21: &str =
    "physical_global_copy_u32_v1";
pub const GFX942_PHYSICAL_GLOBAL_COPY_MAX_BLOCKS_V21: usize = 1;
pub const GFX942_PHYSICAL_GLOBAL_COPY_MAX_INSTRUCTIONS_V21: usize = 32;
pub const GFX942_PHYSICAL_GLOBAL_COPY_MAX_SOURCE_SITES_V21: usize = 34;
pub const GFX942_PHYSICAL_GLOBAL_COPY_MAX_DEFINITIONS_V21: usize = 160;
pub const GFX942_PHYSICAL_GLOBAL_COPY_DECLARATION_BYTES_V21: usize = 461;
pub const GFX942_PHYSICAL_GLOBAL_COPY_STEP_BYTES_V21: usize = 80;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Gfx942PhysicalGlobalCopyDeclarationV1 {
    pub origin: Gfx942PhysicalEntryOriginVNext,
    pub begin_site: Gfx942PhysicalEntrySourceSiteVNext,
    /// Actual logical input ReadOnly slice, then output ReadWrite slice.
    pub parameters: [ValueId; 2],
    pub native_instruction_count: u8,
    pub workgroup: [u32; 3],
    pub maximum_workgroups: [u32; 3],
    pub block: Gfx942PhysicalEntryBlockContractVNext,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Gfx942PhysicalGlobalCopyStepV1 {
    pub site: Gfx942PhysicalEntrySourceSiteVNext,
    pub native_ordinal: u8,
    pub instruction: Gfx942PhysicalGlobalCopyInstructionV1,
    pub operands: [Option<ValueId>; 5],
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Gfx942PhysicalGlobalCopyShapeErrorV21 {
    Origin,
    Site,
    Count,
    Parameter,
    Block,
    Ordinal,
    Instruction,
    OperandArity,
}
pub(crate) fn global_copy_site_v21(site: Gfx942PhysicalEntrySourceSiteVNext) -> bool {
    site.is_complete()
        && usize::from(site.occurrence) < GFX942_PHYSICAL_GLOBAL_COPY_MAX_SOURCE_SITES_V21
}
impl Gfx942PhysicalGlobalCopyDeclarationV1 {
    pub fn validate_shape(&self) -> Result<(), Gfx942PhysicalGlobalCopyShapeErrorV21> {
        use Gfx942PhysicalGlobalCopyShapeErrorV21 as E;
        if !self.origin.is_complete() {
            return Err(E::Origin);
        }
        if !global_copy_site_v21(self.begin_site) || self.begin_site.occurrence != 0 {
            return Err(E::Site);
        }
        if !(1..=32).contains(&self.native_instruction_count)
            || self.workgroup != [64, 1, 1]
            || self.maximum_workgroups != [2, 1, 1]
        {
            return Err(E::Count);
        }
        if self.parameters[0] == self.parameters[1] {
            return Err(E::Parameter);
        }
        if self.block.label != 0
            || !global_copy_site_v21(self.block.label_site)
            || !global_copy_site_v21(self.block.terminator_site)
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
impl Gfx942PhysicalGlobalCopyStepV1 {
    pub fn validate_shape(&self) -> Result<(), Gfx942PhysicalGlobalCopyShapeErrorV21> {
        use Gfx942PhysicalGlobalCopyShapeErrorV21 as E;
        if !global_copy_site_v21(self.site) {
            return Err(E::Site);
        }
        if usize::from(self.native_ordinal) >= GFX942_PHYSICAL_GLOBAL_COPY_MAX_INSTRUCTIONS_V21 {
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
