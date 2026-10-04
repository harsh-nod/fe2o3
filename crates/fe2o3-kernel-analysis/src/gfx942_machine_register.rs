//! Closed physical-register facts for the bounded gfx942 machine checker.
//!
//! LLVM MC supplies canonical register names and operand roles in the
//! authenticated trace. This module independently expands the scalar profile's
//! contiguous SGPR/VGPR aliases and special control registers into atomic
//! units. The result is derived checker input, not compiler refinement or
//! launch authority.

use crate::{PhysicalMachineInstructionTraceV1, PhysicalMachineOperandValueV1};
use std::{collections::BTreeSet, error::Error, fmt};

pub const MAX_GFX942_REGISTER_ALIAS_UNITS_V1: usize = 32;
pub const MAX_GFX942_SGPR_INDEX_V1: u16 = 127;
pub const MAX_GFX942_VGPR_INDEX_V1: u16 = 255;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Gfx942RegisterUnitV1 {
    Sgpr(u16),
    Vgpr(u16),
    ExecLow,
    ExecHigh,
    VccLow,
    VccHigh,
    Scc,
    Mode,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Gfx942RegisterAliasV1 {
    units: Vec<Gfx942RegisterUnitV1>,
}

impl Gfx942RegisterAliasV1 {
    pub fn decode(name: &str) -> Result<Self, Gfx942RegisterFactsErrorV1> {
        decode_register_alias(name)
    }

    pub fn units(&self) -> &[Gfx942RegisterUnitV1] {
        &self.units
    }

    pub fn overlaps(&self, other: &Self) -> bool {
        self.units
            .iter()
            .any(|unit| other.units.binary_search(unit).is_ok())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Gfx942InstructionRegisterFactsV1 {
    operand_aliases: Vec<Option<Gfx942RegisterAliasV1>>,
    explicit_definition_count: u16,
    implicit_definitions: Vec<Gfx942RegisterAliasV1>,
    implicit_uses: Vec<Gfx942RegisterAliasV1>,
    definitions: Vec<Gfx942RegisterUnitV1>,
    uses: Vec<Gfx942RegisterUnitV1>,
}

impl Gfx942InstructionRegisterFactsV1 {
    pub fn derive(
        instruction: &PhysicalMachineInstructionTraceV1,
    ) -> Result<Self, Gfx942RegisterFactsErrorV1> {
        derive_instruction_register_facts(instruction)
    }

    pub fn operand_aliases(&self) -> &[Option<Gfx942RegisterAliasV1>] {
        &self.operand_aliases
    }

    pub const fn explicit_definition_count(&self) -> u16 {
        self.explicit_definition_count
    }

    pub fn implicit_definitions(&self) -> &[Gfx942RegisterAliasV1] {
        &self.implicit_definitions
    }

    pub fn implicit_uses(&self) -> &[Gfx942RegisterAliasV1] {
        &self.implicit_uses
    }

    pub fn definitions(&self) -> &[Gfx942RegisterUnitV1] {
        &self.definitions
    }

    pub fn uses(&self) -> &[Gfx942RegisterUnitV1] {
        &self.uses
    }

    pub const fn establishes_machine_semantics(&self) -> bool {
        false
    }

    pub const fn establishes_compiler_refinement(&self) -> bool {
        false
    }

    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

fn derive_instruction_register_facts(
    instruction: &PhysicalMachineInstructionTraceV1,
) -> Result<Gfx942InstructionRegisterFactsV1, Gfx942RegisterFactsErrorV1> {
    let definition_count = usize::from(instruction.explicit_definition_count());
    if definition_count > instruction.operands().len() {
        return Err(Gfx942RegisterFactsErrorV1::InvalidDefinitionCount);
    }
    let mut operand_aliases = Vec::with_capacity(instruction.operands().len());
    for (index, operand) in instruction.operands().iter().enumerate() {
        let alias = match operand.value() {
            PhysicalMachineOperandValueV1::Register(name) => {
                Some(Gfx942RegisterAliasV1::decode(name)?)
            }
            _ if index < definition_count => {
                return Err(Gfx942RegisterFactsErrorV1::NonRegisterDefinition {
                    operand: index as u16,
                });
            }
            _ => None,
        };
        operand_aliases.push(alias);
    }
    for (index, operand) in instruction.operands().iter().enumerate() {
        let Some(tied) = operand.tied_to() else {
            continue;
        };
        let tied = usize::from(tied);
        if tied >= operand_aliases.len() || (index < definition_count) == (tied < definition_count)
        {
            return Err(Gfx942RegisterFactsErrorV1::InvalidTiedOperand {
                operand: index as u16,
                tied_to: tied as u16,
            });
        }
        let Some(left) = &operand_aliases[index] else {
            return Err(Gfx942RegisterFactsErrorV1::InvalidTiedOperand {
                operand: index as u16,
                tied_to: tied as u16,
            });
        };
        if operand_aliases[tied].as_ref() != Some(left) {
            return Err(Gfx942RegisterFactsErrorV1::TiedRegisterMismatch {
                operand: index as u16,
                tied_to: tied as u16,
            });
        }
    }

    let implicit_definitions = instruction
        .implicit_definitions()
        .iter()
        .map(|name| Gfx942RegisterAliasV1::decode(name))
        .collect::<Result<Vec<_>, _>>()?;
    let implicit_uses = instruction
        .implicit_uses()
        .iter()
        .map(|name| Gfx942RegisterAliasV1::decode(name))
        .collect::<Result<Vec<_>, _>>()?;
    let mut definitions = BTreeSet::new();
    let mut uses = BTreeSet::new();
    for alias in operand_aliases[..definition_count].iter().flatten() {
        definitions.extend(alias.units.iter().copied());
    }
    for alias in operand_aliases[definition_count..].iter().flatten() {
        uses.extend(alias.units.iter().copied());
    }
    for alias in &implicit_definitions {
        definitions.extend(alias.units.iter().copied());
    }
    for alias in &implicit_uses {
        uses.extend(alias.units.iter().copied());
    }
    Ok(Gfx942InstructionRegisterFactsV1 {
        operand_aliases,
        explicit_definition_count: instruction.explicit_definition_count(),
        implicit_definitions,
        implicit_uses,
        definitions: definitions.into_iter().collect(),
        uses: uses.into_iter().collect(),
    })
}

fn decode_register_alias(name: &str) -> Result<Gfx942RegisterAliasV1, Gfx942RegisterFactsErrorV1> {
    let special = match name {
        "EXEC" => Some(vec![
            Gfx942RegisterUnitV1::ExecLow,
            Gfx942RegisterUnitV1::ExecHigh,
        ]),
        "EXEC_LO" => Some(vec![Gfx942RegisterUnitV1::ExecLow]),
        "EXEC_HI" => Some(vec![Gfx942RegisterUnitV1::ExecHigh]),
        "VCC" => Some(vec![
            Gfx942RegisterUnitV1::VccLow,
            Gfx942RegisterUnitV1::VccHigh,
        ]),
        "VCC_LO" => Some(vec![Gfx942RegisterUnitV1::VccLow]),
        "VCC_HI" => Some(vec![Gfx942RegisterUnitV1::VccHigh]),
        "SCC" => Some(vec![Gfx942RegisterUnitV1::Scc]),
        "MODE" => Some(vec![Gfx942RegisterUnitV1::Mode]),
        _ => None,
    };
    if let Some(units) = special {
        return Ok(Gfx942RegisterAliasV1 { units });
    }
    if name.starts_with("SGPR") {
        return decode_indexed_alias(name, "SGPR", MAX_GFX942_SGPR_INDEX_V1, |index| {
            Gfx942RegisterUnitV1::Sgpr(index)
        });
    }
    if name.starts_with("VGPR") {
        return decode_indexed_alias(name, "VGPR", MAX_GFX942_VGPR_INDEX_V1, |index| {
            Gfx942RegisterUnitV1::Vgpr(index)
        });
    }
    Err(Gfx942RegisterFactsErrorV1::UnsupportedRegister(
        name.to_owned(),
    ))
}

fn decode_indexed_alias(
    name: &str,
    prefix: &str,
    maximum: u16,
    unit: impl Fn(u16) -> Gfx942RegisterUnitV1,
) -> Result<Gfx942RegisterAliasV1, Gfx942RegisterFactsErrorV1> {
    let components = name.split('_').collect::<Vec<_>>();
    if components.is_empty() || components.len() > MAX_GFX942_REGISTER_ALIAS_UNITS_V1 {
        return Err(Gfx942RegisterFactsErrorV1::AliasWidth {
            actual: components.len(),
        });
    }
    let mut units = Vec::with_capacity(components.len());
    let mut previous = None;
    for component in components {
        let Some(index) = component.strip_prefix(prefix) else {
            return Err(Gfx942RegisterFactsErrorV1::MixedRegisterAlias(
                name.to_owned(),
            ));
        };
        if index.is_empty() || (index.len() > 1 && index.starts_with('0')) {
            return Err(Gfx942RegisterFactsErrorV1::NonCanonicalRegister(
                name.to_owned(),
            ));
        }
        let index = index
            .parse::<u16>()
            .map_err(|_| Gfx942RegisterFactsErrorV1::NonCanonicalRegister(name.to_owned()))?;
        if index > maximum {
            return Err(Gfx942RegisterFactsErrorV1::RegisterIndexOutOfRange {
                name: name.to_owned(),
                index,
                maximum,
            });
        }
        if previous.is_some_and(|previous| index != previous + 1) {
            return Err(Gfx942RegisterFactsErrorV1::NonContiguousRegisterAlias(
                name.to_owned(),
            ));
        }
        units.push(unit(index));
        previous = Some(index);
    }
    Ok(Gfx942RegisterAliasV1 { units })
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Gfx942RegisterFactsErrorV1 {
    UnsupportedRegister(String),
    NonCanonicalRegister(String),
    MixedRegisterAlias(String),
    NonContiguousRegisterAlias(String),
    RegisterIndexOutOfRange {
        name: String,
        index: u16,
        maximum: u16,
    },
    AliasWidth {
        actual: usize,
    },
    InvalidDefinitionCount,
    NonRegisterDefinition {
        operand: u16,
    },
    InvalidTiedOperand {
        operand: u16,
        tied_to: u16,
    },
    TiedRegisterMismatch {
        operand: u16,
        tied_to: u16,
    },
}

impl fmt::Display for Gfx942RegisterFactsErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid gfx942 register facts: {self:?}")
    }
}

impl Error for Gfx942RegisterFactsErrorV1 {}
