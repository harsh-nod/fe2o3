//! Independent inert V32 program descriptions extending only the reviewed V30
//! grammar. Descriptors are source packing, not native words. Complete IDs and
//! checked borrows establish local shape only, never source occurrence custody,
//! target availability, physical values, artifact or resume authority.
//! Five physical declarations belong to the call, not shared callee identity.

use super::*;

pub const SEMANTIC_GFX942_U32_PROGRAM_MAX_STEPS_V32: usize = 16;

/// Logical operand roles; the first three roles are immutable live-ins.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum SemanticGfx942ProgramRoleV32 {
    Input0 = 0,
    Input1 = 1,
    Input2 = 2,
    Scratch = 3,
    Output = 4,
}

impl SemanticGfx942ProgramRoleV32 {
    const fn from_tag(tag: u8) -> Option<Self> {
        match tag {
            0 => Some(Self::Input0),
            1 => Some(Self::Input1),
            2 => Some(Self::Input2),
            3 => Some(Self::Scratch),
            4 => Some(Self::Output),
            _ => None,
        }
    }

    const fn bit(self) -> u8 {
        1 << self as u8
    }
}

/// The only writable roles. An input destination has no typed representation.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SemanticGfx942ProgramDestinationV32 {
    Scratch,
    Output,
}

impl SemanticGfx942ProgramDestinationV32 {
    pub const fn role(self) -> SemanticGfx942ProgramRoleV32 {
        match self {
            Self::Scratch => SemanticGfx942ProgramRoleV32::Scratch,
            Self::Output => SemanticGfx942ProgramRoleV32::Output,
        }
    }

    const fn descriptor_bit(self) -> u16 {
        match self {
            Self::Scratch => 0,
            Self::Output => 1 << 3,
        }
    }
}

/// Logical binary meanings, not an assertion about native opcode encodings.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum SemanticGfx942ProgramBinaryOpcodeV32 {
    Add = 1,
    Subtract = 2,
    And = 3,
    Or = 4,
    Xor = 5,
}

impl SemanticGfx942ProgramBinaryOpcodeV32 {
    const fn from_tag(tag: u8) -> Option<Self> {
        match tag {
            1 => Some(Self::Add),
            2 => Some(Self::Subtract),
            3 => Some(Self::And),
            4 => Some(Self::Or),
            5 => Some(Self::Xor),
            _ => None,
        }
    }
}

/// Structurally checked arity/roles. This alone does not prove initialization.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SemanticGfx942ProgramInstructionV32 {
    Move {
        destination: SemanticGfx942ProgramDestinationV32,
        source: SemanticGfx942ProgramRoleV32,
    },
    Binary {
        opcode: SemanticGfx942ProgramBinaryOpcodeV32,
        destination: SemanticGfx942ProgramDestinationV32,
        left: SemanticGfx942ProgramRoleV32,
        right: SemanticGfx942ProgramRoleV32,
    },
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
enum SemanticGfx942ProgramDescriptorErrorV32 {
    ReservedBits { bits: u16 },
    Opcode { tag: u8 },
    Source { operand: u8, tag: u8 },
    MoveUnusedSource { tag: u8 },
}

impl SemanticGfx942ProgramInstructionV32 {
    /// Bits 0..2 opcode, bit 3 destination, bits 4..6 source0, bits 7..9 source1.
    /// A move has opcode 0 and canonical zero source1; bits 10..15 are zero.
    pub const fn descriptor(self) -> u16 {
        match self {
            Self::Move {
                destination,
                source,
            } => destination.descriptor_bit() | ((source as u16) << 4),
            Self::Binary {
                opcode,
                destination,
                left,
                right,
            } => {
                opcode as u16
                    | destination.descriptor_bit()
                    | ((left as u16) << 4)
                    | ((right as u16) << 7)
            }
        }
    }

    fn from_descriptor(word: u16) -> Result<Self, SemanticGfx942ProgramDescriptorErrorV32> {
        let reserved = word & 0xfc00;
        if reserved != 0 {
            return Err(SemanticGfx942ProgramDescriptorErrorV32::ReservedBits { bits: reserved });
        }
        let opcode = (word & 7) as u8;
        if opcode > 5 {
            return Err(SemanticGfx942ProgramDescriptorErrorV32::Opcode { tag: opcode });
        }
        let destination = if word & 8 == 0 {
            SemanticGfx942ProgramDestinationV32::Scratch
        } else {
            SemanticGfx942ProgramDestinationV32::Output
        };
        let left_tag = ((word >> 4) & 7) as u8;
        let left = SemanticGfx942ProgramRoleV32::from_tag(left_tag).ok_or(
            SemanticGfx942ProgramDescriptorErrorV32::Source {
                operand: 0,
                tag: left_tag,
            },
        )?;
        let right_tag = ((word >> 7) & 7) as u8;
        if opcode == 0 {
            if right_tag != 0 {
                return Err(SemanticGfx942ProgramDescriptorErrorV32::MoveUnusedSource {
                    tag: right_tag,
                });
            }
            Ok(Self::Move {
                destination,
                source: left,
            })
        } else {
            let right = SemanticGfx942ProgramRoleV32::from_tag(right_tag).ok_or(
                SemanticGfx942ProgramDescriptorErrorV32::Source {
                    operand: 1,
                    tag: right_tag,
                },
            )?;
            Ok(Self::Binary {
                opcode: SemanticGfx942ProgramBinaryOpcodeV32::from_tag(opcode)
                    .expect("checked binary opcode"),
                destination,
                left,
                right,
            })
        }
    }

    pub const fn destination(self) -> SemanticGfx942ProgramDestinationV32 {
        match self {
            Self::Move { destination, .. } | Self::Binary { destination, .. } => destination,
        }
    }

    fn required_roles(self) -> u8 {
        match self {
            Self::Move { source, .. } => source.bit(),
            Self::Binary { left, right, .. } => left.bit() | right.bit(),
        }
    }
}

/// Pure bit packing, without validation or authority; slot 0 occupies low bits.
const fn pack_program_descriptors_v32(
    words: [u16; SEMANTIC_GFX942_U32_PROGRAM_MAX_STEPS_V32],
) -> [u64; 4] {
    let mut packed = [0_u64; 4];
    let mut index = 0;
    while index < SEMANTIC_GFX942_U32_PROGRAM_MAX_STEPS_V32 {
        packed[index / 4] |= (words[index] as u64) << (16 * (index % 4));
        index += 1;
    }
    packed
}

/// Pure inverse shifts, without validation or authority; never native decoding.
const fn unpack_program_descriptors_v32(
    packed: [u64; 4],
) -> [u16; SEMANTIC_GFX942_U32_PROGRAM_MAX_STEPS_V32] {
    let mut words = [0_u16; SEMANTIC_GFX942_U32_PROGRAM_MAX_STEPS_V32];
    let mut index = 0;
    while index < SEMANTIC_GFX942_U32_PROGRAM_MAX_STEPS_V32 {
        words[index] = (packed[index / 4] >> (16 * (index % 4))) as u16;
        index += 1;
    }
    words
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
enum SemanticGfx942ProgramErrorV32 {
    StepCount {
        count: usize,
    },
    Descriptor {
        step: usize,
        error: SemanticGfx942ProgramDescriptorErrorV32,
    },
    ReadBeforeDefinition {
        step: usize,
        roles: u8,
    },
    NonZeroPadding {
        step: usize,
        descriptor: u16,
    },
    OutputNotDefined,
}

/// Immutable fixed-capacity logical program, not a canonical or source owner.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SemanticGfx942U32ProgramV32 {
    count: u8,
    words: [u16; SEMANTIC_GFX942_U32_PROGRAM_MAX_STEPS_V32],
}

impl SemanticGfx942U32ProgramV32 {
    pub fn from_descriptors(
        count: u8,
        words: [u16; SEMANTIC_GFX942_U32_PROGRAM_MAX_STEPS_V32],
    ) -> Result<Self, SemanticMirErrorV1> {
        Self::checked_descriptors(count, words)
            .map_err(|_| SemanticMirErrorV1::InvalidOrderedProgramV32)
    }

    fn checked_descriptors(
        count: u8,
        words: [u16; SEMANTIC_GFX942_U32_PROGRAM_MAX_STEPS_V32],
    ) -> Result<Self, SemanticGfx942ProgramErrorV32> {
        let length = usize::from(count);
        if !(1..=SEMANTIC_GFX942_U32_PROGRAM_MAX_STEPS_V32).contains(&length) {
            return Err(SemanticGfx942ProgramErrorV32::StepCount { count: length });
        }
        let mut defined = 0b00111_u8;
        for (step, &word) in words.iter().enumerate() {
            if step >= length {
                if word != 0 {
                    return Err(SemanticGfx942ProgramErrorV32::NonZeroPadding {
                        step,
                        descriptor: word,
                    });
                }
                continue;
            }
            let instruction = SemanticGfx942ProgramInstructionV32::from_descriptor(word)
                .map_err(|error| SemanticGfx942ProgramErrorV32::Descriptor { step, error })?;
            // Both reads must exist before this step's destination is defined.
            let undefined = instruction.required_roles() & !defined;
            if undefined != 0 {
                return Err(SemanticGfx942ProgramErrorV32::ReadBeforeDefinition {
                    step,
                    roles: undefined,
                });
            }
            defined |= instruction.destination().role().bit();
        }
        if defined & SemanticGfx942ProgramRoleV32::Output.bit() == 0 {
            return Err(SemanticGfx942ProgramErrorV32::OutputNotDefined);
        }
        Ok(Self { count, words })
    }

    pub fn from_packed(count: u8, packed: [u64; 4]) -> Result<Self, SemanticMirErrorV1> {
        Self::from_descriptors(count, unpack_program_descriptors_v32(packed))
    }

    pub fn from_instructions(
        instructions: &[SemanticGfx942ProgramInstructionV32],
    ) -> Result<Self, SemanticMirErrorV1> {
        let length = instructions.len();
        if !(1..=SEMANTIC_GFX942_U32_PROGRAM_MAX_STEPS_V32).contains(&length) {
            return Err(SemanticMirErrorV1::InvalidOrderedProgramV32);
        }
        let mut words = [0_u16; SEMANTIC_GFX942_U32_PROGRAM_MAX_STEPS_V32];
        for (slot, instruction) in words.iter_mut().zip(instructions) {
            *slot = instruction.descriptor();
        }
        Self::from_descriptors(length as u8, words)
    }

    pub const fn count(&self) -> u8 {
        self.count
    }

    /// Includes canonical zero padding, which is part of the exact value.
    pub const fn descriptors(&self) -> &[u16; SEMANTIC_GFX942_U32_PROGRAM_MAX_STEPS_V32] {
        &self.words
    }

    pub fn active_descriptors(&self) -> &[u16] {
        &self.words[..usize::from(self.count)]
    }

    pub fn instructions(
        &self,
    ) -> impl ExactSizeIterator<Item = SemanticGfx942ProgramInstructionV32> + '_ {
        self.active_descriptors().iter().copied().map(|word| {
            SemanticGfx942ProgramInstructionV32::from_descriptor(word)
                .expect("private validated descriptor invariant")
        })
    }

    pub const fn packed_words(&self) -> [u64; 4] {
        pack_program_descriptors_v32(self.words)
    }

    /// At most 16 logical steps. Local array entries are not physical VGPR state.
    /// Every source is read before assigning this step's destination.
    pub fn evaluate(&self, inputs: [u32; 3]) -> u32 {
        let mut values = [inputs[0], inputs[1], inputs[2], 0, 0];
        for instruction in self.instructions() {
            let value = match instruction {
                SemanticGfx942ProgramInstructionV32::Move { source, .. } => values[source as usize],
                SemanticGfx942ProgramInstructionV32::Binary {
                    opcode,
                    left,
                    right,
                    ..
                } => {
                    let lhs = values[left as usize];
                    let rhs = values[right as usize];
                    match opcode {
                        SemanticGfx942ProgramBinaryOpcodeV32::Add => lhs.wrapping_add(rhs),
                        SemanticGfx942ProgramBinaryOpcodeV32::Subtract => lhs.wrapping_sub(rhs),
                        SemanticGfx942ProgramBinaryOpcodeV32::And => lhs & rhs,
                        SemanticGfx942ProgramBinaryOpcodeV32::Or => lhs | rhs,
                        SemanticGfx942ProgramBinaryOpcodeV32::Xor => lhs ^ rhs,
                    }
                }
            };
            values[instruction.destination().role() as usize] = value;
        }
        values[SemanticGfx942ProgramRoleV32::Output as usize]
    }
}

/// Checked local bindings, not a register file or lifetime/occupancy proof.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SemanticGfx942OrderedProgramRegistersV32 {
    scratch: u8,
    output: u8,
    inputs: [u8; 3],
}

impl SemanticGfx942OrderedProgramRegistersV32 {
    pub fn new(scratch: u8, output: u8, inputs: [u8; 3]) -> Result<Self, SemanticMirErrorV1> {
        let indices = [scratch, output, inputs[0], inputs[1], inputs[2]];
        for (position, index) in indices.into_iter().enumerate() {
            if index > 63 || indices[..position].contains(&index) {
                return Err(SemanticMirErrorV1::InvalidOrderedProgramV32);
            }
        }
        Ok(Self {
            scratch,
            output,
            inputs,
        })
    }

    pub const fn scratch(self) -> u8 {
        self.scratch
    }

    pub const fn output(self) -> u8 {
        self.output
    }

    pub const fn inputs(self) -> [u8; 3] {
        self.inputs
    }

    /// Register-index high water only; not complete kernel resource usage.
    pub const fn vgpr_high_water(self) -> u8 {
        let mut maximum = self.scratch;
        if self.output > maximum {
            maximum = self.output;
        }
        let mut position = 0;
        while position < self.inputs.len() {
            if self.inputs[position] > maximum {
                maximum = self.inputs[position];
            }
            position += 1;
        }
        maximum + 1
    }
}

/// Four nonzero inert references for one call occurrence, never callee authority.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SemanticOrderedProgramSourceV32 {
    frontend_unit: [u8; 32],
    function: SemanticFunctionIdentityV1,
    contract: [u8; 32],
    statement: [u8; 32],
}

impl SemanticOrderedProgramSourceV32 {
    pub fn new(
        frontend_unit: [u8; 32],
        function: SemanticFunctionIdentityV1,
        contract: [u8; 32],
        statement: [u8; 32],
    ) -> Result<Self, SemanticMirErrorV1> {
        if [frontend_unit, *function.as_bytes(), contract, statement].contains(&[0; 32]) {
            return Err(SemanticMirErrorV1::InvalidOrderedProgramV32);
        }
        Ok(Self {
            frontend_unit,
            function,
            contract,
            statement,
        })
    }

    pub const fn frontend_unit(self) -> [u8; 32] {
        self.frontend_unit
    }

    pub const fn function(self) -> SemanticFunctionIdentityV1 {
        self.function
    }

    pub const fn contract(self) -> [u8; 32] {
        self.contract
    }

    pub const fn statement(self) -> [u8; 32] {
        self.statement
    }
}

/// A checked borrow of one actual call in an admitted inert graph. Not a source
/// occurrence authenticator or a semantic-to-KIR lowering authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SemanticGfx942OrderedProgramCallV32<'a> {
    program: SemanticGfx942U32ProgramV32,
    source: SemanticOrderedProgramSourceV32,
    registers: SemanticGfx942OrderedProgramRegistersV32,
    inputs: &'a [SemanticOperandV1; 3],
}

impl<'a> SemanticGfx942OrderedProgramCallV32<'a> {
    pub const fn program(self) -> SemanticGfx942U32ProgramV32 {
        self.program
    }

    pub const fn source(self) -> SemanticOrderedProgramSourceV32 {
        self.source
    }

    pub const fn registers(self) -> SemanticGfx942OrderedProgramRegistersV32 {
        self.registers
    }

    pub const fn inputs(self) -> &'a [SemanticOperandV1; 3] {
        self.inputs
    }
}

impl AdmittedInertSemanticMirV1 {
    /// Selects from this immutable graph, not a caller-supplied replacement call.
    /// This validates inert shape/caller binding only. Target/root custody and
    /// occurrence replay must still be checked by the production source owner.
    pub fn checked_gfx942_ordered_program_call_v32(
        &self,
        function: SemanticFunctionIdV1,
        block: SemanticBlockIdV1,
    ) -> Result<SemanticGfx942OrderedProgramCallV32<'_>, SemanticMirErrorV1> {
        if self.wire_version != SemanticMirWireVersionV1::V32 {
            return Err(SemanticMirErrorV1::InvalidOrderedProgramV32);
        }
        let function = self
            .request
            .functions
            .get(function.0 as usize)
            .ok_or(SemanticMirErrorV1::InvalidOrderedProgramV32)?;
        let block = function
            .blocks
            .get(block.0 as usize)
            .ok_or(SemanticMirErrorV1::InvalidOrderedProgramV32)?;
        let SemanticTerminatorKindV1::Call(call) = &block.terminator.kind else {
            return Err(SemanticMirErrorV1::InvalidOrderedProgramV32);
        };
        checked_call(&self.request, function, call)
    }
}

pub(super) fn signature_matches(
    request: &InertSemanticMirRequestV1,
    abi: &SemanticFunctionAbiV1,
) -> bool {
    let inputs = abi.source_input_types();
    let output = abi.source_output_type();
    abi.canon_abi() == SemanticCanonAbiV1::Rust
        && abi.extern_abi() == SemanticExternAbiV1::Rust
        && !abi.c_variadic()
        && !abi.can_unwind()
        && inputs.len() == 8
        && inputs[..3].iter().all(|input| *input == output)
        // This local validator is also used before whole-request admission.
        // The shared scalar helper assumes that type-table bounds were checked.
        && request.types.get(output.0 as usize).is_some()
        && is_unsigned_integer_with_bits(request, output, 32)
        && inputs[3..].iter().all(|input| *input == inputs[3])
        && request.types.get(inputs[3].0 as usize).is_some()
        && is_unsigned_integer_with_bits(request, inputs[3], 8)
        && abi.arguments().len() == 8
        && abi
            .arguments()
            .iter()
            .all(|argument| matches!(argument.value().mode(), SemanticAbiPassModeV1::Direct(_)))
        && matches!(abi.return_value().mode(), SemanticAbiPassModeV1::Direct(_))
}

fn checked_call<'a>(
    request: &InertSemanticMirRequestV1,
    function: &SemanticFunctionDeclV1,
    call: &'a SemanticDirectCallV1,
) -> Result<SemanticGfx942OrderedProgramCallV32<'a>, SemanticMirErrorV1> {
    let Some(SemanticCallableDeclV1::CompilerIntrinsic {
        operation: SemanticCompilerIntrinsicOperationV1::Gfx942OrderedProgram(program),
        binding,
        ..
    }) = request.callables.get(call.callee.0 as usize)
    else {
        return Err(SemanticMirErrorV1::InvalidOrderedProgramV32);
    };
    // Defend the local invariant even against future private construction mistakes.
    SemanticGfx942U32ProgramV32::from_descriptors(program.count(), *program.descriptors())?;
    let source = call
        .ordered_program_source_v32
        .ok_or(SemanticMirErrorV1::InvalidOrderedProgramV32)?;
    SemanticOrderedProgramSourceV32::new(
        source.frontend_unit,
        source.function,
        source.contract,
        source.statement,
    )?;
    if call.inline_assembly_source_v30.is_some()
        || call.ordered_region_source_v31.is_some()
        || source.function != function.identity
        || !signature_matches(request, &binding.abi)
        || !call.variadic_argument_abis.is_empty()
        || call.arguments.len() != 8
        || !matches!(
            call.unwind,
            SemanticUnwindActionV1::Continue | SemanticUnwindActionV1::Unreachable
        )
    {
        return Err(SemanticMirErrorV1::InvalidOrderedProgramV32);
    }
    let abi_inputs = binding.abi.source_input_types();
    if call
        .arguments
        .iter()
        .zip(abi_inputs)
        .any(|(argument, expected)| argument.ty() != *expected)
    {
        return Err(SemanticMirErrorV1::InvalidOrderedProgramV32);
    }
    let inputs = call.arguments[..3]
        .try_into()
        .map_err(|_| SemanticMirErrorV1::InvalidOrderedProgramV32)?;
    let mut physical = [0_u8; 5];
    for (position, operand) in call.arguments[3..].iter().enumerate() {
        let SemanticOperandV1::Constant(SemanticConstantV1 {
            value: SemanticConstantValueV1::Scalar(value),
            ..
        }) = operand
        else {
            return Err(SemanticMirErrorV1::InvalidOrderedProgramV32);
        };
        if value.size_bytes() != 1 {
            return Err(SemanticMirErrorV1::InvalidOrderedProgramV32);
        }
        physical[position] =
            u8::try_from(value.bits()).map_err(|_| SemanticMirErrorV1::InvalidOrderedProgramV32)?;
    }
    let registers = SemanticGfx942OrderedProgramRegistersV32::new(
        physical[0],
        physical[1],
        [physical[2], physical[3], physical[4]],
    )?;
    Ok(SemanticGfx942OrderedProgramCallV32 {
        program: *program,
        source,
        registers,
        inputs,
    })
}

pub(super) fn validate_call_source(
    request: &InertSemanticMirRequestV1,
    function: &SemanticFunctionDeclV1,
    call: &SemanticDirectCallV1,
) -> Result<(), SemanticMirErrorV1> {
    let region = matches!(
        request.callables.get(call.callee.0 as usize),
        Some(SemanticCallableDeclV1::CompilerIntrinsic {
            operation: SemanticCompilerIntrinsicOperationV1::Gfx942OrderedProgram(_),
            ..
        })
    );
    if region {
        checked_call(request, function, call).map(|_| ())
    } else if call.ordered_program_source_v32.is_some() {
        Err(SemanticMirErrorV1::InvalidOrderedProgramV32)
    } else {
        Ok(())
    }
}

pub(super) fn uses_v32(request: &InertSemanticMirRequestV1) -> bool {
    request.callables.iter().any(|callable| {
        matches!(
            callable,
            SemanticCallableDeclV1::CompilerIntrinsic {
                operation: SemanticCompilerIntrinsicOperationV1::Gfx942OrderedProgram(_),
                ..
            }
        )
    }) || request.functions.iter().any(|function| {
        function.blocks.iter().any(|block| {
            matches!(&block.terminator.kind, SemanticTerminatorKindV1::Call(call)
                if call.ordered_program_source_v32.is_some())
        })
    })
}

#[cfg(test)]
#[path = "gfx942_ordered_program_v32_tests.rs"]
mod tests;
