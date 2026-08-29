//! Canonical LLVM/MC instruction and control-flow trace for one exact gfx942 HSACO.
//!
//! The trace is decoded independently from the native analyzer output and is
//! checked against the exact payload bytes and the closed physical-effect
//! evidence. It preserves instruction encodings, operands, explicit and
//! implicit register def/use facts, basic blocks, direct control-flow targets,
//! and native exact trap classification. These are extractor facts, not an
//! executable semantics: this type
//! does not prove source/compiler refinement, address safety, race freedom,
//! floating-point behavior, termination, or launch safety.

use crate::{
    PhysicalMachineEffectEvidenceV1, PhysicalMachineEffectKindV1, PhysicalMachineEffectRequestV1,
};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::{error::Error, fmt};

pub const PHYSICAL_MACHINE_TRACE_EVIDENCE_DOMAIN_V1: &[u8] =
    b"FE2O3/GFX942-PHYSICAL-MACHINE-TRACE-EVIDENCE/V1\0";
pub const PHYSICAL_MACHINE_TRACE_SCHEMA_VERSION_V1: u16 = 1;
const TRACE_IDENTITY_DOMAIN_V1: &[u8] =
    b"FE2O3/GFX942-PHYSICAL-MACHINE-TRACE-EVIDENCE-IDENTITY/V1\0";

pub const MAX_PHYSICAL_MACHINE_TRACE_BYTES_V1: usize = 16 * 1024 * 1024;
pub const MAX_PHYSICAL_MACHINE_TRACE_BLOCKS_V1: usize = 4_096;
pub const MAX_PHYSICAL_MACHINE_TRACE_INSTRUCTIONS_V1: usize = 16_384;
pub const MAX_PHYSICAL_MACHINE_TRACE_OPERANDS_V1: usize = 64;
pub const MAX_PHYSICAL_MACHINE_TRACE_REGISTERS_V1: usize = 64;
pub const MAX_PHYSICAL_MACHINE_INSTRUCTION_BYTES_V1: usize = 32;
const MAX_MACHINE_TOKEN_BYTES_V1: usize = 256;
const NO_TIED_OPERAND_V1: u16 = u16::MAX;

const FLAG_MAY_LOAD_V1: u16 = 1 << 0;
const FLAG_MAY_STORE_V1: u16 = 1 << 1;
const FLAG_TERMINATOR_V1: u16 = 1 << 2;
const FLAG_BARRIER_V1: u16 = 1 << 3;
const FLAG_PREDICABLE_V1: u16 = 1 << 4;
const FLAG_MAY_TRAP_V1: u16 = 1 << 5;
const KNOWN_INSTRUCTION_FLAGS_V1: u16 = FLAG_MAY_LOAD_V1
    | FLAG_MAY_STORE_V1
    | FLAG_TERMINATOR_V1
    | FLAG_BARRIER_V1
    | FLAG_PREDICABLE_V1
    | FLAG_MAY_TRAP_V1;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PhysicalMachineTraceEvidenceIdentityV1 {
    sha256: [u8; 32],
    byte_len: u64,
}

impl PhysicalMachineTraceEvidenceIdentityV1 {
    pub const fn sha256(self) -> [u8; 32] {
        self.sha256
    }

    pub const fn byte_len(self) -> u64 {
        self.byte_len
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PhysicalMachineInstructionFlagsV1(u16);

impl PhysicalMachineInstructionFlagsV1 {
    pub const fn may_load(self) -> bool {
        self.0 & FLAG_MAY_LOAD_V1 != 0
    }

    pub const fn may_store(self) -> bool {
        self.0 & FLAG_MAY_STORE_V1 != 0
    }

    pub const fn is_terminator(self) -> bool {
        self.0 & FLAG_TERMINATOR_V1 != 0
    }

    pub const fn is_barrier(self) -> bool {
        self.0 & FLAG_BARRIER_V1 != 0
    }

    pub const fn is_predicable(self) -> bool {
        self.0 & FLAG_PREDICABLE_V1 != 0
    }

    /// The native analyzer classifies this exact instruction encoding as a trap.
    ///
    /// This is an extractor fact, not evidence that the trap is unreachable.
    pub const fn may_trap(self) -> bool {
        self.0 & FLAG_MAY_TRAP_V1 != 0
    }

    pub const fn bits(self) -> u16 {
        self.0
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum PhysicalMachineOperandValueV1 {
    Register(String),
    SignedImmediate(i64),
    SingleFloatImmediate(u32),
    DoubleFloatImmediate(u64),
    AbsoluteExpression(i64),
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PhysicalMachineOperandV1 {
    value: PhysicalMachineOperandValueV1,
    tied_to: Option<u16>,
}

impl PhysicalMachineOperandV1 {
    pub const fn value(&self) -> &PhysicalMachineOperandValueV1 {
        &self.value
    }

    pub const fn tied_to(&self) -> Option<u16> {
        self.tied_to
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum PhysicalMachineBranchKindV1 {
    None,
    ConditionalDirect,
    UnconditionalDirect,
    DirectCall,
    Return,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum PhysicalMachineMemoryAccessV1 {
    None,
    Read { byte_width: u16 },
    Write { byte_width: u16 },
}

impl PhysicalMachineMemoryAccessV1 {
    pub const fn byte_width(self) -> u16 {
        match self {
            Self::None => 0,
            Self::Read { byte_width } | Self::Write { byte_width } => byte_width,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PhysicalMachineBasicBlockTraceV1 {
    function_symbol: String,
    ordinal: u32,
    first_instruction_offset: u64,
    instruction_count: u32,
    successors: Vec<u32>,
}

impl PhysicalMachineBasicBlockTraceV1 {
    pub fn function_symbol(&self) -> &str {
        &self.function_symbol
    }

    pub const fn ordinal(&self) -> u32 {
        self.ordinal
    }

    pub const fn first_instruction_offset(&self) -> u64 {
        self.first_instruction_offset
    }

    pub const fn instruction_count(&self) -> u32 {
        self.instruction_count
    }

    pub fn successors(&self) -> &[u32] {
        &self.successors
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PhysicalMachineInstructionTraceV1 {
    function_symbol: String,
    instruction_offset: u64,
    block_ordinal: u32,
    opcode: String,
    encoding: Vec<u8>,
    explicit_definition_count: u16,
    operands: Vec<PhysicalMachineOperandV1>,
    implicit_definitions: Vec<String>,
    implicit_uses: Vec<String>,
    branch_kind: PhysicalMachineBranchKindV1,
    branch_target: Option<u64>,
    flags: PhysicalMachineInstructionFlagsV1,
    memory_access: PhysicalMachineMemoryAccessV1,
}

impl PhysicalMachineInstructionTraceV1 {
    pub fn function_symbol(&self) -> &str {
        &self.function_symbol
    }

    pub const fn instruction_offset(&self) -> u64 {
        self.instruction_offset
    }

    pub const fn block_ordinal(&self) -> u32 {
        self.block_ordinal
    }

    pub fn opcode(&self) -> &str {
        &self.opcode
    }

    pub fn encoding(&self) -> &[u8] {
        &self.encoding
    }

    pub const fn explicit_definition_count(&self) -> u16 {
        self.explicit_definition_count
    }

    pub fn operands(&self) -> &[PhysicalMachineOperandV1] {
        &self.operands
    }

    pub fn implicit_definitions(&self) -> &[String] {
        &self.implicit_definitions
    }

    pub fn implicit_uses(&self) -> &[String] {
        &self.implicit_uses
    }

    pub const fn branch_kind(&self) -> PhysicalMachineBranchKindV1 {
        self.branch_kind
    }

    pub const fn branch_target(&self) -> Option<u64> {
        self.branch_target
    }

    pub const fn flags(&self) -> PhysicalMachineInstructionFlagsV1 {
        self.flags
    }

    pub const fn memory_access(&self) -> PhysicalMachineMemoryAccessV1 {
        self.memory_access
    }
}

/// Independently decoded, exact-byte-bound instruction and finite-CFG trace.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PhysicalMachineTraceEvidenceV1 {
    blocks: Vec<PhysicalMachineBasicBlockTraceV1>,
    instructions: Vec<PhysicalMachineInstructionTraceV1>,
    canonical_bytes: Vec<u8>,
}

impl PhysicalMachineTraceEvidenceV1 {
    pub fn decode_canonical_for(
        request: &PhysicalMachineEffectRequestV1,
        effects: &PhysicalMachineEffectEvidenceV1,
        bytes: &[u8],
    ) -> Result<Self, PhysicalMachineTraceEvidenceErrorV1> {
        decode_trace(request, effects, bytes)
    }

    pub fn blocks(&self) -> &[PhysicalMachineBasicBlockTraceV1] {
        &self.blocks
    }

    pub fn instructions(&self) -> &[PhysicalMachineInstructionTraceV1] {
        &self.instructions
    }

    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }

    pub fn identity(&self) -> PhysicalMachineTraceEvidenceIdentityV1 {
        PhysicalMachineTraceEvidenceIdentityV1 {
            sha256: domain_hash(TRACE_IDENTITY_DOMAIN_V1, &self.canonical_bytes),
            byte_len: self.canonical_bytes.len() as u64,
        }
    }

    pub const fn binds_exact_payload_instruction_bytes(&self) -> bool {
        true
    }

    pub const fn establishes_machine_semantics(&self) -> bool {
        false
    }

    pub const fn establishes_compiler_refinement(&self) -> bool {
        false
    }

    pub const fn grants_load_authority(&self) -> bool {
        false
    }

    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

fn decode_trace(
    request: &PhysicalMachineEffectRequestV1,
    effects: &PhysicalMachineEffectEvidenceV1,
    bytes: &[u8],
) -> Result<PhysicalMachineTraceEvidenceV1, PhysicalMachineTraceEvidenceErrorV1> {
    if bytes.len() > MAX_PHYSICAL_MACHINE_TRACE_BYTES_V1 {
        return Err(PhysicalMachineTraceEvidenceErrorV1::RecordTooLarge);
    }
    let mut input = TraceReader::new(bytes);
    input.expect(PHYSICAL_MACHINE_TRACE_EVIDENCE_DOMAIN_V1)?;
    if input.u32()? as usize != bytes.len() {
        return Err(PhysicalMachineTraceEvidenceErrorV1::LengthMismatch);
    }
    if input.u16()? != PHYSICAL_MACHINE_TRACE_SCHEMA_VERSION_V1 {
        return Err(PhysicalMachineTraceEvidenceErrorV1::UnsupportedVersion);
    }
    require_equal(
        input.array()?,
        request.execution_challenge().as_bytes(),
        "execution challenge",
    )?;
    require_identity(
        &mut input,
        request.identity().sha256(),
        request.identity().byte_len(),
        "machine request",
    )?;
    require_identity(
        &mut input,
        effects.identity().sha256(),
        effects.identity().byte_len(),
        "machine-effect evidence",
    )?;
    require_identity(
        &mut input,
        request.payload_identity().sha256(),
        request.payload_identity().byte_len(),
        "finalized payload",
    )?;
    require_equal(
        input.array()?,
        request.analyzer_identity().as_bytes(),
        "analyzer",
    )?;
    require_equal(
        input.array()?,
        request.toolchain_identity().as_bytes(),
        "toolchain",
    )?;
    if input.u16()? != 1 {
        return Err(PhysicalMachineTraceEvidenceErrorV1::TargetMismatch);
    }

    let block_count = input.u32()? as usize;
    if block_count == 0 || block_count > MAX_PHYSICAL_MACHINE_TRACE_BLOCKS_V1 {
        return Err(PhysicalMachineTraceEvidenceErrorV1::BlockCount);
    }
    let mut blocks = Vec::with_capacity(block_count);
    for _ in 0..block_count {
        let function_symbol = input.token()?;
        let ordinal = input.u32()?;
        let first_instruction_offset = input.u64()?;
        let instruction_count = input.u32()?;
        if instruction_count == 0 {
            return Err(PhysicalMachineTraceEvidenceErrorV1::InvalidBlock);
        }
        let successor_count = input.u16()? as usize;
        if successor_count > 2 {
            return Err(PhysicalMachineTraceEvidenceErrorV1::InvalidBlock);
        }
        let mut successors = Vec::with_capacity(successor_count);
        for _ in 0..successor_count {
            successors.push(input.u32()?);
        }
        if !strictly_sorted(&successors) {
            return Err(PhysicalMachineTraceEvidenceErrorV1::NonCanonicalOrder);
        }
        blocks.push(PhysicalMachineBasicBlockTraceV1 {
            function_symbol,
            ordinal,
            first_instruction_offset,
            instruction_count,
            successors,
        });
    }
    if !blocks
        .windows(2)
        .all(|pair| block_key(&pair[0]) < block_key(&pair[1]))
    {
        return Err(PhysicalMachineTraceEvidenceErrorV1::NonCanonicalOrder);
    }

    let instruction_count = input.u32()? as usize;
    if instruction_count == 0 || instruction_count > MAX_PHYSICAL_MACHINE_TRACE_INSTRUCTIONS_V1 {
        return Err(PhysicalMachineTraceEvidenceErrorV1::InstructionCount);
    }
    let mut instructions = Vec::with_capacity(instruction_count);
    for _ in 0..instruction_count {
        instructions.push(decode_instruction(&mut input)?);
    }
    input.finish()?;
    if !instructions
        .windows(2)
        .all(|pair| instruction_key(&pair[0]) < instruction_key(&pair[1]))
    {
        return Err(PhysicalMachineTraceEvidenceErrorV1::NonCanonicalOrder);
    }

    validate_trace(request, effects, &blocks, &instructions)?;
    Ok(PhysicalMachineTraceEvidenceV1 {
        blocks,
        instructions,
        canonical_bytes: bytes.to_vec(),
    })
}

fn decode_instruction(
    input: &mut TraceReader<'_>,
) -> Result<PhysicalMachineInstructionTraceV1, PhysicalMachineTraceEvidenceErrorV1> {
    let function_symbol = input.token()?;
    let instruction_offset = input.u64()?;
    let block_ordinal = input.u32()?;
    let opcode = input.token()?;
    let encoding_len = input.u16()? as usize;
    if encoding_len == 0 || encoding_len > MAX_PHYSICAL_MACHINE_INSTRUCTION_BYTES_V1 {
        return Err(PhysicalMachineTraceEvidenceErrorV1::InvalidInstruction);
    }
    let encoding = input.take(encoding_len)?.to_vec();
    let explicit_definition_count = input.u16()?;
    let operand_count = input.u16()? as usize;
    if operand_count > MAX_PHYSICAL_MACHINE_TRACE_OPERANDS_V1 {
        return Err(PhysicalMachineTraceEvidenceErrorV1::OperandCount);
    }
    let mut operands = Vec::with_capacity(operand_count);
    for _ in 0..operand_count {
        let tag = input.u8()?;
        let tied = input.u16()?;
        let tied_to = (tied != NO_TIED_OPERAND_V1).then_some(tied);
        let value = match tag {
            1 => PhysicalMachineOperandValueV1::Register(input.token()?),
            2 => PhysicalMachineOperandValueV1::SignedImmediate(input.u64()? as i64),
            3 => PhysicalMachineOperandValueV1::SingleFloatImmediate(input.u32()?),
            4 => PhysicalMachineOperandValueV1::DoubleFloatImmediate(input.u64()?),
            5 => PhysicalMachineOperandValueV1::AbsoluteExpression(input.u64()? as i64),
            _ => return Err(PhysicalMachineTraceEvidenceErrorV1::UnknownOperandKind),
        };
        if (!matches!(value, PhysicalMachineOperandValueV1::Register(_)) && tied_to.is_some())
            || tied_to.is_some_and(|index| index as usize >= operand_count)
        {
            return Err(PhysicalMachineTraceEvidenceErrorV1::InvalidOperand);
        }
        operands.push(PhysicalMachineOperandV1 { value, tied_to });
    }
    if explicit_definition_count as usize > operands.len()
        || operands[..explicit_definition_count as usize]
            .iter()
            .any(|operand| !matches!(operand.value, PhysicalMachineOperandValueV1::Register(_)))
    {
        return Err(PhysicalMachineTraceEvidenceErrorV1::InvalidOperand);
    }
    let implicit_definitions = decode_register_set(input)?;
    let implicit_uses = decode_register_set(input)?;
    let branch_kind = match input.u8()? {
        0 => PhysicalMachineBranchKindV1::None,
        1 => PhysicalMachineBranchKindV1::ConditionalDirect,
        2 => PhysicalMachineBranchKindV1::UnconditionalDirect,
        3 => PhysicalMachineBranchKindV1::DirectCall,
        4 => PhysicalMachineBranchKindV1::Return,
        _ => return Err(PhysicalMachineTraceEvidenceErrorV1::UnknownBranchKind),
    };
    let encoded_target = input.u64()?;
    let branch_target = match branch_kind {
        PhysicalMachineBranchKindV1::ConditionalDirect
        | PhysicalMachineBranchKindV1::UnconditionalDirect
        | PhysicalMachineBranchKindV1::DirectCall => {
            if encoded_target == 0 {
                return Err(PhysicalMachineTraceEvidenceErrorV1::InvalidControlFlow);
            }
            Some(encoded_target)
        }
        PhysicalMachineBranchKindV1::None | PhysicalMachineBranchKindV1::Return => {
            if encoded_target != 0 {
                return Err(PhysicalMachineTraceEvidenceErrorV1::InvalidControlFlow);
            }
            None
        }
    };
    let flag_bits = input.u16()?;
    if flag_bits & !KNOWN_INSTRUCTION_FLAGS_V1 != 0 {
        return Err(PhysicalMachineTraceEvidenceErrorV1::InvalidInstructionFlags);
    }
    let flags = PhysicalMachineInstructionFlagsV1(flag_bits);
    let memory_tag = input.u8()?;
    let byte_width = input.u16()?;
    let memory_access = match (memory_tag, byte_width) {
        (0, 0) => PhysicalMachineMemoryAccessV1::None,
        (1, 1..) => PhysicalMachineMemoryAccessV1::Read { byte_width },
        (2, 1..) => PhysicalMachineMemoryAccessV1::Write { byte_width },
        _ => return Err(PhysicalMachineTraceEvidenceErrorV1::InvalidMemoryAccess),
    };
    validate_instruction_shape(branch_kind, flags, memory_access)?;
    Ok(PhysicalMachineInstructionTraceV1 {
        function_symbol,
        instruction_offset,
        block_ordinal,
        opcode,
        encoding,
        explicit_definition_count,
        operands,
        implicit_definitions,
        implicit_uses,
        branch_kind,
        branch_target,
        flags,
        memory_access,
    })
}

fn decode_register_set(
    input: &mut TraceReader<'_>,
) -> Result<Vec<String>, PhysicalMachineTraceEvidenceErrorV1> {
    let count = input.u16()? as usize;
    if count > MAX_PHYSICAL_MACHINE_TRACE_REGISTERS_V1 {
        return Err(PhysicalMachineTraceEvidenceErrorV1::RegisterCount);
    }
    let mut registers = Vec::with_capacity(count);
    for _ in 0..count {
        registers.push(input.token()?);
    }
    if !strictly_sorted(&registers) {
        return Err(PhysicalMachineTraceEvidenceErrorV1::NonCanonicalOrder);
    }
    Ok(registers)
}

fn validate_instruction_shape(
    branch_kind: PhysicalMachineBranchKindV1,
    flags: PhysicalMachineInstructionFlagsV1,
    memory_access: PhysicalMachineMemoryAccessV1,
) -> Result<(), PhysicalMachineTraceEvidenceErrorV1> {
    // LLVM's MC `Barrier` flag is a scheduling/control-flow barrier and is
    // commonly set on branches. Workgroup synchronization opcodes are rejected
    // separately by the native analyzer's closed side-effect policy.
    if flags.may_load() && flags.may_store() {
        return Err(PhysicalMachineTraceEvidenceErrorV1::InvalidInstructionFlags);
    }
    if flags.may_trap()
        && (branch_kind != PhysicalMachineBranchKindV1::None
            || memory_access != PhysicalMachineMemoryAccessV1::None)
    {
        return Err(PhysicalMachineTraceEvidenceErrorV1::InvalidInstructionFlags);
    }
    match memory_access {
        PhysicalMachineMemoryAccessV1::None => {
            if flags.may_load() || flags.may_store() {
                return Err(PhysicalMachineTraceEvidenceErrorV1::InvalidMemoryAccess);
            }
        }
        PhysicalMachineMemoryAccessV1::Read { .. } => {
            if !flags.may_load() || flags.may_store() {
                return Err(PhysicalMachineTraceEvidenceErrorV1::InvalidMemoryAccess);
            }
        }
        PhysicalMachineMemoryAccessV1::Write { .. } => {
            if flags.may_load() || !flags.may_store() {
                return Err(PhysicalMachineTraceEvidenceErrorV1::InvalidMemoryAccess);
            }
        }
    }
    let terminal = matches!(
        branch_kind,
        PhysicalMachineBranchKindV1::ConditionalDirect
            | PhysicalMachineBranchKindV1::UnconditionalDirect
            | PhysicalMachineBranchKindV1::Return
    );
    if terminal != flags.is_terminator() {
        return Err(PhysicalMachineTraceEvidenceErrorV1::InvalidInstructionFlags);
    }
    Ok(())
}

fn validate_trace(
    request: &PhysicalMachineEffectRequestV1,
    effects: &PhysicalMachineEffectEvidenceV1,
    blocks: &[PhysicalMachineBasicBlockTraceV1],
    instructions: &[PhysicalMachineInstructionTraceV1],
) -> Result<(), PhysicalMachineTraceEvidenceErrorV1> {
    for (matches, field) in [
        (
            effects.execution_challenge() == request.execution_challenge(),
            "effect execution challenge",
        ),
        (
            effects.request_identity() == request.identity(),
            "effect request identity",
        ),
        (
            effects.payload_identity() == request.payload_identity(),
            "effect payload identity",
        ),
        (
            effects.analyzer_identity() == request.analyzer_identity(),
            "effect analyzer identity",
        ),
        (
            effects.toolchain_identity() == request.toolchain_identity(),
            "effect toolchain identity",
        ),
    ] {
        if !matches {
            return Err(PhysicalMachineTraceEvidenceErrorV1::IdentityMismatch(field));
        }
    }

    let functions = effects
        .functions()
        .iter()
        .map(|function| (function.symbol(), function))
        .collect::<BTreeMap<_, _>>();
    let mut blocks_by_function = BTreeMap::<&str, Vec<&PhysicalMachineBasicBlockTraceV1>>::new();
    for block in blocks {
        if !functions.contains_key(block.function_symbol.as_str()) {
            return Err(PhysicalMachineTraceEvidenceErrorV1::UnknownFunction);
        }
        blocks_by_function
            .entry(&block.function_symbol)
            .or_default()
            .push(block);
    }
    let mut instructions_by_function =
        BTreeMap::<&str, Vec<&PhysicalMachineInstructionTraceV1>>::new();
    for instruction in instructions {
        if !functions.contains_key(instruction.function_symbol.as_str()) {
            return Err(PhysicalMachineTraceEvidenceErrorV1::UnknownFunction);
        }
        instructions_by_function
            .entry(&instruction.function_symbol)
            .or_default()
            .push(instruction);
    }
    if functions.keys().any(|symbol| {
        !blocks_by_function.contains_key(symbol) || !instructions_by_function.contains_key(symbol)
    }) {
        return Err(PhysicalMachineTraceEvidenceErrorV1::MissingFunctionTrace);
    }

    for (symbol, function) in &functions {
        validate_function_trace(
            request,
            function.code_offset(),
            function.code_size(),
            &blocks_by_function[symbol],
            &instructions_by_function[symbol],
        )?;
    }
    validate_direct_calls(effects, &functions, &instructions_by_function)?;
    validate_effect_correspondence(request, effects, &functions, &instructions_by_function)
}

fn validate_function_trace(
    request: &PhysicalMachineEffectRequestV1,
    code_offset: u64,
    code_size: u64,
    blocks: &[&PhysicalMachineBasicBlockTraceV1],
    instructions: &[&PhysicalMachineInstructionTraceV1],
) -> Result<(), PhysicalMachineTraceEvidenceErrorV1> {
    let code_end = code_offset
        .checked_add(code_size)
        .ok_or(PhysicalMachineTraceEvidenceErrorV1::InvalidInstructionRange)?;
    if instructions[0].instruction_offset != code_offset {
        return Err(PhysicalMachineTraceEvidenceErrorV1::IncompleteFunctionTrace);
    }
    for (index, instruction) in instructions.iter().enumerate() {
        let end = instruction
            .instruction_offset
            .checked_add(instruction.encoding.len() as u64)
            .ok_or(PhysicalMachineTraceEvidenceErrorV1::InvalidInstructionRange)?;
        if end > code_end
            || (index != 0
                && instruction.instruction_offset
                    != instructions[index - 1].instruction_offset
                        + instructions[index - 1].encoding.len() as u64)
        {
            return Err(PhysicalMachineTraceEvidenceErrorV1::InvalidInstructionRange);
        }
        let start = usize::try_from(instruction.instruction_offset)
            .map_err(|_| PhysicalMachineTraceEvidenceErrorV1::InvalidInstructionRange)?;
        let payload_end = start
            .checked_add(instruction.encoding.len())
            .ok_or(PhysicalMachineTraceEvidenceErrorV1::InvalidInstructionRange)?;
        if request.exact_payload_bytes().get(start..payload_end) != Some(&instruction.encoding) {
            return Err(PhysicalMachineTraceEvidenceErrorV1::InstructionBytesMismatch);
        }
    }
    let last_instruction = instructions
        .last()
        .ok_or(PhysicalMachineTraceEvidenceErrorV1::MissingFunctionTrace)?;
    let decoded_end = last_instruction
        .instruction_offset
        .checked_add(last_instruction.encoding.len() as u64)
        .ok_or(PhysicalMachineTraceEvidenceErrorV1::InvalidInstructionRange)?;
    let padding_start = usize::try_from(decoded_end)
        .map_err(|_| PhysicalMachineTraceEvidenceErrorV1::InvalidInstructionRange)?;
    let padding_end = usize::try_from(code_end)
        .map_err(|_| PhysicalMachineTraceEvidenceErrorV1::InvalidInstructionRange)?;
    if request
        .exact_payload_bytes()
        .get(padding_start..padding_end)
        .is_none_or(|padding| padding.iter().any(|byte| *byte != 0))
    {
        return Err(PhysicalMachineTraceEvidenceErrorV1::IncompleteFunctionTrace);
    }

    let mut instruction_cursor = 0usize;
    let block_starts = blocks
        .iter()
        .map(|block| (block.first_instruction_offset, block.ordinal))
        .collect::<BTreeMap<_, _>>();
    if block_starts.len() != blocks.len() {
        return Err(PhysicalMachineTraceEvidenceErrorV1::InvalidBlock);
    }
    for (expected_ordinal, block) in blocks.iter().enumerate() {
        if block.ordinal != expected_ordinal as u32
            || instruction_cursor >= instructions.len()
            || block.first_instruction_offset != instructions[instruction_cursor].instruction_offset
        {
            return Err(PhysicalMachineTraceEvidenceErrorV1::InvalidBlock);
        }
        let block_end = instruction_cursor
            .checked_add(block.instruction_count as usize)
            .ok_or(PhysicalMachineTraceEvidenceErrorV1::InvalidBlock)?;
        if block_end > instructions.len()
            || instructions[instruction_cursor..block_end]
                .iter()
                .any(|instruction| instruction.block_ordinal != block.ordinal)
        {
            return Err(PhysicalMachineTraceEvidenceErrorV1::InvalidBlock);
        }
        if instructions[instruction_cursor..block_end - 1]
            .iter()
            .any(|instruction| instruction.branch_kind != PhysicalMachineBranchKindV1::None)
        {
            return Err(PhysicalMachineTraceEvidenceErrorV1::InvalidControlFlow);
        }
        let last = instructions[block_end - 1];
        let fallthrough =
            (expected_ordinal + 1 < blocks.len()).then_some((expected_ordinal + 1) as u32);
        let mut expected_successors = BTreeSet::new();
        match last.branch_kind {
            PhysicalMachineBranchKindV1::None | PhysicalMachineBranchKindV1::DirectCall => {
                expected_successors.insert(
                    fallthrough.ok_or(PhysicalMachineTraceEvidenceErrorV1::InvalidControlFlow)?,
                );
            }
            PhysicalMachineBranchKindV1::ConditionalDirect => {
                let branch_target = last
                    .branch_target
                    .ok_or(PhysicalMachineTraceEvidenceErrorV1::InvalidControlFlow)?;
                let target = *block_starts
                    .get(&branch_target)
                    .ok_or(PhysicalMachineTraceEvidenceErrorV1::InvalidControlFlow)?;
                expected_successors.insert(target);
                expected_successors.insert(
                    fallthrough.ok_or(PhysicalMachineTraceEvidenceErrorV1::InvalidControlFlow)?,
                );
            }
            PhysicalMachineBranchKindV1::UnconditionalDirect => {
                let branch_target = last
                    .branch_target
                    .ok_or(PhysicalMachineTraceEvidenceErrorV1::InvalidControlFlow)?;
                let target = *block_starts
                    .get(&branch_target)
                    .ok_or(PhysicalMachineTraceEvidenceErrorV1::InvalidControlFlow)?;
                expected_successors.insert(target);
            }
            PhysicalMachineBranchKindV1::Return => {}
        }
        if block.successors.iter().copied().collect::<BTreeSet<_>>() != expected_successors {
            return Err(PhysicalMachineTraceEvidenceErrorV1::InvalidControlFlow);
        }
        instruction_cursor = block_end;
    }
    if instruction_cursor != instructions.len() {
        return Err(PhysicalMachineTraceEvidenceErrorV1::InvalidBlock);
    }

    let mut reachable = BTreeSet::new();
    let mut pending = vec![0_u32];
    while let Some(ordinal) = pending.pop() {
        if !reachable.insert(ordinal) {
            continue;
        }
        let block = blocks
            .get(ordinal as usize)
            .ok_or(PhysicalMachineTraceEvidenceErrorV1::InvalidControlFlow)?;
        pending.extend(block.successors.iter().copied());
    }
    if reachable.len() != blocks.len() {
        return Err(PhysicalMachineTraceEvidenceErrorV1::UnreachableBlock);
    }
    Ok(())
}

fn validate_direct_calls<'a>(
    effects: &PhysicalMachineEffectEvidenceV1,
    functions: &BTreeMap<&'a str, &'a crate::PhysicalMachineFunctionEvidenceV1>,
    instructions: &BTreeMap<&str, Vec<&PhysicalMachineInstructionTraceV1>>,
) -> Result<(), PhysicalMachineTraceEvidenceErrorV1> {
    let by_offset = functions
        .values()
        .map(|function| (function.code_offset(), function.symbol()))
        .collect::<BTreeMap<_, _>>();
    if by_offset.len() != functions.len() {
        return Err(PhysicalMachineTraceEvidenceErrorV1::AmbiguousFunctionAddress);
    }
    for function in effects.functions() {
        let mut callees = BTreeSet::new();
        for instruction in &instructions[function.symbol()] {
            if instruction.branch_kind != PhysicalMachineBranchKindV1::DirectCall {
                continue;
            }
            let branch_target = instruction
                .branch_target
                .ok_or(PhysicalMachineTraceEvidenceErrorV1::InvalidDirectCall)?;
            let callee = by_offset
                .get(&branch_target)
                .ok_or(PhysicalMachineTraceEvidenceErrorV1::InvalidDirectCall)?;
            callees.insert(*callee);
        }
        if callees
            != function
                .direct_callees()
                .iter()
                .map(String::as_str)
                .collect::<BTreeSet<_>>()
        {
            return Err(PhysicalMachineTraceEvidenceErrorV1::InvalidDirectCall);
        }
    }
    Ok(())
}

fn validate_effect_correspondence(
    request: &PhysicalMachineEffectRequestV1,
    effects: &PhysicalMachineEffectEvidenceV1,
    functions: &BTreeMap<&str, &crate::PhysicalMachineFunctionEvidenceV1>,
    instructions: &BTreeMap<&str, Vec<&PhysicalMachineInstructionTraceV1>>,
) -> Result<(), PhysicalMachineTraceEvidenceErrorV1> {
    type EffectKey<'a> = (&'a str, &'a str, u64, u8, u16);
    let actual = effects
        .effects()
        .iter()
        .map(|effect| {
            (
                effect.entry_symbol(),
                effect.function_symbol(),
                effect.instruction_offset(),
                effect_kind_tag(effect.kind()),
                effect.byte_width(),
            )
        })
        .collect::<Vec<EffectKey<'_>>>();
    let mut expected = Vec::new();
    for entry in request.entries() {
        let mut closure = BTreeSet::new();
        let mut pending = vec![entry.symbol()];
        while let Some(symbol) = pending.pop() {
            if !closure.insert(symbol) {
                continue;
            }
            let function = functions
                .get(symbol)
                .ok_or(PhysicalMachineTraceEvidenceErrorV1::UnknownFunction)?;
            pending.extend(function.direct_callees().iter().map(String::as_str));
        }
        for function_symbol in closure {
            for instruction in &instructions[function_symbol] {
                match instruction.memory_access {
                    PhysicalMachineMemoryAccessV1::None => {}
                    PhysicalMachineMemoryAccessV1::Read { byte_width } => {
                        expected.push((
                            entry.symbol(),
                            function_symbol,
                            instruction.instruction_offset,
                            1,
                            8,
                        ));
                        expected.push((
                            entry.symbol(),
                            function_symbol,
                            instruction.instruction_offset,
                            2,
                            byte_width,
                        ));
                    }
                    PhysicalMachineMemoryAccessV1::Write { byte_width } => {
                        expected.push((
                            entry.symbol(),
                            function_symbol,
                            instruction.instruction_offset,
                            1,
                            8,
                        ));
                        expected.push((
                            entry.symbol(),
                            function_symbol,
                            instruction.instruction_offset,
                            3,
                            byte_width,
                        ));
                    }
                }
                if instruction.branch_kind == PhysicalMachineBranchKindV1::Return {
                    expected.push((
                        entry.symbol(),
                        function_symbol,
                        instruction.instruction_offset,
                        4,
                        0,
                    ));
                }
            }
        }
    }
    expected.sort_unstable();
    if actual != expected {
        return Err(PhysicalMachineTraceEvidenceErrorV1::EffectTraceMismatch);
    }
    Ok(())
}

const fn effect_kind_tag(kind: PhysicalMachineEffectKindV1) -> u8 {
    match kind {
        PhysicalMachineEffectKindV1::GlobalAddress => 1,
        PhysicalMachineEffectKindV1::GlobalRead => 2,
        PhysicalMachineEffectKindV1::GlobalWrite => 3,
        PhysicalMachineEffectKindV1::Return => 4,
    }
}

fn block_key(block: &PhysicalMachineBasicBlockTraceV1) -> (&str, u32) {
    (&block.function_symbol, block.ordinal)
}

fn instruction_key(instruction: &PhysicalMachineInstructionTraceV1) -> (&str, u64) {
    (&instruction.function_symbol, instruction.instruction_offset)
}

fn require_identity(
    input: &mut TraceReader<'_>,
    expected_sha256: [u8; 32],
    expected_len: u64,
    field: &'static str,
) -> Result<(), PhysicalMachineTraceEvidenceErrorV1> {
    require_equal(input.array()?, expected_sha256, field)?;
    if input.u64()? != expected_len {
        return Err(PhysicalMachineTraceEvidenceErrorV1::IdentityMismatch(field));
    }
    Ok(())
}

fn require_equal<T: Eq>(
    actual: T,
    expected: T,
    field: &'static str,
) -> Result<(), PhysicalMachineTraceEvidenceErrorV1> {
    if actual != expected {
        return Err(PhysicalMachineTraceEvidenceErrorV1::IdentityMismatch(field));
    }
    Ok(())
}

fn domain_hash(domain: &[u8], bytes: &[u8]) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(domain);
    digest.update(bytes);
    digest.finalize().into()
}

fn strictly_sorted<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn valid_machine_token(value: &str) -> bool {
    let bytes = value.as_bytes();
    !bytes.is_empty()
        && bytes.len() <= MAX_MACHINE_TOKEN_BYTES_V1
        && (bytes[0].is_ascii_alphabetic() || matches!(bytes[0], b'_' | b'.' | b'$'))
        && bytes
            .iter()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.' | b'$'))
}

struct TraceReader<'a> {
    bytes: &'a [u8],
    position: usize,
}

impl<'a> TraceReader<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, position: 0 }
    }

    fn take(&mut self, count: usize) -> Result<&'a [u8], PhysicalMachineTraceEvidenceErrorV1> {
        let end = self
            .position
            .checked_add(count)
            .ok_or(PhysicalMachineTraceEvidenceErrorV1::Truncated)?;
        let value = self
            .bytes
            .get(self.position..end)
            .ok_or(PhysicalMachineTraceEvidenceErrorV1::Truncated)?;
        self.position = end;
        Ok(value)
    }

    fn expect(&mut self, expected: &[u8]) -> Result<(), PhysicalMachineTraceEvidenceErrorV1> {
        if self.take(expected.len())? != expected {
            return Err(PhysicalMachineTraceEvidenceErrorV1::DomainMismatch);
        }
        Ok(())
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], PhysicalMachineTraceEvidenceErrorV1> {
        self.take(N)?
            .try_into()
            .map_err(|_| PhysicalMachineTraceEvidenceErrorV1::Truncated)
    }

    fn u8(&mut self) -> Result<u8, PhysicalMachineTraceEvidenceErrorV1> {
        Ok(self.take(1)?[0])
    }

    fn u16(&mut self) -> Result<u16, PhysicalMachineTraceEvidenceErrorV1> {
        Ok(u16::from_le_bytes(self.array()?))
    }

    fn u32(&mut self) -> Result<u32, PhysicalMachineTraceEvidenceErrorV1> {
        Ok(u32::from_le_bytes(self.array()?))
    }

    fn u64(&mut self) -> Result<u64, PhysicalMachineTraceEvidenceErrorV1> {
        Ok(u64::from_le_bytes(self.array()?))
    }

    fn token(&mut self) -> Result<String, PhysicalMachineTraceEvidenceErrorV1> {
        let len = self.u16()? as usize;
        let value = std::str::from_utf8(self.take(len)?)
            .map_err(|_| PhysicalMachineTraceEvidenceErrorV1::InvalidToken)?;
        if !valid_machine_token(value) {
            return Err(PhysicalMachineTraceEvidenceErrorV1::InvalidToken);
        }
        Ok(value.to_string())
    }

    fn finish(self) -> Result<(), PhysicalMachineTraceEvidenceErrorV1> {
        if self.position != self.bytes.len() {
            return Err(PhysicalMachineTraceEvidenceErrorV1::TrailingBytes);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum PhysicalMachineTraceEvidenceErrorV1 {
    RecordTooLarge,
    DomainMismatch,
    LengthMismatch,
    UnsupportedVersion,
    TargetMismatch,
    Truncated,
    TrailingBytes,
    IdentityMismatch(&'static str),
    BlockCount,
    InstructionCount,
    OperandCount,
    RegisterCount,
    InvalidToken,
    UnknownOperandKind,
    InvalidOperand,
    UnknownBranchKind,
    InvalidInstructionFlags,
    InvalidMemoryAccess,
    NonCanonicalOrder,
    UnknownFunction,
    MissingFunctionTrace,
    InvalidBlock,
    InvalidInstruction,
    InvalidInstructionRange,
    InstructionBytesMismatch,
    IncompleteFunctionTrace,
    InvalidControlFlow,
    UnreachableBlock,
    AmbiguousFunctionAddress,
    InvalidDirectCall,
    EffectTraceMismatch,
}

impl fmt::Display for PhysicalMachineTraceEvidenceErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RecordTooLarge => formatter.write_str("machine trace exceeds its byte bound"),
            Self::DomainMismatch => formatter.write_str("machine trace domain mismatch"),
            Self::LengthMismatch => formatter.write_str("machine trace length mismatch"),
            Self::UnsupportedVersion => formatter.write_str("unsupported machine trace version"),
            Self::TargetMismatch => formatter.write_str("machine trace target mismatch"),
            Self::Truncated => formatter.write_str("truncated machine trace"),
            Self::TrailingBytes => formatter.write_str("machine trace has trailing bytes"),
            Self::IdentityMismatch(field) => write!(formatter, "machine trace {field} mismatch"),
            Self::BlockCount => formatter.write_str("machine trace block count is outside bounds"),
            Self::InstructionCount => {
                formatter.write_str("machine trace instruction count is outside bounds")
            }
            Self::OperandCount => {
                formatter.write_str("machine trace operand count is outside bounds")
            }
            Self::RegisterCount => {
                formatter.write_str("machine trace register count is outside bounds")
            }
            Self::InvalidToken => formatter.write_str("machine trace token is invalid"),
            Self::UnknownOperandKind => formatter.write_str("unknown machine operand kind"),
            Self::InvalidOperand => formatter.write_str("machine operand is invalid"),
            Self::UnknownBranchKind => formatter.write_str("unknown machine branch kind"),
            Self::InvalidInstructionFlags => {
                formatter.write_str("machine instruction flags are inconsistent")
            }
            Self::InvalidMemoryAccess => {
                formatter.write_str("machine memory access is inconsistent")
            }
            Self::NonCanonicalOrder => formatter.write_str("machine trace order is noncanonical"),
            Self::UnknownFunction => formatter.write_str("machine trace names an unknown function"),
            Self::MissingFunctionTrace => {
                formatter.write_str("machine trace omits a reachable function")
            }
            Self::InvalidBlock => formatter.write_str("machine trace basic block is invalid"),
            Self::InvalidInstruction => formatter.write_str("machine instruction is invalid"),
            Self::InvalidInstructionRange => {
                formatter.write_str("machine instruction range is invalid")
            }
            Self::InstructionBytesMismatch => {
                formatter.write_str("machine instruction bytes differ from the exact payload")
            }
            Self::IncompleteFunctionTrace => {
                formatter.write_str("machine trace does not cover the complete function")
            }
            Self::InvalidControlFlow => formatter.write_str("machine control flow is invalid"),
            Self::UnreachableBlock => {
                formatter.write_str("machine trace contains an unreachable block")
            }
            Self::AmbiguousFunctionAddress => {
                formatter.write_str("machine function address is ambiguous")
            }
            Self::InvalidDirectCall => formatter.write_str("machine direct call is invalid"),
            Self::EffectTraceMismatch => {
                formatter.write_str("machine instruction trace and effect evidence differ")
            }
        }
    }
}

impl Error for PhysicalMachineTraceEvidenceErrorV1 {}
