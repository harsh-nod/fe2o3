//! Bounded structural EXEC control facts for authenticated gfx942 traces.
//!
//! This layer joins exact EXEC-conditioned branches to their unique reaching
//! definitions, CFG successors, immediate post-dominators, and structurally
//! matching saved-mask OR sites. Opcode names and operand shapes remain extractor
//! facts: this module does not assign AMDGPU semantics or prove a mask empty.

use crate::{
    Gfx942InstructionRegisterFactsV1, Gfx942MachineDataflowErrorV1, Gfx942MachineDataflowV1,
    Gfx942ReachingDefinitionV1, Gfx942RegisterAliasV1, Gfx942RegisterFactsErrorV1,
    Gfx942RegisterUnitV1, PhysicalMachineBranchKindV1, PhysicalMachineInstructionTraceV1,
    PhysicalMachineOperandValueV1, PhysicalMachineTraceEvidenceIdentityV1,
    PhysicalMachineTraceEvidenceV1,
};
use std::{collections::BTreeMap, error::Error, fmt};

pub const MAX_GFX942_EXEC_CONTROL_WORK_V1: usize = 4_000_000;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Gfx942ExecBranchSenseV1 {
    Zero,
    NonZero,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Gfx942ExecDefinitionOpcodeV1 {
    And,
    AndNotSecond,
    AndSave,
    AndNotSecondSave,
    Or,
    Xor,
    Move,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Gfx942ExecMaskOperandV1 {
    Register(Gfx942RegisterAliasV1),
    SignedImmediate(i64),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Gfx942ExecMaskDefinitionV1 {
    LiveIn,
    Instruction {
        offset: u64,
        opcode: Gfx942ExecDefinitionOpcodeV1,
        saved_exec: Option<Gfx942RegisterAliasV1>,
        sources: Vec<Gfx942ExecMaskOperandV1>,
    },
}

impl Gfx942ExecMaskDefinitionV1 {
    pub const fn offset(&self) -> Option<u64> {
        match self {
            Self::LiveIn => None,
            Self::Instruction { offset, .. } => Some(*offset),
        }
    }

    pub const fn opcode(&self) -> Option<Gfx942ExecDefinitionOpcodeV1> {
        match self {
            Self::LiveIn => None,
            Self::Instruction { opcode, .. } => Some(*opcode),
        }
    }

    pub const fn saved_exec(&self) -> Option<&Gfx942RegisterAliasV1> {
        match self {
            Self::LiveIn => None,
            Self::Instruction { saved_exec, .. } => saved_exec.as_ref(),
        }
    }

    pub fn sources(&self) -> &[Gfx942ExecMaskOperandV1] {
        match self {
            Self::LiveIn => &[],
            Self::Instruction { sources, .. } => sources,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Gfx942ExecBranchV1 {
    branch_offset: u64,
    block: u32,
    sense: Gfx942ExecBranchSenseV1,
    taken_block: u32,
    fallthrough_block: u32,
    immediate_post_dominator: Option<u32>,
    definition: Gfx942ExecMaskDefinitionV1,
    matching_restore_offset: Option<u64>,
}

impl Gfx942ExecBranchV1 {
    pub const fn branch_offset(&self) -> u64 {
        self.branch_offset
    }

    pub const fn block(&self) -> u32 {
        self.block
    }

    pub const fn sense(&self) -> Gfx942ExecBranchSenseV1 {
        self.sense
    }

    pub const fn taken_block(&self) -> u32 {
        self.taken_block
    }

    pub const fn fallthrough_block(&self) -> u32 {
        self.fallthrough_block
    }

    pub const fn immediate_post_dominator(&self) -> Option<u32> {
        self.immediate_post_dominator
    }

    pub const fn definition(&self) -> &Gfx942ExecMaskDefinitionV1 {
        &self.definition
    }

    pub const fn matching_restore_offset(&self) -> Option<u64> {
        self.matching_restore_offset
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Gfx942ExecControlV1 {
    trace_identity: PhysicalMachineTraceEvidenceIdentityV1,
    branches: BTreeMap<String, Vec<Gfx942ExecBranchV1>>,
}

impl Gfx942ExecControlV1 {
    pub fn derive(
        trace: &PhysicalMachineTraceEvidenceV1,
    ) -> Result<Self, Gfx942ExecControlErrorV1> {
        derive_exec_control(trace)
    }

    pub const fn trace_identity(&self) -> PhysicalMachineTraceEvidenceIdentityV1 {
        self.trace_identity
    }

    pub fn function_symbols(&self) -> impl Iterator<Item = &str> {
        self.branches.keys().map(String::as_str)
    }

    pub fn branches(
        &self,
        function: &str,
    ) -> Result<&[Gfx942ExecBranchV1], Gfx942ExecControlErrorV1> {
        self.branches
            .get(function)
            .map(Vec::as_slice)
            .ok_or_else(|| Gfx942ExecControlErrorV1::UnknownFunction(function.to_owned()))
    }

    pub const fn establishes_machine_semantics(&self) -> bool {
        false
    }

    pub const fn establishes_compiler_refinement(&self) -> bool {
        false
    }

    pub const fn proves_empty_masks(&self) -> bool {
        false
    }

    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

fn derive_exec_control(
    trace: &PhysicalMachineTraceEvidenceV1,
) -> Result<Gfx942ExecControlV1, Gfx942ExecControlErrorV1> {
    let dataflow = Gfx942MachineDataflowV1::derive(trace)?;
    let mut blocks = BTreeMap::<&str, Vec<_>>::new();
    for block in trace.blocks() {
        blocks
            .entry(block.function_symbol())
            .or_default()
            .push(block);
    }
    let mut instructions = BTreeMap::<&str, Vec<_>>::new();
    for instruction in trace.instructions() {
        instructions
            .entry(instruction.function_symbol())
            .or_default()
            .push(instruction);
    }

    let mut work = 0usize;
    let mut branches = BTreeMap::new();
    for (function, function_blocks) in blocks {
        let function_instructions = instructions
            .get(function)
            .ok_or(Gfx942ExecControlErrorV1::InvalidFunctionInventory)?;
        let by_offset = function_instructions
            .iter()
            .map(|instruction| (instruction.instruction_offset(), *instruction))
            .collect::<BTreeMap<_, _>>();
        let first_offset_to_block = function_blocks
            .iter()
            .map(|block| (block.first_instruction_offset(), block.ordinal()))
            .collect::<BTreeMap<_, _>>();
        let mut last_offset_by_block = BTreeMap::new();
        for instruction in function_instructions {
            consume_work(&mut work)?;
            last_offset_by_block.insert(
                instruction.block_ordinal(),
                instruction.instruction_offset(),
            );
        }

        let mut function_branches = Vec::new();
        for branch in function_instructions {
            consume_work(&mut work)?;
            let sense = match branch.opcode() {
                "S_CBRANCH_EXECZ_vi" => Gfx942ExecBranchSenseV1::Zero,
                "S_CBRANCH_EXECNZ_vi" => Gfx942ExecBranchSenseV1::NonZero,
                opcode if opcode.starts_with("S_CBRANCH_EXEC") => {
                    return Err(Gfx942ExecControlErrorV1::UnsupportedExecBranchOpcode(
                        opcode.to_owned(),
                    ));
                }
                _ => continue,
            };
            let facts = Gfx942InstructionRegisterFactsV1::derive(branch)?;
            if branch.branch_kind() != PhysicalMachineBranchKindV1::ConditionalDirect
                || !branch.flags().is_terminator()
                || branch.explicit_definition_count() != 0
                || !facts.definitions().is_empty()
                || facts.uses()
                    != [
                        Gfx942RegisterUnitV1::ExecLow,
                        Gfx942RegisterUnitV1::ExecHigh,
                    ]
                || branch.operands().len() != 1
                || !matches!(
                    branch.operands()[0].value(),
                    PhysicalMachineOperandValueV1::SignedImmediate(_)
                )
                || last_offset_by_block.get(&branch.block_ordinal())
                    != Some(&branch.instruction_offset())
            {
                return Err(Gfx942ExecControlErrorV1::InvalidExecBranchShape(
                    branch.instruction_offset(),
                ));
            }
            let target_offset =
                branch
                    .branch_target()
                    .ok_or(Gfx942ExecControlErrorV1::InvalidExecBranchShape(
                        branch.instruction_offset(),
                    ))?;
            let taken_block = *first_offset_to_block.get(&target_offset).ok_or(
                Gfx942ExecControlErrorV1::InvalidExecBranchTarget(target_offset),
            )?;
            let source_block = function_blocks.get(branch.block_ordinal() as usize).ok_or(
                Gfx942ExecControlErrorV1::InvalidExecBranchShape(branch.instruction_offset()),
            )?;
            if source_block.successors().len() != 2
                || !source_block.successors().contains(&taken_block)
            {
                return Err(Gfx942ExecControlErrorV1::InvalidExecBranchShape(
                    branch.instruction_offset(),
                ));
            }
            let fallthrough = source_block
                .successors()
                .iter()
                .copied()
                .find(|successor| *successor != taken_block)
                .ok_or(Gfx942ExecControlErrorV1::InvalidExecBranchShape(
                    branch.instruction_offset(),
                ))?;

            let low = dataflow.reaching_definitions_before(
                function,
                branch.instruction_offset(),
                Gfx942RegisterUnitV1::ExecLow,
            )?;
            let high = dataflow.reaching_definitions_before(
                function,
                branch.instruction_offset(),
                Gfx942RegisterUnitV1::ExecHigh,
            )?;
            if low != high || low.len() != 1 {
                return Err(Gfx942ExecControlErrorV1::AmbiguousExecDefinition(
                    branch.instruction_offset(),
                ));
            }
            let definition = match low[0] {
                Gfx942ReachingDefinitionV1::LiveIn => Gfx942ExecMaskDefinitionV1::LiveIn,
                Gfx942ReachingDefinitionV1::Instruction { offset } => {
                    if !dataflow.instruction_dominates(
                        function,
                        offset,
                        branch.instruction_offset(),
                    )? {
                        return Err(Gfx942ExecControlErrorV1::NonDominatingExecDefinition {
                            definition: offset,
                            branch: branch.instruction_offset(),
                        });
                    }
                    derive_mask_definition(
                        by_offset
                            .get(&offset)
                            .copied()
                            .ok_or(Gfx942ExecControlErrorV1::MissingExecDefinition(offset))?,
                    )?
                }
            };
            let post_dominator = immediate_post_dominator(
                &dataflow,
                function,
                branch.block_ordinal(),
                function_blocks.len(),
                &mut work,
            )?;
            let matching_restore_offset = match (definition.saved_exec(), post_dominator) {
                (Some(saved), Some(block)) => {
                    find_matching_restore(function_instructions, block, saved, &mut work)?
                }
                _ => None,
            };
            function_branches.push(Gfx942ExecBranchV1 {
                branch_offset: branch.instruction_offset(),
                block: branch.block_ordinal(),
                sense,
                taken_block,
                fallthrough_block: fallthrough,
                immediate_post_dominator: post_dominator,
                definition,
                matching_restore_offset,
            });
        }
        function_branches.sort_by_key(Gfx942ExecBranchV1::branch_offset);
        branches.insert(function.to_owned(), function_branches);
    }
    Ok(Gfx942ExecControlV1 {
        trace_identity: trace.identity(),
        branches,
    })
}

fn derive_mask_definition(
    instruction: &PhysicalMachineInstructionTraceV1,
) -> Result<Gfx942ExecMaskDefinitionV1, Gfx942ExecControlErrorV1> {
    let facts = Gfx942InstructionRegisterFactsV1::derive(instruction)?;
    if !facts.definitions().contains(&Gfx942RegisterUnitV1::ExecLow)
        || !facts
            .definitions()
            .contains(&Gfx942RegisterUnitV1::ExecHigh)
    {
        return Err(Gfx942ExecControlErrorV1::InvalidExecDefinitionShape(
            instruction.instruction_offset(),
        ));
    }
    let (opcode, saved, source_count) = match instruction.opcode() {
        "S_AND_B64_vi" => (Gfx942ExecDefinitionOpcodeV1::And, false, 2),
        "S_ANDN2_B64_vi" => (Gfx942ExecDefinitionOpcodeV1::AndNotSecond, false, 2),
        "S_AND_SAVEEXEC_B64_vi" => (Gfx942ExecDefinitionOpcodeV1::AndSave, true, 1),
        "S_ANDN2_SAVEEXEC_B64_vi" => (Gfx942ExecDefinitionOpcodeV1::AndNotSecondSave, true, 1),
        "S_OR_B64_vi" => (Gfx942ExecDefinitionOpcodeV1::Or, false, 2),
        "S_XOR_B64_vi" => (Gfx942ExecDefinitionOpcodeV1::Xor, false, 2),
        "S_MOV_B64_vi" => (Gfx942ExecDefinitionOpcodeV1::Move, false, 1),
        opcode => {
            return Err(Gfx942ExecControlErrorV1::UnsupportedExecDefinitionOpcode(
                opcode.to_owned(),
            ));
        }
    };
    if instruction.explicit_definition_count() != 1
        || instruction.operands().len() != source_count + 1
    {
        return Err(Gfx942ExecControlErrorV1::InvalidExecDefinitionShape(
            instruction.instruction_offset(),
        ));
    }
    let destination = facts
        .operand_aliases()
        .first()
        .and_then(Option::as_ref)
        .ok_or(Gfx942ExecControlErrorV1::InvalidExecDefinitionShape(
            instruction.instruction_offset(),
        ))?;
    let saved_exec = if saved {
        if !is_sgpr_pair(destination.units()) {
            return Err(Gfx942ExecControlErrorV1::InvalidExecDefinitionShape(
                instruction.instruction_offset(),
            ));
        }
        Some(destination.clone())
    } else {
        if destination.units()
            != [
                Gfx942RegisterUnitV1::ExecLow,
                Gfx942RegisterUnitV1::ExecHigh,
            ]
        {
            return Err(Gfx942ExecControlErrorV1::InvalidExecDefinitionShape(
                instruction.instruction_offset(),
            ));
        }
        None
    };
    let sources = instruction.operands()[1..]
        .iter()
        .map(|operand| match operand.value() {
            PhysicalMachineOperandValueV1::Register(name) => Gfx942RegisterAliasV1::decode(name)
                .map(Gfx942ExecMaskOperandV1::Register)
                .map_err(Gfx942ExecControlErrorV1::RegisterFacts),
            PhysicalMachineOperandValueV1::SignedImmediate(value) => {
                Ok(Gfx942ExecMaskOperandV1::SignedImmediate(*value))
            }
            _ => Err(Gfx942ExecControlErrorV1::InvalidExecDefinitionShape(
                instruction.instruction_offset(),
            )),
        })
        .collect::<Result<Vec<_>, _>>()?;
    if sources.iter().any(|source| {
        matches!(source, Gfx942ExecMaskOperandV1::Register(alias) if !is_mask_alias(alias.units()))
    }) {
        return Err(Gfx942ExecControlErrorV1::InvalidExecDefinitionShape(
            instruction.instruction_offset(),
        ));
    }
    Ok(Gfx942ExecMaskDefinitionV1::Instruction {
        offset: instruction.instruction_offset(),
        opcode,
        saved_exec,
        sources,
    })
}

fn immediate_post_dominator(
    dataflow: &Gfx942MachineDataflowV1,
    function: &str,
    block: u32,
    block_count: usize,
    work: &mut usize,
) -> Result<Option<u32>, Gfx942ExecControlErrorV1> {
    let mut nearest = None;
    for candidate in 0..block_count as u32 {
        consume_work(work)?;
        if candidate == block || !dataflow.block_post_dominates(function, candidate, block)? {
            continue;
        }
        let Some(current) = nearest else {
            nearest = Some(candidate);
            continue;
        };
        if dataflow.block_post_dominates(function, current, candidate)? {
            nearest = Some(candidate);
        } else if !dataflow.block_post_dominates(function, candidate, current)? {
            return Err(Gfx942ExecControlErrorV1::AmbiguousImmediatePostDominator(
                block,
            ));
        }
    }
    Ok(nearest)
}

fn find_matching_restore(
    instructions: &[&PhysicalMachineInstructionTraceV1],
    block: u32,
    saved: &Gfx942RegisterAliasV1,
    work: &mut usize,
) -> Result<Option<u64>, Gfx942ExecControlErrorV1> {
    for instruction in instructions
        .iter()
        .copied()
        .filter(|instruction| instruction.block_ordinal() == block)
    {
        consume_work(work)?;
        let facts = Gfx942InstructionRegisterFactsV1::derive(instruction)?;
        if !facts.definitions().contains(&Gfx942RegisterUnitV1::ExecLow)
            && !facts
                .definitions()
                .contains(&Gfx942RegisterUnitV1::ExecHigh)
        {
            continue;
        }
        let Ok(Gfx942ExecMaskDefinitionV1::Instruction {
            opcode: Gfx942ExecDefinitionOpcodeV1::Or,
            sources,
            ..
        }) = derive_mask_definition(instruction)
        else {
            return Ok(None);
        };
        let exec = Gfx942RegisterAliasV1::decode("EXEC")?;
        let expected = [
            Gfx942ExecMaskOperandV1::Register(exec),
            Gfx942ExecMaskOperandV1::Register(saved.clone()),
        ];
        return Ok((sources == expected).then_some(instruction.instruction_offset()));
    }
    Ok(None)
}

fn is_sgpr_pair(units: &[Gfx942RegisterUnitV1]) -> bool {
    matches!(
        units,
        [Gfx942RegisterUnitV1::Sgpr(first), Gfx942RegisterUnitV1::Sgpr(second)]
            if *second == *first + 1
    )
}

fn is_mask_alias(units: &[Gfx942RegisterUnitV1]) -> bool {
    is_sgpr_pair(units)
        || units
            == [
                Gfx942RegisterUnitV1::ExecLow,
                Gfx942RegisterUnitV1::ExecHigh,
            ]
        || units == [Gfx942RegisterUnitV1::VccLow, Gfx942RegisterUnitV1::VccHigh]
}

fn consume_work(work: &mut usize) -> Result<(), Gfx942ExecControlErrorV1> {
    *work = work
        .checked_add(1)
        .ok_or(Gfx942ExecControlErrorV1::WorkLimit)?;
    if *work > MAX_GFX942_EXEC_CONTROL_WORK_V1 {
        return Err(Gfx942ExecControlErrorV1::WorkLimit);
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Gfx942ExecControlErrorV1 {
    Dataflow(Gfx942MachineDataflowErrorV1),
    RegisterFacts(Gfx942RegisterFactsErrorV1),
    UnknownFunction(String),
    InvalidFunctionInventory,
    UnsupportedExecBranchOpcode(String),
    InvalidExecBranchShape(u64),
    InvalidExecBranchTarget(u64),
    AmbiguousExecDefinition(u64),
    MissingExecDefinition(u64),
    NonDominatingExecDefinition { definition: u64, branch: u64 },
    UnsupportedExecDefinitionOpcode(String),
    InvalidExecDefinitionShape(u64),
    AmbiguousImmediatePostDominator(u32),
    WorkLimit,
}

impl From<Gfx942MachineDataflowErrorV1> for Gfx942ExecControlErrorV1 {
    fn from(error: Gfx942MachineDataflowErrorV1) -> Self {
        Self::Dataflow(error)
    }
}

impl From<Gfx942RegisterFactsErrorV1> for Gfx942ExecControlErrorV1 {
    fn from(error: Gfx942RegisterFactsErrorV1) -> Self {
        Self::RegisterFacts(error)
    }
}

impl fmt::Display for Gfx942ExecControlErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid gfx942 EXEC control: {self:?}")
    }
}

impl Error for Gfx942ExecControlErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Dataflow(error) => Some(error),
            Self::RegisterFacts(error) => Some(error),
            _ => None,
        }
    }
}
