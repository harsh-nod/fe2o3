//! Bounded inert contract for 1..=16 gfx942 logical u32 ordered instructions.
//!
//! Descriptors are canonical source packing, never native machine encodings.
//! Every authored step is retained, including dead/repeated/self writes. This
//! contract does not authenticate source IDs, observe a physical register file,
//! prove lifetimes/occupancy, or grant artifact/launch/resume authority. Exact
//! gfx942:xnack-, Wave64, launch and immutable-owner rules are enforced separately.
//! Validation uses fixed arrays and bounded loops, with no heap allocation.

use crate::{AssemblySourceIdentity, Operation, OperationKind, ScalarType, ValueId};
use std::fmt;

/// Structural requirement only, not source, target or proof authority.
pub const AMDGPU_GFX942_ORDERED_PROGRAM_CAPABILITY_NAMESPACE: &str = "amd.gfx942";
pub const AMDGPU_GFX942_ORDERED_PROGRAM_CAPABILITY_NAME: &str = "ordered_program_u32_e32_v1";

#[cfg(test)]
#[path = "gfx942_ordered_program_v1_tests.rs"]
mod tests;

pub const GFX942_U32_PROGRAM_MAX_STEPS_V1: usize = 16;

/// Logical operand roles; the first three roles are immutable live-ins.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum Gfx942ProgramRoleV1 {
    Input0 = 0,
    Input1 = 1,
    Input2 = 2,
    Scratch = 3,
    Output = 4,
}

impl Gfx942ProgramRoleV1 {
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
pub enum Gfx942ProgramDestinationV1 {
    Scratch,
    Output,
}

impl Gfx942ProgramDestinationV1 {
    pub const fn role(self) -> Gfx942ProgramRoleV1 {
        match self {
            Self::Scratch => Gfx942ProgramRoleV1::Scratch,
            Self::Output => Gfx942ProgramRoleV1::Output,
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
pub enum Gfx942ProgramBinaryOpcodeV1 {
    Add = 1,
    Subtract = 2,
    And = 3,
    Or = 4,
    Xor = 5,
}

impl Gfx942ProgramBinaryOpcodeV1 {
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
pub enum Gfx942ProgramInstructionV1 {
    Move {
        destination: Gfx942ProgramDestinationV1,
        source: Gfx942ProgramRoleV1,
    },
    Binary {
        opcode: Gfx942ProgramBinaryOpcodeV1,
        destination: Gfx942ProgramDestinationV1,
        left: Gfx942ProgramRoleV1,
        right: Gfx942ProgramRoleV1,
    },
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Gfx942ProgramDescriptorErrorV1 {
    ReservedBits { bits: u16 },
    Opcode { tag: u8 },
    Source { operand: u8, tag: u8 },
    MoveUnusedSource { tag: u8 },
}

impl Gfx942ProgramInstructionV1 {
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

    pub fn from_descriptor(word: u16) -> Result<Self, Gfx942ProgramDescriptorErrorV1> {
        let reserved = word & 0xfc00;
        if reserved != 0 {
            return Err(Gfx942ProgramDescriptorErrorV1::ReservedBits { bits: reserved });
        }
        let opcode = (word & 7) as u8;
        if opcode > 5 {
            return Err(Gfx942ProgramDescriptorErrorV1::Opcode { tag: opcode });
        }
        let destination = if word & 8 == 0 {
            Gfx942ProgramDestinationV1::Scratch
        } else {
            Gfx942ProgramDestinationV1::Output
        };
        let left_tag = ((word >> 4) & 7) as u8;
        let left = Gfx942ProgramRoleV1::from_tag(left_tag).ok_or(
            Gfx942ProgramDescriptorErrorV1::Source {
                operand: 0,
                tag: left_tag,
            },
        )?;
        let right_tag = ((word >> 7) & 7) as u8;
        if opcode == 0 {
            if right_tag != 0 {
                return Err(Gfx942ProgramDescriptorErrorV1::MoveUnusedSource { tag: right_tag });
            }
            Ok(Self::Move {
                destination,
                source: left,
            })
        } else {
            let right = Gfx942ProgramRoleV1::from_tag(right_tag).ok_or(
                Gfx942ProgramDescriptorErrorV1::Source {
                    operand: 1,
                    tag: right_tag,
                },
            )?;
            Ok(Self::Binary {
                opcode: Gfx942ProgramBinaryOpcodeV1::from_tag(opcode)
                    .expect("checked binary opcode"),
                destination,
                left,
                right,
            })
        }
    }

    pub const fn destination(self) -> Gfx942ProgramDestinationV1 {
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
const fn pack_program_descriptors_v1(words: [u16; GFX942_U32_PROGRAM_MAX_STEPS_V1]) -> [u64; 4] {
    let mut packed = [0_u64; 4];
    let mut index = 0;
    while index < GFX942_U32_PROGRAM_MAX_STEPS_V1 {
        packed[index / 4] |= (words[index] as u64) << (16 * (index % 4));
        index += 1;
    }
    packed
}

/// Pure inverse shifts, without validation or authority; never native decoding.
const fn unpack_program_descriptors_v1(packed: [u64; 4]) -> [u16; GFX942_U32_PROGRAM_MAX_STEPS_V1] {
    let mut words = [0_u16; GFX942_U32_PROGRAM_MAX_STEPS_V1];
    let mut index = 0;
    while index < GFX942_U32_PROGRAM_MAX_STEPS_V1 {
        words[index] = (packed[index / 4] >> (16 * (index % 4))) as u16;
        index += 1;
    }
    words
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Gfx942ProgramErrorV1 {
    StepCount {
        count: usize,
    },
    Descriptor {
        step: usize,
        error: Gfx942ProgramDescriptorErrorV1,
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
///
/// ```compile_fail
/// use fe2o3_kernel_ir::Gfx942U32ProgramV1;
/// let unchecked = Gfx942U32ProgramV1 { count: 0, words: [0; 16] };
/// ```
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Gfx942U32ProgramV1 {
    count: u8,
    words: [u16; GFX942_U32_PROGRAM_MAX_STEPS_V1],
}

impl Gfx942U32ProgramV1 {
    pub fn from_descriptors(
        count: u8,
        words: [u16; GFX942_U32_PROGRAM_MAX_STEPS_V1],
    ) -> Result<Self, Gfx942ProgramErrorV1> {
        let length = usize::from(count);
        if !(1..=GFX942_U32_PROGRAM_MAX_STEPS_V1).contains(&length) {
            return Err(Gfx942ProgramErrorV1::StepCount { count: length });
        }
        let mut defined = 0b00111_u8;
        for (step, &word) in words.iter().enumerate() {
            if step >= length {
                if word != 0 {
                    return Err(Gfx942ProgramErrorV1::NonZeroPadding {
                        step,
                        descriptor: word,
                    });
                }
                continue;
            }
            let instruction = Gfx942ProgramInstructionV1::from_descriptor(word)
                .map_err(|error| Gfx942ProgramErrorV1::Descriptor { step, error })?;
            // Both reads must exist before this step's destination is defined.
            let undefined = instruction.required_roles() & !defined;
            if undefined != 0 {
                return Err(Gfx942ProgramErrorV1::ReadBeforeDefinition {
                    step,
                    roles: undefined,
                });
            }
            defined |= instruction.destination().role().bit();
        }
        if defined & Gfx942ProgramRoleV1::Output.bit() == 0 {
            return Err(Gfx942ProgramErrorV1::OutputNotDefined);
        }
        Ok(Self { count, words })
    }

    pub fn from_packed(count: u8, packed: [u64; 4]) -> Result<Self, Gfx942ProgramErrorV1> {
        Self::from_descriptors(count, unpack_program_descriptors_v1(packed))
    }

    pub fn from_instructions(
        instructions: &[Gfx942ProgramInstructionV1],
    ) -> Result<Self, Gfx942ProgramErrorV1> {
        let length = instructions.len();
        if !(1..=GFX942_U32_PROGRAM_MAX_STEPS_V1).contains(&length) {
            return Err(Gfx942ProgramErrorV1::StepCount { count: length });
        }
        let mut words = [0_u16; GFX942_U32_PROGRAM_MAX_STEPS_V1];
        for (slot, instruction) in words.iter_mut().zip(instructions) {
            *slot = instruction.descriptor();
        }
        Self::from_descriptors(length as u8, words)
    }

    pub const fn count(&self) -> u8 {
        self.count
    }

    /// Includes canonical zero padding, which is part of the exact value.
    pub const fn descriptors(&self) -> &[u16; GFX942_U32_PROGRAM_MAX_STEPS_V1] {
        &self.words
    }

    pub fn active_descriptors(&self) -> &[u16] {
        &self.words[..usize::from(self.count)]
    }

    pub fn instructions(&self) -> impl ExactSizeIterator<Item = Gfx942ProgramInstructionV1> + '_ {
        self.active_descriptors().iter().copied().map(|word| {
            Gfx942ProgramInstructionV1::from_descriptor(word)
                .expect("private validated descriptor invariant")
        })
    }

    pub const fn packed_words(&self) -> [u64; 4] {
        pack_program_descriptors_v1(self.words)
    }

    /// At most 16 logical steps. Local array entries are not physical VGPR state.
    /// Every source is read before assigning this step's destination.
    pub fn evaluate(&self, inputs: [u32; 3]) -> u32 {
        let mut values = [inputs[0], inputs[1], inputs[2], 0, 0];
        for instruction in self.instructions() {
            let value = match instruction {
                Gfx942ProgramInstructionV1::Move { source, .. } => values[source as usize],
                Gfx942ProgramInstructionV1::Binary {
                    opcode,
                    left,
                    right,
                    ..
                } => {
                    let lhs = values[left as usize];
                    let rhs = values[right as usize];
                    match opcode {
                        Gfx942ProgramBinaryOpcodeV1::Add => lhs.wrapping_add(rhs),
                        Gfx942ProgramBinaryOpcodeV1::Subtract => lhs.wrapping_sub(rhs),
                        Gfx942ProgramBinaryOpcodeV1::And => lhs & rhs,
                        Gfx942ProgramBinaryOpcodeV1::Or => lhs | rhs,
                        Gfx942ProgramBinaryOpcodeV1::Xor => lhs ^ rhs,
                    }
                }
            };
            values[instruction.destination().role() as usize] = value;
        }
        values[Gfx942ProgramRoleV1::Output as usize]
    }
}

impl fmt::Display for Gfx942ProgramDescriptorErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ReservedBits { bits } => {
                write!(formatter, "reserved descriptor bits {bits:#06x}")
            }
            Self::Opcode { tag } => write!(formatter, "unsupported descriptor opcode {tag}"),
            Self::Source { operand, tag } => {
                write!(formatter, "invalid source role {tag} at operand {operand}")
            }
            Self::MoveUnusedSource { tag } => {
                write!(formatter, "move requires zero unused source, got {tag}")
            }
        }
    }
}
impl std::error::Error for Gfx942ProgramDescriptorErrorV1 {}

impl fmt::Display for Gfx942ProgramErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::StepCount { count } => write!(
                formatter,
                "ordered program requires 1..=16 steps, got {count}"
            ),
            Self::Descriptor { step, error } => write!(formatter, "step {step}: {error}"),
            Self::ReadBeforeDefinition { step, roles } => {
                write!(formatter, "step {step} reads undefined roles {roles:#04x}")
            }
            Self::NonZeroPadding { step, descriptor } => write!(
                formatter,
                "inactive step {step} has nonzero descriptor {descriptor:#06x}"
            ),
            Self::OutputNotDefined => formatter.write_str("ordered program does not define output"),
        }
    }
}
impl std::error::Error for Gfx942ProgramErrorV1 {}

/// Five distinct region-local VGPR bindings in the initial reviewed v0..v63 range.
///
/// Output is early-clobber and scratch is clobbered. Neither is a handle that can
/// escape this unit. The reviewed instructions read EXEC and write no implicit state.
///
/// ```compile_fail
/// use fe2o3_kernel_ir::Gfx942OrderedProgramRegistersV1;
/// let unchecked = Gfx942OrderedProgramRegistersV1 {
///     scratch: 255, output: 255, inputs: [255; 3],
/// };
/// ```
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Gfx942OrderedProgramRegistersV1 {
    scratch: u8,
    output: u8,
    inputs: [u8; 3],
}

impl Gfx942OrderedProgramRegistersV1 {
    pub fn new(
        scratch: u8,
        output: u8,
        inputs: [u8; 3],
    ) -> Result<Self, Gfx942OrderedProgramErrorV1> {
        let registers = Self {
            scratch,
            output,
            inputs,
        };
        registers.validate()?;
        Ok(registers)
    }

    fn validate(self) -> Result<(), Gfx942OrderedProgramErrorV1> {
        let bindings = [
            self.scratch,
            self.output,
            self.inputs[0],
            self.inputs[1],
            self.inputs[2],
        ];
        if bindings.iter().any(|register| *register >= 64) {
            return Err(Gfx942OrderedProgramErrorV1::RegisterOutOfRange);
        }
        for (position, binding) in bindings.iter().enumerate() {
            if bindings[..position].contains(binding) {
                return Err(Gfx942OrderedProgramErrorV1::RegisterOverlap);
            }
        }
        Ok(())
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

    /// Necessary VGPR high-water, not final descriptor or occupancy qualification.
    pub const fn vgpr_high_water(self) -> u8 {
        let bindings = [
            self.scratch,
            self.output,
            self.inputs[0],
            self.inputs[1],
            self.inputs[2],
        ];
        let mut highest = 0;
        let mut index = 0;
        while index < bindings.len() {
            if bindings[index] > highest {
                highest = bindings[index];
            }
            index += 1;
        }
        highest + 1
    }

    /// Maps a logical role to its declared register number, never its value.
    pub const fn binding(self, role: Gfx942ProgramRoleV1) -> u8 {
        match role {
            Gfx942ProgramRoleV1::Scratch => self.scratch,
            Gfx942ProgramRoleV1::Output => self.output,
            Gfx942ProgramRoleV1::Input0 => self.inputs[0],
            Gfx942ProgramRoleV1::Input1 => self.inputs[1],
            Gfx942ProgramRoleV1::Input2 => self.inputs[2],
        }
    }
}

/// Fixed-size inert ordered-program payload in the normal executable Module.
///
/// Complete source IDs are necessary shape checks, never source authentication.
/// No instruction text/options/effects can be supplied. The reviewed program is
/// NoMemory, reads EXEC without implicit writes, and retains an unused result's
/// unit; it is not a pure CSE/DCE expression or an external memory fence.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Gfx942OrderedProgramV1 {
    source: AssemblySourceIdentity,
    registers: Gfx942OrderedProgramRegistersV1,
    inputs: [ValueId; 3],
    program: Gfx942U32ProgramV1,
}

impl Gfx942OrderedProgramV1 {
    pub fn new(
        source: AssemblySourceIdentity,
        registers: Gfx942OrderedProgramRegistersV1,
        inputs: [ValueId; 3],
        program: Gfx942U32ProgramV1,
    ) -> Result<Self, Gfx942OrderedProgramErrorV1> {
        let region = Self {
            source,
            registers,
            inputs,
            program,
        };
        region.validate_shape()?;
        Ok(region)
    }

    pub(crate) fn validate_shape(&self) -> Result<(), Gfx942OrderedProgramErrorV1> {
        if !self.source.is_complete() {
            return Err(Gfx942OrderedProgramErrorV1::IncompleteSourceIdentity);
        }
        self.registers.validate()?;
        Gfx942U32ProgramV1::from_descriptors(self.program.count, self.program.words)
            .map_err(Gfx942OrderedProgramErrorV1::InvalidProgram)?;
        Ok(())
    }

    pub const fn program(&self) -> &Gfx942U32ProgramV1 {
        &self.program
    }

    pub const fn source(&self) -> AssemblySourceIdentity {
        self.source
    }

    pub const fn registers(&self) -> Gfx942OrderedProgramRegistersV1 {
        self.registers
    }

    pub const fn inputs(&self) -> &[ValueId; 3] {
        &self.inputs
    }
}

/// Bounded contract failures without copied source text or unbounded diagnostics.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Gfx942OrderedProgramErrorV1 {
    NotOrderedProgram,
    RegisterOutOfRange,
    RegisterOverlap,
    IncompleteSourceIdentity,
    ResultArity,
    ResultType,
    InputType,
    InvalidProgram(Gfx942ProgramErrorV1),
}

impl fmt::Display for Gfx942OrderedProgramErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Self::InvalidProgram(error) = self {
            return write!(formatter, "invalid ordered program: {error}");
        }
        formatter.write_str(match self {
            Self::NotOrderedProgram => "expected a gfx942 ordered program operation",
            Self::RegisterOutOfRange => "ordered program requires VGPR indices in 0..64",
            Self::RegisterOverlap => "ordered program requires five distinct VGPR bindings",
            Self::IncompleteSourceIdentity => "ordered program source references are incomplete",
            Self::ResultArity => "ordered program requires exactly one result",
            Self::ResultType => "ordered program result must be u32",
            Self::InputType => "ordered program inputs must resolve to u32",
            Self::InvalidProgram(_) => unreachable!("handled above"),
        })
    }
}

impl std::error::Error for Gfx942OrderedProgramErrorV1 {}

/// Validated operation shape, not an authenticated source or executable owner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ValidatedGfx942OrderedProgramV1 {
    region: Gfx942OrderedProgramV1,
    result: ValueId,
}

impl ValidatedGfx942OrderedProgramV1 {
    pub const fn program(&self) -> &Gfx942U32ProgramV1 {
        self.region.program()
    }

    pub const fn source(&self) -> AssemblySourceIdentity {
        self.region.source()
    }

    pub const fn registers(&self) -> Gfx942OrderedProgramRegistersV1 {
        self.region.registers()
    }

    pub const fn result(&self) -> ValueId {
        self.result
    }

    pub const fn inputs(&self) -> &[ValueId; 3] {
        self.region.inputs()
    }
}

/// Checks the closed operation shape without allocating or walking a Module.
///
/// The caller resolves input definitions in the containing function; missing or
/// non-scalar definitions return None. Definition/dominance, source custody,
/// target/launch, capability and retention rules remain the owners' responsibility.
pub fn validate_gfx942_ordered_program_v1(
    operation: &Operation,
    value_type: impl Fn(ValueId) -> Option<ScalarType>,
) -> Result<ValidatedGfx942OrderedProgramV1, Gfx942OrderedProgramErrorV1> {
    let OperationKind::Gfx942OrderedProgram(region) = &operation.kind else {
        return Err(Gfx942OrderedProgramErrorV1::NotOrderedProgram);
    };
    region.validate_shape()?;
    let [result] = operation.results.as_slice() else {
        return Err(Gfx942OrderedProgramErrorV1::ResultArity);
    };
    if result.ty.as_scalar() != Some(ScalarType::U32) {
        return Err(Gfx942OrderedProgramErrorV1::ResultType);
    }
    for input in region.inputs() {
        if value_type(*input) != Some(ScalarType::U32) {
            return Err(Gfx942OrderedProgramErrorV1::InputType);
        }
    }
    Ok(ValidatedGfx942OrderedProgramV1 {
        region: *region,
        result: result.id,
    })
}
