//! Bounded CFG and register dataflow over authenticated gfx942 trace facts.
//!
//! This layer computes ordinary graph reachability, dominators, post-dominators,
//! natural loops, and reaching physical-register definitions from exact decoded
//! instruction facts. It does not interpret AMDGPU opcodes or establish that any
//! branch, trap, address, or floating-point recurrence refines source semantics.

use crate::{
    Gfx942InstructionRegisterFactsV1, Gfx942RegisterFactsErrorV1, Gfx942RegisterUnitV1,
    PhysicalMachineTraceEvidenceIdentityV1, PhysicalMachineTraceEvidenceV1,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fmt,
};

pub const MAX_GFX942_MACHINE_DATAFLOW_WORK_V1: usize = 4_000_000;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Gfx942ReachingDefinitionV1 {
    LiveIn,
    Instruction { offset: u64 },
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct InstructionFactsV1 {
    offset: u64,
    block: u32,
    registers: Gfx942InstructionRegisterFactsV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct BlockFactsV1 {
    first_instruction: usize,
    instruction_end: usize,
    predecessors: Vec<u32>,
    successors: Vec<u32>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct FunctionDataflowV1 {
    blocks: Vec<BlockFactsV1>,
    instructions: Vec<InstructionFactsV1>,
    instruction_by_offset: BTreeMap<u64, usize>,
    dominators: Vec<Vec<u64>>,
    post_dominators: Vec<Vec<u64>>,
    natural_loops: Vec<Gfx942NaturalLoopV1>,
}

/// One canonical natural loop induced by a dominance-qualified CFG backedge.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Gfx942NaturalLoopV1 {
    header: u32,
    latch: u32,
    blocks: Vec<u32>,
    exits: Vec<(u32, u32)>,
}

impl Gfx942NaturalLoopV1 {
    pub const fn header(&self) -> u32 {
        self.header
    }

    pub const fn latch(&self) -> u32 {
        self.latch
    }

    pub fn blocks(&self) -> &[u32] {
        &self.blocks
    }

    pub fn exits(&self) -> &[(u32, u32)] {
        &self.exits
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Gfx942MachineDataflowV1 {
    trace_identity: PhysicalMachineTraceEvidenceIdentityV1,
    functions: BTreeMap<String, FunctionDataflowV1>,
}

impl Gfx942MachineDataflowV1 {
    pub fn derive(
        trace: &PhysicalMachineTraceEvidenceV1,
    ) -> Result<Self, Gfx942MachineDataflowErrorV1> {
        derive_dataflow(trace)
    }

    pub const fn trace_identity(&self) -> PhysicalMachineTraceEvidenceIdentityV1 {
        self.trace_identity
    }

    pub fn function_symbols(&self) -> impl Iterator<Item = &str> {
        self.functions.keys().map(String::as_str)
    }

    pub fn block_dominates(
        &self,
        function: &str,
        dominator: u32,
        block: u32,
    ) -> Result<bool, Gfx942MachineDataflowErrorV1> {
        let function = self.function(function)?;
        let dominators = function
            .dominators
            .get(block as usize)
            .ok_or(Gfx942MachineDataflowErrorV1::UnknownBlock(block))?;
        if dominator as usize >= function.blocks.len() {
            return Err(Gfx942MachineDataflowErrorV1::UnknownBlock(dominator));
        }
        Ok(bit_is_set(dominators, dominator as usize))
    }

    pub fn instruction_dominates(
        &self,
        function: &str,
        definition: u64,
        use_offset: u64,
    ) -> Result<bool, Gfx942MachineDataflowErrorV1> {
        let function = self.function(function)?;
        let definition_index = *function
            .instruction_by_offset
            .get(&definition)
            .ok_or(Gfx942MachineDataflowErrorV1::UnknownInstruction(definition))?;
        let use_index = *function
            .instruction_by_offset
            .get(&use_offset)
            .ok_or(Gfx942MachineDataflowErrorV1::UnknownInstruction(use_offset))?;
        let definition = &function.instructions[definition_index];
        let use_instruction = &function.instructions[use_index];
        if definition.block == use_instruction.block {
            return Ok(definition_index < use_index);
        }
        Ok(bit_is_set(
            &function.dominators[use_instruction.block as usize],
            definition.block as usize,
        ))
    }

    pub fn block_post_dominates(
        &self,
        function: &str,
        post_dominator: u32,
        block: u32,
    ) -> Result<bool, Gfx942MachineDataflowErrorV1> {
        let function = self.function(function)?;
        let post_dominators = function
            .post_dominators
            .get(block as usize)
            .ok_or(Gfx942MachineDataflowErrorV1::UnknownBlock(block))?;
        if post_dominator as usize >= function.blocks.len() {
            return Err(Gfx942MachineDataflowErrorV1::UnknownBlock(post_dominator));
        }
        Ok(bit_is_set(post_dominators, post_dominator as usize))
    }

    /// Returns natural loops in canonical `(header, latch)` order.
    pub fn natural_loops(
        &self,
        function: &str,
    ) -> Result<&[Gfx942NaturalLoopV1], Gfx942MachineDataflowErrorV1> {
        Ok(&self.function(function)?.natural_loops)
    }

    pub fn reaching_definitions_before(
        &self,
        function: &str,
        instruction_offset: u64,
        unit: Gfx942RegisterUnitV1,
    ) -> Result<Vec<Gfx942ReachingDefinitionV1>, Gfx942MachineDataflowErrorV1> {
        let function = self.function(function)?;
        let instruction_index = *function
            .instruction_by_offset
            .get(&instruction_offset)
            .ok_or(Gfx942MachineDataflowErrorV1::UnknownInstruction(
                instruction_offset,
            ))?;
        let initial_block = function.instructions[instruction_index].block as usize;
        let mut pending = vec![(initial_block, instruction_index)];
        let mut visited = BTreeSet::new();
        let mut definitions = BTreeSet::new();
        let mut work = 0usize;
        while let Some((block_index, before)) = pending.pop() {
            consume_work(&mut work)?;
            if !visited.insert((block_index, before)) {
                continue;
            }
            let block = &function.blocks[block_index];
            if before < block.first_instruction || before > block.instruction_end {
                return Err(Gfx942MachineDataflowErrorV1::InvalidBlockRange);
            }
            let mut found = None;
            for index in (block.first_instruction..before).rev() {
                consume_work(&mut work)?;
                if function.instructions[index]
                    .registers
                    .definitions()
                    .binary_search(&unit)
                    .is_ok()
                {
                    found = Some(function.instructions[index].offset);
                    break;
                }
            }
            if let Some(offset) = found {
                definitions.insert(Gfx942ReachingDefinitionV1::Instruction { offset });
                continue;
            }
            if block.predecessors.is_empty() {
                definitions.insert(Gfx942ReachingDefinitionV1::LiveIn);
                continue;
            }
            for predecessor in &block.predecessors {
                let predecessor = *predecessor as usize;
                pending.push((predecessor, function.blocks[predecessor].instruction_end));
            }
        }
        if definitions.is_empty() {
            return Err(Gfx942MachineDataflowErrorV1::NoReachingDefinition);
        }
        Ok(definitions.into_iter().collect())
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

    fn function(&self, symbol: &str) -> Result<&FunctionDataflowV1, Gfx942MachineDataflowErrorV1> {
        self.functions
            .get(symbol)
            .ok_or_else(|| Gfx942MachineDataflowErrorV1::UnknownFunction(symbol.to_owned()))
    }
}

fn derive_dataflow(
    trace: &PhysicalMachineTraceEvidenceV1,
) -> Result<Gfx942MachineDataflowV1, Gfx942MachineDataflowErrorV1> {
    let mut blocks_by_function = BTreeMap::<&str, Vec<_>>::new();
    for block in trace.blocks() {
        blocks_by_function
            .entry(block.function_symbol())
            .or_default()
            .push(block);
    }
    let mut instructions_by_function = BTreeMap::<&str, Vec<_>>::new();
    for instruction in trace.instructions() {
        instructions_by_function
            .entry(instruction.function_symbol())
            .or_default()
            .push(instruction);
    }
    if blocks_by_function.keys().copied().collect::<Vec<_>>()
        != instructions_by_function.keys().copied().collect::<Vec<_>>()
    {
        return Err(Gfx942MachineDataflowErrorV1::FunctionSetMismatch);
    }

    let mut functions = BTreeMap::new();
    let mut work = 0usize;
    for (symbol, source_blocks) in blocks_by_function {
        let source_instructions = &instructions_by_function[symbol];
        let mut instructions = Vec::with_capacity(source_instructions.len());
        let mut instruction_by_offset = BTreeMap::new();
        for (index, instruction) in source_instructions.iter().enumerate() {
            let registers = Gfx942InstructionRegisterFactsV1::derive(instruction)
                .map_err(Gfx942MachineDataflowErrorV1::RegisterFacts)?;
            if instruction_by_offset
                .insert(instruction.instruction_offset(), index)
                .is_some()
            {
                return Err(Gfx942MachineDataflowErrorV1::DuplicateInstruction);
            }
            instructions.push(InstructionFactsV1 {
                offset: instruction.instruction_offset(),
                block: instruction.block_ordinal(),
                registers,
            });
        }
        let mut blocks = Vec::with_capacity(source_blocks.len());
        let mut cursor = 0usize;
        for (expected, block) in source_blocks.iter().enumerate() {
            if block.ordinal() as usize != expected
                || block.instruction_count() == 0
                || cursor >= instructions.len()
                || instructions[cursor].offset != block.first_instruction_offset()
            {
                return Err(Gfx942MachineDataflowErrorV1::InvalidBlockRange);
            }
            let end = cursor
                .checked_add(block.instruction_count() as usize)
                .ok_or(Gfx942MachineDataflowErrorV1::InvalidBlockRange)?;
            if end > instructions.len()
                || instructions[cursor..end]
                    .iter()
                    .any(|instruction| instruction.block as usize != expected)
            {
                return Err(Gfx942MachineDataflowErrorV1::InvalidBlockRange);
            }
            blocks.push(BlockFactsV1 {
                first_instruction: cursor,
                instruction_end: end,
                predecessors: Vec::new(),
                successors: block.successors().to_vec(),
            });
            cursor = end;
        }
        if cursor != instructions.len() {
            return Err(Gfx942MachineDataflowErrorV1::InvalidBlockRange);
        }
        for block in 0..blocks.len() {
            for successor in blocks[block].successors.clone() {
                let Some(successor_block) = blocks.get_mut(successor as usize) else {
                    return Err(Gfx942MachineDataflowErrorV1::UnknownBlock(successor));
                };
                successor_block.predecessors.push(block as u32);
            }
        }
        for block in &mut blocks {
            block.predecessors.sort_unstable();
            block.predecessors.dedup();
        }
        validate_reachability(&blocks, &mut work)?;
        let dominators = derive_dominators(&blocks, &mut work)?;
        let post_dominators = derive_post_dominators(&blocks, &mut work)?;
        let natural_loops = derive_natural_loops(&blocks, &dominators, &mut work)?;
        functions.insert(
            symbol.to_owned(),
            FunctionDataflowV1 {
                blocks,
                instructions,
                instruction_by_offset,
                dominators,
                post_dominators,
                natural_loops,
            },
        );
    }
    Ok(Gfx942MachineDataflowV1 {
        trace_identity: trace.identity(),
        functions,
    })
}

fn validate_reachability(
    blocks: &[BlockFactsV1],
    work: &mut usize,
) -> Result<(), Gfx942MachineDataflowErrorV1> {
    if blocks.is_empty() || !blocks[0].predecessors.is_empty() {
        return Err(Gfx942MachineDataflowErrorV1::InvalidEntryBlock);
    }
    let mut pending = vec![0usize];
    let mut reachable = BTreeSet::new();
    while let Some(block) = pending.pop() {
        consume_work(work)?;
        if !reachable.insert(block) {
            continue;
        }
        pending.extend(blocks[block].successors.iter().map(|block| *block as usize));
    }
    if reachable.len() != blocks.len() {
        return Err(Gfx942MachineDataflowErrorV1::UnreachableBlock);
    }
    Ok(())
}

fn derive_dominators(
    blocks: &[BlockFactsV1],
    work: &mut usize,
) -> Result<Vec<Vec<u64>>, Gfx942MachineDataflowErrorV1> {
    let word_count = blocks.len().div_ceil(u64::BITS as usize);
    let mut all = vec![u64::MAX; word_count];
    let trailing = blocks.len() % u64::BITS as usize;
    if trailing != 0 {
        all[word_count - 1] = (1_u64 << trailing) - 1;
    }
    let mut dominators = vec![all; blocks.len()];
    dominators[0] = vec![0; word_count];
    set_bit(&mut dominators[0], 0);
    let mut changed = true;
    while changed {
        changed = false;
        for block in 1..blocks.len() {
            consume_work(work)?;
            if blocks[block].predecessors.is_empty() {
                return Err(Gfx942MachineDataflowErrorV1::UnreachableBlock);
            }
            let mut next = vec![u64::MAX; word_count];
            for predecessor in &blocks[block].predecessors {
                for (word, predecessor_word) in
                    next.iter_mut().zip(&dominators[*predecessor as usize])
                {
                    consume_work(work)?;
                    *word &= predecessor_word;
                }
            }
            set_bit(&mut next, block);
            if next != dominators[block] {
                dominators[block] = next;
                changed = true;
            }
        }
    }
    Ok(dominators)
}

fn derive_post_dominators(
    blocks: &[BlockFactsV1],
    work: &mut usize,
) -> Result<Vec<Vec<u64>>, Gfx942MachineDataflowErrorV1> {
    let exits = blocks
        .iter()
        .enumerate()
        .filter_map(|(block, facts)| facts.successors.is_empty().then_some(block))
        .collect::<Vec<_>>();
    if exits.is_empty() {
        return Err(Gfx942MachineDataflowErrorV1::NoExitBlock);
    }
    let mut pending = exits.clone();
    let mut can_reach_exit = BTreeSet::new();
    while let Some(block) = pending.pop() {
        consume_work(work)?;
        if !can_reach_exit.insert(block) {
            continue;
        }
        pending.extend(
            blocks[block]
                .predecessors
                .iter()
                .map(|predecessor| *predecessor as usize),
        );
    }
    if let Some(block) = (0..blocks.len()).find(|block| !can_reach_exit.contains(block)) {
        return Err(Gfx942MachineDataflowErrorV1::BlockCannotReachExit(
            block as u32,
        ));
    }

    let word_count = blocks.len().div_ceil(u64::BITS as usize);
    let mut all = vec![u64::MAX; word_count];
    let trailing = blocks.len() % u64::BITS as usize;
    if trailing != 0 {
        all[word_count - 1] = (1_u64 << trailing) - 1;
    }
    let mut post_dominators = vec![all; blocks.len()];
    for block in exits {
        post_dominators[block] = vec![0; word_count];
        set_bit(&mut post_dominators[block], block);
    }
    let mut changed = true;
    while changed {
        changed = false;
        for block in (0..blocks.len()).rev() {
            consume_work(work)?;
            if blocks[block].successors.is_empty() {
                continue;
            }
            let mut next = vec![u64::MAX; word_count];
            for successor in &blocks[block].successors {
                for (word, successor_word) in
                    next.iter_mut().zip(&post_dominators[*successor as usize])
                {
                    consume_work(work)?;
                    *word &= successor_word;
                }
            }
            set_bit(&mut next, block);
            if next != post_dominators[block] {
                post_dominators[block] = next;
                changed = true;
            }
        }
    }
    Ok(post_dominators)
}

fn derive_natural_loops(
    blocks: &[BlockFactsV1],
    dominators: &[Vec<u64>],
    work: &mut usize,
) -> Result<Vec<Gfx942NaturalLoopV1>, Gfx942MachineDataflowErrorV1> {
    let mut loops = Vec::new();
    for (latch, block) in blocks.iter().enumerate() {
        for header in &block.successors {
            consume_work(work)?;
            let header_index = *header as usize;
            if !bit_is_set(&dominators[latch], header_index) {
                continue;
            }
            let mut members = BTreeSet::from([header_index, latch]);
            let mut pending = (latch != header_index)
                .then_some(latch)
                .into_iter()
                .collect::<Vec<_>>();
            while let Some(member) = pending.pop() {
                consume_work(work)?;
                for predecessor in &blocks[member].predecessors {
                    consume_work(work)?;
                    let predecessor = *predecessor as usize;
                    if members.insert(predecessor) && predecessor != header_index {
                        pending.push(predecessor);
                    }
                }
            }
            let mut exits = BTreeSet::new();
            for member in &members {
                for successor in &blocks[*member].successors {
                    consume_work(work)?;
                    if !members.contains(&(*successor as usize)) {
                        exits.insert((*member as u32, *successor));
                    }
                }
            }
            loops.push(Gfx942NaturalLoopV1 {
                header: *header,
                latch: latch as u32,
                blocks: members.into_iter().map(|block| block as u32).collect(),
                exits: exits.into_iter().collect(),
            });
        }
    }
    loops.sort_by_key(|loop_| (loop_.header, loop_.latch));
    Ok(loops)
}

fn bit_is_set(words: &[u64], bit: usize) -> bool {
    words[bit / u64::BITS as usize] & (1_u64 << (bit % u64::BITS as usize)) != 0
}

fn set_bit(words: &mut [u64], bit: usize) {
    words[bit / u64::BITS as usize] |= 1_u64 << (bit % u64::BITS as usize);
}

fn consume_work(work: &mut usize) -> Result<(), Gfx942MachineDataflowErrorV1> {
    *work = work
        .checked_add(1)
        .ok_or(Gfx942MachineDataflowErrorV1::WorkLimit)?;
    if *work > MAX_GFX942_MACHINE_DATAFLOW_WORK_V1 {
        return Err(Gfx942MachineDataflowErrorV1::WorkLimit);
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Gfx942MachineDataflowErrorV1 {
    RegisterFacts(Gfx942RegisterFactsErrorV1),
    FunctionSetMismatch,
    UnknownFunction(String),
    UnknownBlock(u32),
    UnknownInstruction(u64),
    DuplicateInstruction,
    InvalidEntryBlock,
    InvalidBlockRange,
    UnreachableBlock,
    NoExitBlock,
    BlockCannotReachExit(u32),
    NoReachingDefinition,
    WorkLimit,
}

impl fmt::Display for Gfx942MachineDataflowErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid gfx942 machine dataflow: {self:?}")
    }
}

impl Error for Gfx942MachineDataflowErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::RegisterFacts(error) => Some(error),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dense_dominator_bits_cross_word_boundaries_and_charge_work() {
        let count = 130usize;
        let blocks = (0..count)
            .map(|block| BlockFactsV1 {
                first_instruction: 0,
                instruction_end: 0,
                predecessors: (block != 0)
                    .then(|| vec![(block - 1) as u32])
                    .unwrap_or_default(),
                successors: (block + 1 < count)
                    .then(|| vec![(block + 1) as u32])
                    .unwrap_or_default(),
            })
            .collect::<Vec<_>>();
        let mut work = 0;
        let dominators = derive_dominators(&blocks, &mut work).unwrap();

        assert!(work > count);
        assert_eq!(dominators[129].len(), 3);
        for block in 0..count {
            assert!(bit_is_set(&dominators[129], block));
        }
        assert_eq!(dominators[129][2] >> 2, 0, "unused high bits stay clear");

        let mut exhausted = MAX_GFX942_MACHINE_DATAFLOW_WORK_V1;
        assert_eq!(
            derive_dominators(&blocks, &mut exhausted),
            Err(Gfx942MachineDataflowErrorV1::WorkLimit),
        );
    }

    #[test]
    fn post_dominators_require_every_block_to_reach_an_exit() {
        let no_exit = vec![BlockFactsV1 {
            first_instruction: 0,
            instruction_end: 0,
            predecessors: vec![0],
            successors: vec![0],
        }];
        assert_eq!(
            derive_post_dominators(&no_exit, &mut 0),
            Err(Gfx942MachineDataflowErrorV1::NoExitBlock),
        );

        let closed_cycle = vec![
            BlockFactsV1 {
                first_instruction: 0,
                instruction_end: 0,
                predecessors: Vec::new(),
                successors: vec![1, 2],
            },
            BlockFactsV1 {
                first_instruction: 0,
                instruction_end: 0,
                predecessors: vec![0, 1],
                successors: vec![1],
            },
            BlockFactsV1 {
                first_instruction: 0,
                instruction_end: 0,
                predecessors: vec![0],
                successors: Vec::new(),
            },
        ];
        assert_eq!(
            derive_post_dominators(&closed_cycle, &mut 0),
            Err(Gfx942MachineDataflowErrorV1::BlockCannotReachExit(1)),
        );
    }

    #[test]
    fn post_dominators_intersect_distinct_exit_paths() {
        let blocks = vec![
            BlockFactsV1 {
                first_instruction: 0,
                instruction_end: 0,
                predecessors: Vec::new(),
                successors: vec![1, 2],
            },
            BlockFactsV1 {
                first_instruction: 0,
                instruction_end: 0,
                predecessors: vec![0],
                successors: vec![3],
            },
            BlockFactsV1 {
                first_instruction: 0,
                instruction_end: 0,
                predecessors: vec![0],
                successors: vec![4],
            },
            BlockFactsV1 {
                first_instruction: 0,
                instruction_end: 0,
                predecessors: vec![1],
                successors: Vec::new(),
            },
            BlockFactsV1 {
                first_instruction: 0,
                instruction_end: 0,
                predecessors: vec![2],
                successors: Vec::new(),
            },
        ];
        let post_dominators = derive_post_dominators(&blocks, &mut 0).unwrap();

        assert!(bit_is_set(&post_dominators[0], 0));
        assert!(!bit_is_set(&post_dominators[0], 1));
        assert!(!bit_is_set(&post_dominators[0], 2));
        assert!(bit_is_set(&post_dominators[1], 3));
        assert!(bit_is_set(&post_dominators[2], 4));
    }
}
