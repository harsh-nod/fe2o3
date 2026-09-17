//! Conditional, bounded execution of gfx942 scalar mask/control and address-carry instructions.
//!
//! Inputs are the existing exact-byte machine trace and explicit register live-ins, not a
//! claimed kernel-entry state. All instructions in the selected interval are checked, including
//! paths not taken in this observation. The exclusive stop offset must be another instruction
//! in the same function; that instruction is not executed. Memory, other numeric instructions,
//! termination, and source-to-machine refinement are outside this component.
//!
//! ISA: AMD Instinct MI300/CDNA3 reference, sections 12.1, 12.3, 12.5 and 13.1.
//! https://www.amd.com/content/dam/amd/en/documents/instinct-tech-docs/instruction-set-architectures/amd-instinct-mi300-cdna3-instruction-set-architecture.pdf
//! MC operand/effect forms follow LLVM 22.1.8 SOPInstructions.td. Only four-byte encodings,
//! ordinary aligned SGPR pairs, EXEC/VCC, and inline integer constants are supported for masks.
//! The U32 ADD/ADDC/SUB/SUBB subset accepts only individual s0..s101 and inline -16..64.
//! SCC is carry-out for addition, borrow-out for subtraction (zero means no borrow).
//! Literals, special-register U32 operands, modifiers, and S_MOV_B32 are not admitted.
//! https://github.com/llvm/llvm-project/blob/llvmorg-22.1.8/llvm/lib/Target/AMDGPU/SOPInstructions.td

use crate::{
    AuthenticatedPhysicalMachineAnalysisExecutionV1,
    AuthenticatedPhysicalMachineAnalysisReceiptIdentityV1, Gfx942RegisterAliasV1,
    Gfx942RegisterUnitV1, PhysicalMachineBranchKindV1, PhysicalMachineInstructionTraceV1,
    PhysicalMachineMemoryAccessV1, PhysicalMachineOperandValueV1,
    PhysicalMachineTraceEvidenceIdentityV1, PhysicalMachineTraceEvidenceV1,
};
use std::{error::Error, fmt};

#[macro_use]
mod transition_expressions {
    include!("../verus/gfx942_exec_semantics_v1.rs");
}

pub const MAX_GFX942_EXEC_STEPS_V1: usize = 65_536;
pub const MAX_GFX942_EXEC_TRACE_INSTRUCTIONS_V1: usize = 16_384;
pub const MAX_GFX942_EXEC_OBSERVATIONS_V1: usize = 65_536;
const SGPR_WORDS: usize = 102;

/// Caller-declared slice coordinates, in canonical HSACO file offsets.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Gfx942ExecSliceV1<'a> {
    function_symbol: &'a str,
    start_offset: u64,
    stop_offset: u64,
}

impl<'a> Gfx942ExecSliceV1<'a> {
    pub fn new(
        function_symbol: &'a str,
        start_offset: u64,
        stop_offset: u64,
    ) -> Result<Self, Gfx942ExecExecutionErrorV1> {
        if function_symbol.is_empty()
            || function_symbol.len() > 256
            || start_offset >= stop_offset
            || !start_offset.is_multiple_of(4)
            || !stop_offset.is_multiple_of(4)
        {
            return Err(Gfx942ExecExecutionErrorV1::InvalidSlice);
        }
        Ok(Self {
            function_symbol,
            start_offset,
            stop_offset,
        })
    }

    pub const fn function_symbol(self) -> &'a str {
        self.function_symbol
    }
    pub const fn start_offset(self) -> u64 {
        self.start_offset
    }
    pub const fn stop_offset(self) -> u64 {
        self.stop_offset
    }
    pub const fn byte_len(self) -> u64 {
        self.stop_offset - self.start_offset
    }
}

/// Undefined registers remain undefined until explicitly supplied or written by execution.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Gfx942ExecStateV1 {
    sgpr: [Option<u32>; SGPR_WORDS],
    exec: Option<u64>,
    vcc: Option<u64>,
    scc: Option<bool>,
}

impl Default for Gfx942ExecStateV1 {
    fn default() -> Self {
        Self {
            sgpr: [None; SGPR_WORDS],
            exec: None,
            vcc: None,
            scc: None,
        }
    }
}

impl Gfx942ExecStateV1 {
    pub fn set_sgpr(&mut self, index: u16, value: u32) -> Result<(), Gfx942ExecExecutionErrorV1> {
        let slot = self
            .sgpr
            .get_mut(usize::from(index))
            .ok_or(Gfx942ExecExecutionErrorV1::InvalidSgpr(index))?;
        *slot = Some(value);
        Ok(())
    }
    pub fn set_exec(&mut self, value: u64) {
        self.exec = Some(value);
    }
    pub fn set_vcc(&mut self, value: u64) {
        self.vcc = Some(value);
    }
    pub fn set_scc(&mut self, value: bool) {
        self.scc = Some(value);
    }
    pub fn sgpr(&self, index: u16) -> Option<u32> {
        self.sgpr.get(usize::from(index)).copied().flatten()
    }
    pub const fn exec(&self) -> Option<u64> {
        self.exec
    }
    pub const fn vcc(&self) -> Option<u64> {
        self.vcc
    }
    pub const fn scc(&self) -> Option<bool> {
        self.scc
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Gfx942ExecLimitsV1 {
    max_steps: usize,
    max_trace_instructions: usize,
    max_observations: usize,
}

impl Gfx942ExecLimitsV1 {
    /// Zero budgets are valid and reject the first required unit of work.
    pub fn new(
        max_steps: usize,
        max_trace_instructions: usize,
        max_observations: usize,
    ) -> Result<Self, Gfx942ExecExecutionErrorV1> {
        if max_steps > MAX_GFX942_EXEC_STEPS_V1
            || max_trace_instructions > MAX_GFX942_EXEC_TRACE_INSTRUCTIONS_V1
            || max_observations > MAX_GFX942_EXEC_OBSERVATIONS_V1
        {
            return Err(Gfx942ExecExecutionErrorV1::InvalidLimits);
        }
        Ok(Self {
            max_steps,
            max_trace_instructions,
            max_observations,
        })
    }
    pub const fn max_steps(self) -> usize {
        self.max_steps
    }
    pub const fn max_trace_instructions(self) -> usize {
        self.max_trace_instructions
    }
    pub const fn max_observations(self) -> usize {
        self.max_observations
    }
}

impl Default for Gfx942ExecLimitsV1 {
    fn default() -> Self {
        Self {
            max_steps: 4096,
            max_trace_instructions: MAX_GFX942_EXEC_TRACE_INSTRUCTIONS_V1,
            max_observations: 4096,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Gfx942ExecStepObservationV1 {
    pub instruction_offset: u64,
    pub next_offset: u64,
    pub exec_before: Option<u64>,
    pub exec_after: Option<u64>,
    pub scc_before: Option<bool>,
    pub scc_after: Option<bool>,
}

/// A complete observation of this slice for precisely the supplied live-ins, not a theorem.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Gfx942ExecObservationV1 {
    function_symbol: Box<str>,
    start_offset: u64,
    stop_offset: u64,
    initial_state: Gfx942ExecStateV1,
    final_state: Gfx942ExecStateV1,
    steps: Vec<Gfx942ExecStepObservationV1>,
    trace_identity: PhysicalMachineTraceEvidenceIdentityV1,
    authenticated_execution_identity: Option<AuthenticatedPhysicalMachineAnalysisReceiptIdentityV1>,
}

impl Gfx942ExecObservationV1 {
    pub fn function_symbol(&self) -> &str {
        &self.function_symbol
    }
    pub const fn start_offset(&self) -> u64 {
        self.start_offset
    }
    pub const fn stop_offset(&self) -> u64 {
        self.stop_offset
    }
    pub const fn final_pc(&self) -> u64 {
        self.stop_offset
    }
    pub const fn initial_state(&self) -> &Gfx942ExecStateV1 {
        &self.initial_state
    }
    pub const fn final_state(&self) -> &Gfx942ExecStateV1 {
        &self.final_state
    }
    pub fn steps(&self) -> &[Gfx942ExecStepObservationV1] {
        &self.steps
    }
    pub const fn trace_identity(&self) -> PhysicalMachineTraceEvidenceIdentityV1 {
        self.trace_identity
    }
    pub const fn authenticated_execution_identity(
        &self,
    ) -> Option<AuthenticatedPhysicalMachineAnalysisReceiptIdentityV1> {
        self.authenticated_execution_identity
    }
    pub const fn is_conditional_on_supplied_live_ins(&self) -> bool {
        true
    }
    pub const fn establishes_kernel_entry_state(&self) -> bool {
        false
    }
    pub const fn establishes_compiler_refinement(&self) -> bool {
        false
    }
    pub const fn grants_runtime_authority(&self) -> bool {
        false
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum Gfx942ExecExecutionErrorV1 {
    InvalidSlice,
    InvalidLimits,
    InvalidSgpr(u16),
    TraceInstructionLimit {
        actual: usize,
        maximum: usize,
    },
    StepLimit {
        completed: usize,
        maximum: usize,
    },
    ObservationLimit {
        completed: usize,
        maximum: usize,
    },
    AllocationFailed,
    MissingBoundary {
        offset: u64,
    },
    UnsupportedInstruction {
        offset: u64,
    },
    InvalidEncoding {
        offset: u64,
    },
    InvalidOperands {
        offset: u64,
    },
    InvalidEffects {
        offset: u64,
    },
    InvalidBranchTarget {
        offset: u64,
    },
    UndefinedRegister {
        offset: u64,
        register: Gfx942RegisterUnitV1,
    },
}

impl fmt::Display for Gfx942ExecExecutionErrorV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "conditional gfx942 EXEC slice execution failed: {self:?}"
        )
    }
}
impl Error for Gfx942ExecExecutionErrorV1 {}

pub fn execute_authenticated_gfx942_exec_slice_v1(
    execution: &AuthenticatedPhysicalMachineAnalysisExecutionV1,
    slice: Gfx942ExecSliceV1<'_>,
    initial: &Gfx942ExecStateV1,
    limits: Gfx942ExecLimitsV1,
) -> Result<Gfx942ExecObservationV1, Gfx942ExecExecutionErrorV1> {
    let mut observation =
        execute_gfx942_exec_slice_v1(execution.analysis().trace(), slice, initial, limits)?;
    observation.authenticated_execution_identity = Some(execution.identity());
    Ok(observation)
}

/// Offline replay of decoded extractor facts. This does not authenticate their producer.
pub fn execute_gfx942_exec_slice_v1(
    trace: &PhysicalMachineTraceEvidenceV1,
    slice: Gfx942ExecSliceV1<'_>,
    initial: &Gfx942ExecStateV1,
    limits: Gfx942ExecLimitsV1,
) -> Result<Gfx942ExecObservationV1, Gfx942ExecExecutionErrorV1> {
    if trace.instructions().len() > limits.max_trace_instructions {
        return Err(Gfx942ExecExecutionErrorV1::TraceInstructionLimit {
            actual: trace.instructions().len(),
            maximum: limits.max_trace_instructions,
        });
    }
    // Index only borrowed existing instructions. The scan and allocation are bounded before use.
    let mut instructions = Vec::new();
    instructions
        .try_reserve_exact(trace.instructions().len())
        .map_err(|_| Gfx942ExecExecutionErrorV1::AllocationFailed)?;
    for instruction in trace.instructions() {
        if instruction.function_symbol() == slice.function_symbol
            && instruction.instruction_offset() >= slice.start_offset
            && instruction.instruction_offset() <= slice.stop_offset
        {
            instructions.push(instruction);
        }
    }
    for boundary in [slice.start_offset, slice.stop_offset] {
        find_instruction(&instructions, boundary)
            .ok_or(Gfx942ExecExecutionErrorV1::MissingBoundary { offset: boundary })?;
    }
    for instruction in &instructions {
        if instruction.instruction_offset() == slice.stop_offset {
            break;
        }
        validate_instruction(instruction)?;
        let fallthrough = next_offset(instruction)?;
        if find_instruction(&instructions, fallthrough).is_none() {
            return Err(Gfx942ExecExecutionErrorV1::MissingBoundary {
                offset: fallthrough,
            });
        }
        if instruction.branch_kind() != PhysicalMachineBranchKindV1::None {
            let target = branch_target(instruction)?;
            if find_instruction(&instructions, target).is_none() {
                return Err(Gfx942ExecExecutionErrorV1::InvalidBranchTarget {
                    offset: instruction.instruction_offset(),
                });
            }
        }
    }
    let mut state = initial.clone();
    let mut pc = slice.start_offset;
    let mut steps = Vec::new();
    steps
        .try_reserve_exact(limits.max_steps.min(limits.max_observations))
        .map_err(|_| Gfx942ExecExecutionErrorV1::AllocationFailed)?;
    while pc != slice.stop_offset {
        if steps.len() == limits.max_steps {
            return Err(Gfx942ExecExecutionErrorV1::StepLimit {
                completed: steps.len(),
                maximum: limits.max_steps,
            });
        }
        if steps.len() == limits.max_observations {
            return Err(Gfx942ExecExecutionErrorV1::ObservationLimit {
                completed: steps.len(),
                maximum: limits.max_observations,
            });
        }
        let instruction = find_instruction(&instructions, pc)
            .ok_or(Gfx942ExecExecutionErrorV1::MissingBoundary { offset: pc })?;
        let exec_before = state.exec;
        let scc_before = state.scc;
        let next = execute_instruction(instruction, &mut state)?;
        steps.push(Gfx942ExecStepObservationV1 {
            instruction_offset: pc,
            next_offset: next,
            exec_before,
            exec_after: state.exec,
            scc_before,
            scc_after: state.scc,
        });
        pc = next;
    }
    Ok(Gfx942ExecObservationV1 {
        function_symbol: slice.function_symbol.into(),
        start_offset: slice.start_offset,
        stop_offset: slice.stop_offset,
        initial_state: initial.clone(),
        final_state: state,
        steps,
        trace_identity: trace.identity(),
        authenticated_execution_identity: None,
    })
}

fn find_instruction<'a>(
    instructions: &[&'a PhysicalMachineInstructionTraceV1],
    offset: u64,
) -> Option<&'a PhysicalMachineInstructionTraceV1> {
    instructions
        .binary_search_by_key(&offset, |instruction| instruction.instruction_offset())
        .ok()
        .map(|index| instructions[index])
}

fn next_offset(
    instruction: &PhysicalMachineInstructionTraceV1,
) -> Result<u64, Gfx942ExecExecutionErrorV1> {
    instruction.instruction_offset().checked_add(4).ok_or(
        Gfx942ExecExecutionErrorV1::InvalidBranchTarget {
            offset: instruction.instruction_offset(),
        },
    )
}

fn branch_target(
    instruction: &PhysicalMachineInstructionTraceV1,
) -> Result<u64, Gfx942ExecExecutionErrorV1> {
    let word = instruction_word(instruction)?;
    let displacement = i64::from(word as u16 as i16);
    let target = instruction
        .instruction_offset()
        .checked_add_signed(gfx942_exec_transition_v1!(branch_delta, displacement))
        .ok_or(Gfx942ExecExecutionErrorV1::InvalidBranchTarget {
            offset: instruction.instruction_offset(),
        })?;
    if instruction.branch_target() != Some(target) {
        return Err(Gfx942ExecExecutionErrorV1::InvalidBranchTarget {
            offset: instruction.instruction_offset(),
        });
    }
    Ok(target)
}

fn instruction_word(
    instruction: &PhysicalMachineInstructionTraceV1,
) -> Result<u32, Gfx942ExecExecutionErrorV1> {
    let bytes: [u8; 4] = instruction.encoding().try_into().map_err(|_| {
        Gfx942ExecExecutionErrorV1::InvalidEncoding {
            offset: instruction.instruction_offset(),
        }
    })?;
    Ok(u32::from_le_bytes(bytes))
}

fn exact_names(actual: &[String], expected: &[&str]) -> bool {
    actual
        .iter()
        .map(String::as_str)
        .eq(expected.iter().copied())
}

fn is_u32_arithmetic(opcode: &str) -> bool {
    matches!(
        opcode,
        "S_ADD_U32_vi" | "S_ADDC_U32_vi" | "S_SUB_U32_vi" | "S_SUBB_U32_vi"
    )
}

fn validate_instruction(
    instruction: &PhysicalMachineInstructionTraceV1,
) -> Result<(), Gfx942ExecExecutionErrorV1> {
    let offset = instruction.instruction_offset();
    let word = instruction_word(instruction)?;
    let operands = instruction.operands();
    if operands.iter().any(|operand| operand.tied_to().is_some()) {
        return Err(Gfx942ExecExecutionErrorV1::InvalidOperands { offset });
    }
    let (encoding, sources, saved, branch) = match instruction.opcode() {
        "S_ADD_U32_vi" => (0x8000_0000, 2, false, None),
        "S_SUB_U32_vi" => (0x8080_0000, 2, false, None),
        "S_ADDC_U32_vi" => (0x8200_0000, 2, false, None),
        "S_SUBB_U32_vi" => (0x8280_0000, 2, false, None),
        "S_MOV_B64_vi" => (0xbe80_0100, 1, false, None),
        "S_AND_SAVEEXEC_B64_vi" => (0xbe80_2000, 1, true, None),
        "S_ANDN2_SAVEEXEC_B64_vi" => (0xbe80_2300, 1, true, None),
        "S_AND_B64_vi" => (0x8000_0000 | (13 << 23), 2, false, None),
        "S_OR_B64_vi" => (0x8000_0000 | (15 << 23), 2, false, None),
        "S_XOR_B64_vi" => (0x8000_0000 | (17 << 23), 2, false, None),
        "S_ANDN2_B64_vi" => (0x8000_0000 | (19 << 23), 2, false, None),
        "S_BRANCH_vi" => (
            0xbf82_0000,
            0,
            false,
            Some(PhysicalMachineBranchKindV1::UnconditionalDirect),
        ),
        "S_CBRANCH_EXECZ_vi" => (
            0xbf88_0000,
            0,
            false,
            Some(PhysicalMachineBranchKindV1::ConditionalDirect),
        ),
        "S_CBRANCH_EXECNZ_vi" => (
            0xbf89_0000,
            0,
            false,
            Some(PhysicalMachineBranchKindV1::ConditionalDirect),
        ),
        _ => return Err(Gfx942ExecExecutionErrorV1::UnsupportedInstruction { offset }),
    };
    let encoding_mask = if branch.is_some() {
        0xffff_0000
    } else if sources == 1 {
        0xff80_ff00
    } else {
        0xff80_0000
    };
    if word & encoding_mask != encoding {
        return Err(Gfx942ExecExecutionErrorV1::InvalidEncoding { offset });
    }
    if let Some(kind) = branch {
        if instruction.explicit_definition_count() != 0
            || operands.len() != 1
            || operands[0].value()
                != &PhysicalMachineOperandValueV1::SignedImmediate(i64::from(word as u16 as i16))
        {
            return Err(Gfx942ExecExecutionErrorV1::InvalidOperands { offset });
        }
        let conditional = kind == PhysicalMachineBranchKindV1::ConditionalDirect;
        // LLVM marks unconditional branches as barriers (no fallthrough), not EXEC branches.
        let flags = if conditional { 4 } else { 4 | 8 };
        if instruction.branch_kind() != kind
            || instruction.flags().bits() != flags
            || instruction.memory_access() != PhysicalMachineMemoryAccessV1::None
            || !instruction.implicit_definitions().is_empty()
            || !exact_names(
                instruction.implicit_uses(),
                if conditional { &["EXEC"] } else { &[] },
            )
        {
            return Err(Gfx942ExecExecutionErrorV1::InvalidEffects { offset });
        }
        branch_target(instruction)?;
        return Ok(());
    }
    if instruction.explicit_definition_count() != 1 || operands.len() != sources + 1 {
        return Err(Gfx942ExecExecutionErrorV1::InvalidOperands { offset });
    }
    let scalar32 = is_u32_arithmetic(instruction.opcode());
    let destination = if scalar32 {
        sgpr32_selector(operands[0].value(), offset)?
    } else {
        register_selector(operands[0].value(), offset)?
    };
    if (saved && destination > 100) || destination != ((word >> 16) & 0x7f) as u8 {
        return Err(Gfx942ExecExecutionErrorV1::InvalidOperands { offset });
    }
    for (index, operand) in operands[1..].iter().enumerate() {
        let selector = source_selector(operand.value(), offset, scalar32)?;
        if selector != ((word >> (index * 8)) & 0xff) as u8 {
            return Err(Gfx942ExecExecutionErrorV1::InvalidOperands { offset });
        }
    }
    let definitions: &[&str] = if saved {
        &["EXEC", "SCC"]
    } else if instruction.opcode() == "S_MOV_B64_vi" {
        &[]
    } else {
        &["SCC"]
    };
    let uses: &[&str] = if saved {
        &["EXEC"]
    } else if matches!(instruction.opcode(), "S_ADDC_U32_vi" | "S_SUBB_U32_vi") {
        &["SCC"]
    } else {
        &[]
    };
    if instruction.branch_kind() != PhysicalMachineBranchKindV1::None
        || instruction.branch_target().is_some()
        || instruction.flags().bits() != 0
        || instruction.memory_access() != PhysicalMachineMemoryAccessV1::None
        || !exact_names(instruction.implicit_definitions(), definitions)
        || !exact_names(instruction.implicit_uses(), uses)
    {
        return Err(Gfx942ExecExecutionErrorV1::InvalidEffects { offset });
    }
    Ok(())
}

fn sgpr32_selector(
    value: &PhysicalMachineOperandValueV1,
    offset: u64,
) -> Result<u8, Gfx942ExecExecutionErrorV1> {
    let PhysicalMachineOperandValueV1::Register(name) = value else {
        return Err(Gfx942ExecExecutionErrorV1::InvalidOperands { offset });
    };
    let alias = Gfx942RegisterAliasV1::decode(name)
        .map_err(|_| Gfx942ExecExecutionErrorV1::InvalidOperands { offset })?;
    match alias.units() {
        [Gfx942RegisterUnitV1::Sgpr(index)] if usize::from(*index) < SGPR_WORDS => Ok(*index as u8),
        _ => Err(Gfx942ExecExecutionErrorV1::InvalidOperands { offset }),
    }
}

fn register_selector(
    value: &PhysicalMachineOperandValueV1,
    offset: u64,
) -> Result<u8, Gfx942ExecExecutionErrorV1> {
    let PhysicalMachineOperandValueV1::Register(name) = value else {
        return Err(Gfx942ExecExecutionErrorV1::InvalidOperands { offset });
    };
    let alias = Gfx942RegisterAliasV1::decode(name)
        .map_err(|_| Gfx942ExecExecutionErrorV1::InvalidOperands { offset })?;
    match alias.units() {
        [
            Gfx942RegisterUnitV1::Sgpr(low),
            Gfx942RegisterUnitV1::Sgpr(high),
        ] if *low <= 100 && *low % 2 == 0 && *high == *low + 1 => Ok(*low as u8),
        [Gfx942RegisterUnitV1::VccLow, Gfx942RegisterUnitV1::VccHigh] => Ok(106),
        [
            Gfx942RegisterUnitV1::ExecLow,
            Gfx942RegisterUnitV1::ExecHigh,
        ] => Ok(126),
        _ => Err(Gfx942ExecExecutionErrorV1::InvalidOperands { offset }),
    }
}

fn source_selector(
    value: &PhysicalMachineOperandValueV1,
    offset: u64,
    scalar32: bool,
) -> Result<u8, Gfx942ExecExecutionErrorV1> {
    match value {
        PhysicalMachineOperandValueV1::SignedImmediate(value) if (0..=64).contains(value) => {
            Ok((128 + value) as u8)
        }
        PhysicalMachineOperandValueV1::SignedImmediate(value) if (-16..=-1).contains(value) => {
            Ok((192 - value) as u8)
        }
        _ if scalar32 => sgpr32_selector(value, offset),
        _ => register_selector(value, offset),
    }
}

fn read_source32(
    value: &PhysicalMachineOperandValueV1,
    state: &Gfx942ExecStateV1,
    offset: u64,
) -> Result<u32, Gfx942ExecExecutionErrorV1> {
    if let PhysicalMachineOperandValueV1::SignedImmediate(value) = value {
        return Ok(*value as u32);
    }
    let index = sgpr32_selector(value, offset)?;
    state.sgpr[usize::from(index)].ok_or(Gfx942ExecExecutionErrorV1::UndefinedRegister {
        offset,
        register: Gfx942RegisterUnitV1::Sgpr(u16::from(index)),
    })
}

fn read_source(
    value: &PhysicalMachineOperandValueV1,
    state: &Gfx942ExecStateV1,
    offset: u64,
) -> Result<u64, Gfx942ExecExecutionErrorV1> {
    if let PhysicalMachineOperandValueV1::SignedImmediate(value) = value {
        return Ok(*value as u64);
    }
    let selector = register_selector(value, offset)?;
    let undefined = |register| Gfx942ExecExecutionErrorV1::UndefinedRegister { offset, register };
    match selector {
        106 => state
            .vcc
            .ok_or_else(|| undefined(Gfx942RegisterUnitV1::VccLow)),
        126 => state
            .exec
            .ok_or_else(|| undefined(Gfx942RegisterUnitV1::ExecLow)),
        index => {
            let low = state.sgpr[usize::from(index)]
                .ok_or_else(|| undefined(Gfx942RegisterUnitV1::Sgpr(u16::from(index))))?;
            let high = state.sgpr[usize::from(index) + 1]
                .ok_or_else(|| undefined(Gfx942RegisterUnitV1::Sgpr(u16::from(index) + 1)))?;
            Ok(u64::from(low) | (u64::from(high) << 32))
        }
    }
}

fn execute_instruction(
    instruction: &PhysicalMachineInstructionTraceV1,
    state: &mut Gfx942ExecStateV1,
) -> Result<u64, Gfx942ExecExecutionErrorV1> {
    let offset = instruction.instruction_offset();
    let next = next_offset(instruction)?;
    let opcode = instruction.opcode();
    if instruction.branch_kind() != PhysicalMachineBranchKindV1::None {
        let target = branch_target(instruction)?;
        if opcode == "S_BRANCH_vi" {
            return Ok(target);
        }
        let exec = state
            .exec
            .ok_or(Gfx942ExecExecutionErrorV1::UndefinedRegister {
                offset,
                register: Gfx942RegisterUnitV1::ExecLow,
            })?;
        return Ok(if opcode == "S_CBRANCH_EXECZ_vi" {
            gfx942_exec_transition_v1!(execz, exec, target, next)
        } else {
            gfx942_exec_transition_v1!(execnz, exec, target, next)
        });
    }
    let operands = instruction.operands();
    if is_u32_arithmetic(opcode) {
        let destination = sgpr32_selector(operands[0].value(), offset)?;
        // Snapshot both words and incoming SCC before writing any aliased pair member.
        let left = read_source32(operands[1].value(), state, offset)?;
        let right = read_source32(operands[2].value(), state, offset)?;
        let carry_or_borrow = if matches!(opcode, "S_ADDC_U32_vi" | "S_SUBB_U32_vi") {
            state
                .scc
                .ok_or(Gfx942ExecExecutionErrorV1::UndefinedRegister {
                    offset,
                    register: Gfx942RegisterUnitV1::Scc,
                })?
        } else {
            false
        };
        let (value, scc) = if matches!(opcode, "S_ADD_U32_vi" | "S_ADDC_U32_vi") {
            gfx942_exec_transition_v1!(add_u32, left, right, carry_or_borrow)
        } else {
            gfx942_exec_transition_v1!(sub_u32, left, right, carry_or_borrow)
        };
        state.sgpr[usize::from(destination)] = Some(value);
        state.scc = Some(scc);
        return Ok(next);
    }
    let destination = register_selector(operands[0].value(), offset)?;
    // Every source, including old EXEC, is snapshotted before any destination write.
    let left = read_source(operands[1].value(), state, offset)?;
    let right = if operands.len() == 3 {
        read_source(operands[2].value(), state, offset)?
    } else {
        0
    };
    let (value, scc, exec_override) = match opcode {
        "S_MOV_B64_vi" => gfx942_exec_transition_v1!(mov, left, state.scc),
        "S_AND_B64_vi" => gfx942_exec_transition_v1!(and, left, right),
        "S_OR_B64_vi" => gfx942_exec_transition_v1!(or, left, right),
        "S_XOR_B64_vi" => gfx942_exec_transition_v1!(xor, left, right),
        "S_ANDN2_B64_vi" => gfx942_exec_transition_v1!(andn2, left, right),
        "S_AND_SAVEEXEC_B64_vi" | "S_ANDN2_SAVEEXEC_B64_vi" => {
            let exec = state
                .exec
                .ok_or(Gfx942ExecExecutionErrorV1::UndefinedRegister {
                    offset,
                    register: Gfx942RegisterUnitV1::ExecLow,
                })?;
            if opcode == "S_AND_SAVEEXEC_B64_vi" {
                gfx942_exec_transition_v1!(and_save, left, exec)
            } else {
                gfx942_exec_transition_v1!(andn2_save, left, exec)
            }
        }
        _ => return Err(Gfx942ExecExecutionErrorV1::UnsupportedInstruction { offset }),
    };
    match destination {
        106 => state.vcc = Some(value),
        126 => state.exec = Some(value),
        index => {
            state.sgpr[usize::from(index)] = Some(value as u32);
            state.sgpr[usize::from(index) + 1] = Some((value >> 32) as u32);
        }
    }
    if let Some(exec) = exec_override {
        state.exec = Some(exec);
    }
    state.scc = scc;
    Ok(next)
}
