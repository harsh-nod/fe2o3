//! Exact instruction-word, operand and decoded-effect validation for the closed recurrence profile.
//!
//! These structural checks do not establish numerical semantics or launch authority.

use super::Gfx942ScalarF32RecurrenceStepAnalysisErrorV1;
use crate::{
    Gfx942InstructionRegisterFactsV1, Gfx942RegisterUnitV1, PhysicalMachineBranchKindV1,
    PhysicalMachineInstructionTraceV1, PhysicalMachineMemoryAccessV1,
};

pub(super) struct ArithmeticShapeV1 {
    pub(super) destination: Gfx942RegisterUnitV1,
    pub(super) sources: Vec<Gfx942RegisterUnitV1>,
}

pub(super) fn arithmetic_shape(
    instruction: &PhysicalMachineInstructionTraceV1,
) -> Result<ArithmeticShapeV1, Gfx942ScalarF32RecurrenceStepAnalysisErrorV1> {
    let invalid = || Gfx942ScalarF32RecurrenceStepAnalysisErrorV1::InvalidArithmeticInstruction {
        offset: instruction.instruction_offset(),
    };
    let opcode = match instruction.opcode() {
        "V_MUL_F32_e32_vi" => 5,
        "V_ADD_F32_e32_vi" => 1,
        _ => return Err(invalid()),
    };
    let encoding: [u8; 4] = instruction.encoding().try_into().map_err(|_| invalid())?;
    let word = u32::from_le_bytes(encoding);
    // CDNA3 ISA 13.3.1; LLVM 22.1.8 VOP2Instructions.td (VOP2e and VI opcodes).
    // SRC0 must encode a VGPR, excluding SGPRs, literals, SDWA and DPP extensions.
    // WorkerMachineEffect.cpp sorts MC implicit register names (EXEC before MODE).
    // Its six serialized flags are zero here; isConvergent and FP exceptions are
    // not serialized flags and are not established by this structural check.
    if word >> 25 != opcode
        || word & 0x100 == 0
        || instruction.branch_kind() != PhysicalMachineBranchKindV1::None
        || instruction.branch_target().is_some()
        || instruction.flags().bits() != 0
        || instruction.memory_access() != PhysicalMachineMemoryAccessV1::None
        || instruction
            .operands()
            .iter()
            .any(|operand| operand.tied_to().is_some())
        || !instruction.implicit_definitions().is_empty()
        || !instruction
            .implicit_uses()
            .iter()
            .map(String::as_str)
            .eq(["EXEC", "MODE"])
    {
        return Err(invalid());
    }
    let facts = Gfx942InstructionRegisterFactsV1::derive(instruction)?;
    if facts.explicit_definition_count() != 1 || facts.operand_aliases().len() != 3 {
        return Err(
            Gfx942ScalarF32RecurrenceStepAnalysisErrorV1::InvalidArithmeticInstruction {
                offset: instruction.instruction_offset(),
            },
        );
    }
    let destination = single_vgpr(facts.operand_aliases()[0].as_ref()).ok_or(
        Gfx942ScalarF32RecurrenceStepAnalysisErrorV1::InvalidArithmeticInstruction {
            offset: instruction.instruction_offset(),
        },
    )?;
    let sources = facts.operand_aliases()[1..]
        .iter()
        .map(|alias| single_vgpr(alias.as_ref()))
        .collect::<Option<Vec<_>>>()
        .ok_or(
            Gfx942ScalarF32RecurrenceStepAnalysisErrorV1::InvalidArithmeticInstruction {
                offset: instruction.instruction_offset(),
            },
        )?;
    if destination != Gfx942RegisterUnitV1::Vgpr(((word >> 17) & 0xff) as u16)
        || sources
            != [
                Gfx942RegisterUnitV1::Vgpr((word & 0xff) as u16),
                Gfx942RegisterUnitV1::Vgpr(((word >> 9) & 0xff) as u16),
            ]
    {
        return Err(invalid());
    }
    Ok(ArithmeticShapeV1 {
        destination,
        sources,
    })
}

fn single_vgpr(alias: Option<&crate::Gfx942RegisterAliasV1>) -> Option<Gfx942RegisterUnitV1> {
    let [unit @ Gfx942RegisterUnitV1::Vgpr(_)] = alias?.units() else {
        return None;
    };
    Some(*unit)
}

pub(super) fn admitted_register_copy_source(
    instruction: &PhysicalMachineInstructionTraceV1,
    expected_destination: Gfx942RegisterUnitV1,
) -> Result<Option<Gfx942RegisterUnitV1>, Gfx942ScalarF32RecurrenceStepAnalysisErrorV1> {
    if !matches!(instruction.opcode(), "V_MOV_B32_e32" | "V_MOV_B32_e32_vi")
        || instruction.branch_kind() != PhysicalMachineBranchKindV1::None
        || instruction.branch_target().is_some()
        || instruction.flags().bits() != 0
        || instruction.memory_access() != PhysicalMachineMemoryAccessV1::None
        || instruction
            .operands()
            .iter()
            .any(|operand| operand.tied_to().is_some())
        || !instruction.implicit_definitions().is_empty()
        || !instruction
            .implicit_uses()
            .iter()
            .map(String::as_str)
            .eq(["EXEC"])
    {
        return Ok(None);
    }
    let Ok(encoding) = <[u8; 4]>::try_from(instruction.encoding()) else {
        return Ok(None);
    };
    let word = u32::from_le_bytes(encoding);
    // CDNA3 ISA 13.3.2; LLVM 22.1.8 VOP1Instructions.td. The native MC spelling
    // has the _vi suffix; both names denote only this unmodified VGPR copy.
    if word & 0xfe01_fe00 != 0x7e00_0200 || word & 0x100 == 0 {
        return Ok(None);
    }
    let facts = Gfx942InstructionRegisterFactsV1::derive(instruction)?;
    if facts.explicit_definition_count() != 1 || facts.operand_aliases().len() != 2 {
        return Ok(None);
    }
    let Some(destination) = single_vgpr(facts.operand_aliases()[0].as_ref()) else {
        return Ok(None);
    };
    let Some(source) = single_vgpr(facts.operand_aliases()[1].as_ref()) else {
        return Ok(None);
    };
    Ok((destination == expected_destination
        && destination == Gfx942RegisterUnitV1::Vgpr(((word >> 17) & 0xff) as u16)
        && source == Gfx942RegisterUnitV1::Vgpr((word & 0xff) as u16))
    .then_some(source))
}
