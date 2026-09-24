//! Bounded source-event to actual KIR CFG/SSA construction. No interpreter,
//! caller-selected execution plan, Module admission or source-custody constructor.
use fe2o3_kernel_ir::{
    BasicBlock, BlockId, CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource, Function, FunctionRole,
    GFX942_PHYSICAL_ENTRY_REGISTERS_V20, Gfx942PhysicalEntryBlockContractVNext as Contract,
    Gfx942PhysicalEntryBranchEncodingVNext as Encoding,
    Gfx942PhysicalEntryDeclarationVNext as Declaration,
    Gfx942PhysicalEntryInstructionVNext as Instruction, Gfx942PhysicalEntryOpcodeV20 as Opcode,
    Gfx942PhysicalEntryOriginVNext as Origin, Gfx942PhysicalEntryRegisterV20 as Register,
    Gfx942PhysicalEntrySourceSiteVNext as Site, Gfx942PhysicalEntryStepVNext as Step, Operation,
    OperationKind, ScalarType, Signature, Terminator, Type, ValueDef, ValueId,
};
#[path = "physical_entry_materialization_storage_v20.rs"]
mod storage;
use storage::Scope;
#[cfg(test)]
#[path = "physical_entry_materialization_v20_tests.rs"]
pub(crate) mod tests;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum EventKind {
    Label(u8),
    Step(Instruction),
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Event {
    pub(crate) site: Site,
    pub(crate) kind: EventKind,
}
pub(crate) struct Input<'a> {
    pub(crate) origin: Origin,
    pub(crate) export_name: &'a str,
    pub(crate) begin: Site,
    pub(crate) events: &'a [Event],
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Error {
    Resource(Resource),
    Origin,
    Symbol,
    Occurrence,
    Shape,
    Instruction,
    UndefinedRegister,
    Count,
    Accounting,
}
impl From<Resource> for Error {
    fn from(e: Resource) -> Self {
        Self::Resource(e)
    }
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "physical-entry canonical construction: {self:?}")
    }
}
impl std::error::Error for Error {}
pub(crate) struct Pending {
    function: Function,
    retained: usize,
}
impl Pending {
    pub(crate) fn function(&self) -> &Function {
        &self.function
    }
    pub(crate) fn into_function(self) -> Function {
        self.function
    }
    pub(crate) fn retained_storage(&self) -> usize {
        self.retained
    }
}
const PARAMETERS: [ValueId; 5] = [ValueId(0), ValueId(1), ValueId(2), ValueId(3), ValueId(4)];
const WORK: usize = 32768;
type State = [Option<ValueId>; 131];
#[derive(Clone, Copy)]
struct ParsedBlock {
    contract: Contract,
    first: usize,
    end: usize,
    target: Option<u8>,
}
const EMPTY: ParsedBlock = ParsedBlock {
    contract: Contract::ZERO,
    first: 0,
    end: 0,
    target: None,
};
struct Parsed {
    blocks: [ParsedBlock; 4],
    count: usize,
    native: u8,
}

fn parse(input: &Input<'_>) -> Result<Parsed, Error> {
    if !input.origin.is_complete() {
        return Err(Error::Origin);
    }
    if input.export_name.is_empty()
        || input.export_name.len() > 128
        || !input
            .export_name
            .bytes()
            .enumerate()
            .all(|(i, b)| b == b'_' || b.is_ascii_alphabetic() || (i > 0 && b.is_ascii_digit()))
    {
        return Err(Error::Symbol);
    }
    if !input.begin.is_complete() || input.begin.occurrence != 0 || input.events.len() > 72 {
        return Err(Error::Occurrence);
    }
    let mut parsed = Parsed {
        blocks: [EMPTY; 4],
        count: 0,
        native: 0,
    };
    let mut open = false;
    for (index, event) in input.events.iter().enumerate() {
        if !event.site.is_complete()
            || usize::from(event.site.occurrence) != index + 1
            || event.site.raw_block == input.begin.raw_block
            || event.site.semantic_block_index == input.begin.semantic_block_index
            || input.events[..index].iter().any(|old| {
                old.site.raw_block == event.site.raw_block
                    || old.site.semantic_block_index == event.site.semantic_block_index
            })
        {
            return Err(Error::Occurrence);
        }
        match event.kind {
            EventKind::Label(label) => {
                if open
                    || parsed.count == 4
                    || parsed.blocks[..parsed.count]
                        .iter()
                        .any(|old| old.contract.label == label)
                {
                    return Err(Error::Shape);
                }
                let block = &mut parsed.blocks[parsed.count];
                block.contract.label = label;
                block.contract.label_site = event.site;
                block.first = index + 1;
                parsed.count += 1;
                open = true;
            }
            EventKind::Step(instruction) => {
                if !open {
                    return Err(Error::Shape);
                }
                instruction
                    .validate_shape()
                    .map_err(|_| Error::Instruction)?;
                let ordinal = if instruction.opcode == Opcode::Fallthrough {
                    None
                } else {
                    let ordinal = parsed.native;
                    parsed.native = parsed
                        .native
                        .checked_add(1)
                        .filter(|n| *n <= 64)
                        .ok_or(Error::Count)?;
                    Some(ordinal)
                };
                if instruction.opcode.is_control() {
                    let block = &mut parsed.blocks[parsed.count - 1];
                    block.end = index;
                    block.contract.terminator_site = event.site;
                    block.contract.native_ordinal = ordinal;
                    block.contract.encoding = match instruction.opcode {
                        Opcode::BranchScc1 => Encoding::Scc1,
                        Opcode::Branch => Encoding::Jump,
                        Opcode::Fallthrough => Encoding::Fallthrough,
                        Opcode::Endpgm0 => Encoding::Endpgm0,
                        _ => return Err(Error::Instruction),
                    };
                    block.target = if instruction.opcode == Opcode::Endpgm0 {
                        None
                    } else {
                        Some(instruction.immediate as u8)
                    };
                    open = false;
                }
            }
        }
    }
    if open {
        return Err(Error::Shape);
    }
    let b = &parsed.blocks;
    let shape = match parsed.count {
        1 => b[0].contract.encoding == Encoding::Endpgm0,
        4 => {
            b[0].contract.encoding == Encoding::Scc1
                && b[0].target == Some(b[2].contract.label)
                && b[1].contract.encoding == Encoding::Jump
                && b[1].target == Some(b[3].contract.label)
                && b[2].contract.encoding == Encoding::Fallthrough
                && b[2].target == Some(b[3].contract.label)
                && b[3].contract.encoding == Encoding::Endpgm0
        }
        _ => false,
    };
    if !shape || parsed.native == 0 {
        return Err(Error::Shape);
    }
    Ok(parsed)
}
fn fresh(next: &mut u32) -> Result<ValueId, Error> {
    if *next >= 768 {
        return Err(Error::Count);
    }
    let value = ValueId(*next);
    *next += 1;
    Ok(value)
}
fn read(state: &State, reg: Register) -> Result<ValueId, Error> {
    reg.state_index()
        .and_then(|i| state[i])
        .ok_or(Error::UndefinedRegister)
}

/// Only the sole semantic importer uses this constructor. The canonical V20
/// owner independently verifies general SSA/type/effects and physical facts.
pub(crate) fn materialize(input: &Input<'_>, budget: &mut Budget<'_>) -> Result<Pending, Error> {
    budget.charge_work(WORK)?;
    let parsed = parse(input)?;
    let mut scope = Scope::new(budget);
    scope.reserve(std::mem::size_of::<Pending>())?;
    let mut blocks = scope.vec::<BasicBlock>(parsed.count)?;
    let mut outgoing = [[None; 131]; 4];
    let mut next = 5;
    let mut declaration_results = scope.vec(5)?;
    let mut entry_state = [None; 131];
    for reg in GFX942_PHYSICAL_ENTRY_REGISTERS_V20 {
        let value = fresh(&mut next)?;
        entry_state[reg.state_index().ok_or(Error::Instruction)?] = Some(value);
        declaration_results.push(ValueDef::new(value, Type::Scalar(reg.scalar_type())));
    }
    let mut native = 0u8;
    for ordinal in 0..parsed.count {
        let source = parsed.blocks[ordinal];
        let mut block = BasicBlock::new(BlockId(ordinal as u32));
        let mut state = match ordinal {
            0 => entry_state,
            1 | 2 => outgoing[0],
            3 => [None; 131],
            _ => return Err(Error::Shape),
        };
        if ordinal == 3 {
            let count = outgoing[1]
                .iter()
                .zip(&outgoing[2])
                .filter(|(a, b)| a.is_some() && b.is_some() && a != b)
                .count();
            block.parameters = scope.vec(count)?;
            let mut left = scope.vec(count)?;
            let mut right = scope.vec(count)?;
            for (index, slot) in state.iter_mut().enumerate() {
                match (outgoing[1][index], outgoing[2][index]) {
                    (Some(a), Some(b)) if a == b => *slot = Some(a),
                    (Some(a), Some(b)) => {
                        let reg = Register::from_state_index(index).ok_or(Error::Instruction)?;
                        let value = fresh(&mut next)?;
                        block
                            .parameters
                            .push(ValueDef::new(value, Type::Scalar(reg.scalar_type())));
                        left.push(a);
                        right.push(b);
                        *slot = Some(value);
                    }
                    _ => {}
                }
            }
            match &mut blocks[1].terminator {
                Some(Terminator::Branch {
                    target: BlockId(3),
                    arguments,
                }) => *arguments = left,
                _ => return Err(Error::Shape),
            }
            match &mut blocks[2].terminator {
                Some(Terminator::Branch {
                    target: BlockId(3),
                    arguments,
                }) => *arguments = right,
                _ => return Err(Error::Shape),
            }
        }
        block.operations = scope.vec(source.end - source.first + usize::from(ordinal == 0))?;
        if ordinal == 0 {
            let contracts = parsed.blocks.map(|b| b.contract);
            block.operations.push(Operation::new(
                std::mem::take(&mut declaration_results),
                OperationKind::Gfx942PhysicalEntryDeclaration(Declaration {
                    origin: input.origin,
                    begin_site: input.begin,
                    parameters: PARAMETERS,
                    block_count: parsed.count as u8,
                    native_instruction_count: parsed.native,
                    workgroup: [64, 1, 1],
                    maximum_workgroups: [2, 1, 1],
                    blocks: contracts,
                }),
            ));
        }
        for event in &input.events[source.first..source.end] {
            let EventKind::Step(instruction) = event.kind else {
                return Err(Error::Shape);
            };
            let mut operands = [None; 6];
            for (slot, reg) in operands.iter_mut().zip(instruction.operand_registers()) {
                if let Some(reg) = reg {
                    *slot = Some(read(&state, reg)?);
                }
            }
            let registers = instruction.result_registers();
            let mut results = scope.vec(registers.iter().flatten().count())?;
            for reg in registers.into_iter().flatten() {
                let value = fresh(&mut next)?;
                state[reg.state_index().ok_or(Error::Instruction)?] = Some(value);
                results.push(ValueDef::new(value, Type::Scalar(reg.scalar_type())));
            }
            block.operations.push(Operation::new(
                results,
                OperationKind::Gfx942PhysicalEntryStep(Step {
                    site: event.site,
                    native_ordinal: native,
                    instruction,
                    operands,
                }),
            ));
            native = native.checked_add(1).ok_or(Error::Count)?;
        }
        block.terminator = Some(match source.contract.encoding {
            Encoding::Scc1 => Terminator::ConditionalBranch {
                condition: read(&state, Register::Scc)?,
                then_target: BlockId(2),
                then_arguments: Vec::new(),
                else_target: BlockId(1),
                else_arguments: Vec::new(),
            },
            Encoding::Jump | Encoding::Fallthrough => Terminator::Branch {
                target: BlockId(3),
                arguments: Vec::new(),
            },
            Encoding::Endpgm0 => Terminator::Return { values: Vec::new() },
            Encoding::Inactive => return Err(Error::Shape),
        });
        if source.contract.encoding != Encoding::Fallthrough {
            if source.contract.native_ordinal != Some(native) {
                return Err(Error::Accounting);
            }
            native = native.checked_add(1).ok_or(Error::Count)?;
        }
        outgoing[ordinal] = state;
        blocks.push(block);
    }
    if native != parsed.native {
        return Err(Error::Accounting);
    }
    let mut types = scope.vec(5)?;
    scope.reserve(std::mem::size_of::<Type>())?;
    types.push(Type::slice(
        Type::Scalar(ScalarType::U32),
        fe2o3_kernel_ir::AddressSpace::Global,
        fe2o3_kernel_ir::AccessMode::ReadWrite,
    ));
    types.extend((0..4).map(|_| Type::Scalar(ScalarType::U32)));
    let parameters = scope.copy(&PARAMETERS)?;
    let name = scope.string(input.export_name)?;
    let function = Function {
        id: name.into(),
        signature: Signature::new(types, Vec::new()),
        role: FunctionRole::KernelEntry,
        body: Some(fe2o3_kernel_ir::FunctionBody { parameters, blocks }),
        required_capabilities: std::collections::BTreeSet::new(),
    };
    Ok(Pending {
        function,
        retained: scope.bytes(),
    })
}
