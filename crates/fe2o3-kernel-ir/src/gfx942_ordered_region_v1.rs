//! Closed, inert contract for one gfx942 XOR-e32/add-e32 ordered region.
//!
//! This data fixes the two instructions and their region-local physical bindings.
//! It does not authenticate source references, grant hardware/resource availability,
//! prove physical lifetimes, or model a physical register file. Consumers separately
//! enforce the exact gfx942:xnack- wave64/launch and canonical-owner boundaries.

use std::fmt;

use crate::{AssemblySourceIdentity, Operation, OperationKind, ScalarType, ValueId};

/// Structural requirement only; this capability is not source or proof authority.
pub const AMDGPU_GFX942_ORDERED_REGION_CAPABILITY_NAMESPACE: &str = "amd.gfx942";
pub const AMDGPU_GFX942_ORDERED_REGION_CAPABILITY_NAME: &str = "ordered_xor_add_u32_e32_v1";

#[cfg(test)]
#[path = "gfx942_ordered_region_v1_tests.rs"]
mod tests;

/// The sole reviewed ordered instruction profile; not a caller-defined program.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Gfx942OrderedRegionProfileV1 {
    XorAddU32E32,
}

impl Gfx942OrderedRegionProfileV1 {
    /// Evaluates only the three-input U32 value abstraction.
    ///
    /// This does not simulate EXEC, physical registers, scheduling or encodings.
    pub fn evaluate_bits(self, inputs: &[u32]) -> Result<u32, Gfx942OrderedRegionErrorV1> {
        let [a, b, c] = inputs else {
            return Err(Gfx942OrderedRegionErrorV1::InputArity);
        };
        match self {
            Self::XorAddU32E32 => Ok((a ^ b).wrapping_add(*c)),
        }
    }
}

/// An exact instruction in the closed profile, including its selected e32 form.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Gfx942OrderedRegionInstructionV1 {
    VXorB32E32,
    VAddU32E32,
}

impl Gfx942OrderedRegionInstructionV1 {
    pub const fn mnemonic(self) -> &'static str {
        match self {
            Self::VXorB32E32 => "v_xor_b32_e32",
            Self::VAddU32E32 => "v_add_u32_e32",
        }
    }
}

/// One read-only step description, not independently executable region input.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Gfx942OrderedRegionStepV1 {
    instruction: Gfx942OrderedRegionInstructionV1,
    output: u8,
    inputs: [u8; 2],
}

impl Gfx942OrderedRegionStepV1 {
    pub const fn instruction(self) -> Gfx942OrderedRegionInstructionV1 {
        self.instruction
    }

    pub const fn output(self) -> u8 {
        self.output
    }

    pub const fn inputs(self) -> [u8; 2] {
        self.inputs
    }
}

/// Five distinct region-local VGPR bindings in the initial reviewed v0..v63 range.
///
/// Output is early-clobber and scratch is clobbered. Neither is a handle that can
/// escape this unit. The fixed instructions read EXEC and write no implicit state.
///
/// ```compile_fail
/// use fe2o3_kernel_ir::Gfx942OrderedRegionRegistersV1;
/// let unchecked = Gfx942OrderedRegionRegistersV1 {
///     scratch: 255, output: 255, inputs: [255; 3],
/// };
/// ```
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Gfx942OrderedRegionRegistersV1 {
    scratch: u8,
    output: u8,
    inputs: [u8; 3],
}

impl Gfx942OrderedRegionRegistersV1 {
    pub fn new(
        scratch: u8,
        output: u8,
        inputs: [u8; 3],
    ) -> Result<Self, Gfx942OrderedRegionErrorV1> {
        let registers = Self {
            scratch,
            output,
            inputs,
        };
        registers.validate()?;
        Ok(registers)
    }

    fn validate(self) -> Result<(), Gfx942OrderedRegionErrorV1> {
        let bindings = [
            self.scratch,
            self.output,
            self.inputs[0],
            self.inputs[1],
            self.inputs[2],
        ];
        if bindings.iter().any(|register| *register >= 64) {
            return Err(Gfx942OrderedRegionErrorV1::RegisterOutOfRange);
        }
        for (position, binding) in bindings.iter().enumerate() {
            if bindings[..position].contains(binding) {
                return Err(Gfx942OrderedRegionErrorV1::RegisterOverlap);
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

    /// Source-order fixed steps, with no allocation or caller-supplied opcode.
    pub const fn steps(self) -> [Gfx942OrderedRegionStepV1; 2] {
        [
            Gfx942OrderedRegionStepV1 {
                instruction: Gfx942OrderedRegionInstructionV1::VXorB32E32,
                output: self.scratch,
                inputs: [self.inputs[0], self.inputs[1]],
            },
            Gfx942OrderedRegionStepV1 {
                instruction: Gfx942OrderedRegionInstructionV1::VAddU32E32,
                output: self.output,
                inputs: [self.scratch, self.inputs[2]],
            },
        ]
    }
}

/// Fixed-size inert region payload in the normal executable Module.
///
/// Complete source IDs are necessary shape checks, never source authentication.
/// No instruction text/options/effects can be supplied. The exact profile is
/// NoMemory, reads EXEC without implicit writes, and retains an unused result's
/// unit; it is not a pure CSE/DCE expression or an external memory fence.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Gfx942OrderedRegionV1 {
    source: AssemblySourceIdentity,
    registers: Gfx942OrderedRegionRegistersV1,
    inputs: [ValueId; 3],
}

impl Gfx942OrderedRegionV1 {
    pub fn new(
        source: AssemblySourceIdentity,
        registers: Gfx942OrderedRegionRegistersV1,
        inputs: [ValueId; 3],
    ) -> Result<Self, Gfx942OrderedRegionErrorV1> {
        let region = Self {
            source,
            registers,
            inputs,
        };
        region.validate_shape()?;
        Ok(region)
    }

    pub(crate) fn validate_shape(&self) -> Result<(), Gfx942OrderedRegionErrorV1> {
        if !self.source.is_complete() {
            return Err(Gfx942OrderedRegionErrorV1::IncompleteSourceIdentity);
        }
        self.registers.validate()
    }

    pub const fn profile(&self) -> Gfx942OrderedRegionProfileV1 {
        Gfx942OrderedRegionProfileV1::XorAddU32E32
    }

    pub const fn source(&self) -> AssemblySourceIdentity {
        self.source
    }

    pub const fn registers(&self) -> Gfx942OrderedRegionRegistersV1 {
        self.registers
    }

    pub const fn inputs(&self) -> &[ValueId; 3] {
        &self.inputs
    }
}

/// Bounded contract failures without copied source text or unbounded diagnostics.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Gfx942OrderedRegionErrorV1 {
    NotOrderedRegion,
    RegisterOutOfRange,
    RegisterOverlap,
    IncompleteSourceIdentity,
    ResultArity,
    ResultType,
    InputType,
    InputArity,
}

impl fmt::Display for Gfx942OrderedRegionErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::NotOrderedRegion => "expected a gfx942 ordered region operation",
            Self::RegisterOutOfRange => "ordered region requires VGPR indices in 0..64",
            Self::RegisterOverlap => "ordered region requires five distinct VGPR bindings",
            Self::IncompleteSourceIdentity => "ordered region source references are incomplete",
            Self::ResultArity => "ordered region requires exactly one result",
            Self::ResultType => "ordered region result must be u32",
            Self::InputType => "ordered region inputs must resolve to u32",
            Self::InputArity => "ordered region evaluation requires exactly three input values",
        })
    }
}

impl std::error::Error for Gfx942OrderedRegionErrorV1 {}

/// Validated operation shape, not an authenticated source or executable owner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ValidatedGfx942OrderedRegionV1 {
    region: Gfx942OrderedRegionV1,
    result: ValueId,
}

impl ValidatedGfx942OrderedRegionV1 {
    pub const fn profile(&self) -> Gfx942OrderedRegionProfileV1 {
        self.region.profile()
    }

    pub const fn source(&self) -> AssemblySourceIdentity {
        self.region.source()
    }

    pub const fn registers(&self) -> Gfx942OrderedRegionRegistersV1 {
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
pub fn validate_gfx942_ordered_region_v1(
    operation: &Operation,
    value_type: impl Fn(ValueId) -> Option<ScalarType>,
) -> Result<ValidatedGfx942OrderedRegionV1, Gfx942OrderedRegionErrorV1> {
    let OperationKind::Gfx942OrderedRegion(region) = &operation.kind else {
        return Err(Gfx942OrderedRegionErrorV1::NotOrderedRegion);
    };
    region.validate_shape()?;
    let [result] = operation.results.as_slice() else {
        return Err(Gfx942OrderedRegionErrorV1::ResultArity);
    };
    if result.ty.as_scalar() != Some(ScalarType::U32) {
        return Err(Gfx942OrderedRegionErrorV1::ResultType);
    }
    for input in region.inputs() {
        if value_type(*input) != Some(ScalarType::U32) {
            return Err(Gfx942OrderedRegionErrorV1::InputType);
        }
    }
    Ok(ValidatedGfx942OrderedRegionV1 {
        region: *region,
        result: result.id,
    })
}
