//! Bounded source-event to actual KIR22 SSA construction. No interpreter,
//! caller-selected execution plan, source authentication or artifact admission.
use fe2o3_kernel_ir::{
    BasicBlock, BlockId, CanonicalKernelIrVerificationResourceBudgetV1 as Budget, Function,
    FunctionRole, GFX942_PHYSICAL_ENTRY_REGISTERS_V20,
    Gfx942PhysicalEntryBlockContractVNext as Contract,
    Gfx942PhysicalEntryBranchEncodingVNext as Encoding, Gfx942PhysicalEntryOriginVNext as Origin,
    Gfx942PhysicalEntryRegisterV20 as Register, Gfx942PhysicalEntrySourceSiteVNext as Site,
    Gfx942PhysicalLdsExchangeDeclarationV1 as Declaration,
    Gfx942PhysicalLdsExchangeInstructionV1 as Instruction,
    Gfx942PhysicalLdsExchangeOpcodeV1 as Opcode, Gfx942PhysicalLdsExchangeStepV1 as Step,
    Operation, OperationKind, ScalarType, Signature, Terminator, Type, ValueDef, ValueId,
};
// Reuse the existing prepaid logical-payload helper, not a second allocator/account.
pub(crate) use crate::physical_entry_materialization_v20::Error;
use crate::physical_entry_materialization_v20::SharedConstructionScope as Scope;

#[cfg(test)]
#[path = "physical_lds_exchange_materialization_v22_tests.rs"]
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
    pub(crate) lds_frame: fe2o3_kernel_ir::Gfx942PhysicalLdsExchangeFrameV1,
    pub(crate) events: &'a [Event],
}
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
const PARAMETERS: [ValueId; 2] = [ValueId(0), ValueId(1)];
const WORK: usize = 32768;
type State = [Option<ValueId>; 131];

fn parse(input: &Input<'_>) -> Result<Contract, Error> {
    if input.lds_frame
        != (fe2o3_kernel_ir::Gfx942PhysicalLdsExchangeFrameV1 {
            byte_offset: 0,
            byte_length: 512,
            alignment: 4,
            publication_epoch: 1,
        })
    {
        return Err(Error::Shape);
    }
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
    if !input.begin.is_complete() || input.begin.occurrence != 0 || input.events.len() > 41 {
        return Err(Error::Occurrence);
    }
    if input.events.len() < 2 {
        return Err(Error::Shape);
    }
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
            EventKind::Label(0) if index == 0 => {}
            EventKind::Step(instruction) if index > 0 => {
                instruction
                    .validate_shape()
                    .map_err(|_| Error::Instruction)?;
                if (instruction.opcode == Opcode::Endpgm0) != (index + 1 == input.events.len()) {
                    return Err(Error::Shape);
                }
            }
            _ => return Err(Error::Shape),
        }
    }
    let native = u8::try_from(input.events.len() - 1).map_err(|_| Error::Count)?;
    if native == 0 || native > 40 {
        return Err(Error::Count);
    }
    Ok(Contract {
        label: 0,
        label_site: input.events[0].site,
        encoding: Encoding::Endpgm0,
        terminator_site: input.events.last().ok_or(Error::Shape)?.site,
        native_ordinal: Some(native - 1),
    })
}
fn fresh(next: &mut u32) -> Result<ValueId, Error> {
    if *next >= 192 {
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

/// Only the sole semantic importer consumes this pending graph. Independent
/// KIR22 owner admission checks general SSA/types and exact pointer/readiness
/// facts, including immediate VM completion, before retaining any graph.
pub(crate) fn materialize(input: &Input<'_>, budget: &mut Budget<'_>) -> Result<Pending, Error> {
    budget.charge_work(WORK)?;
    let contract = parse(input)?;
    let native = u8::try_from(input.events.len() - 1).map_err(|_| Error::Count)?;
    let mut scope = Scope::new(budget);
    scope.reserve(std::mem::size_of::<Pending>())?;
    let mut blocks = scope.vec(1)?;
    let mut state = [None; 131];
    let mut next = 2;
    let mut declaration_results = scope.vec(5)?;
    for reg in GFX942_PHYSICAL_ENTRY_REGISTERS_V20 {
        let value = fresh(&mut next)?;
        state[reg.state_index().ok_or(Error::Instruction)?] = Some(value);
        declaration_results.push(ValueDef::new(value, Type::Scalar(reg.scalar_type())));
    }
    let mut block = BasicBlock::new(BlockId(0));
    block.operations = scope.vec(input.events.len() - 1)?;
    block.operations.push(Operation::new(
        declaration_results,
        OperationKind::Gfx942PhysicalLdsExchangeDeclaration(Declaration {
            origin: input.origin,
            begin_site: input.begin,
            parameters: PARAMETERS,
            native_instruction_count: native,
            workgroup: [128, 1, 1],
            maximum_workgroups: [1, 1, 1],
            lds_frame: input.lds_frame,
            block: contract,
        }),
    ));
    for (ordinal, event) in input.events[1..input.events.len() - 1].iter().enumerate() {
        let EventKind::Step(instruction) = event.kind else {
            return Err(Error::Shape);
        };
        let mut operands = [None; 5];
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
            OperationKind::Gfx942PhysicalLdsExchangeStep(Step {
                site: event.site,
                native_ordinal: u8::try_from(ordinal).map_err(|_| Error::Count)?,
                instruction,
                operands,
            }),
        ));
    }
    if contract.native_ordinal != Some(native - 1) {
        return Err(Error::Accounting);
    }
    block.terminator = Some(Terminator::Return { values: Vec::new() });
    blocks.push(block);
    let mut types = scope.vec(2)?;
    scope.reserve(2 * std::mem::size_of::<Type>())?;
    for access in [
        fe2o3_kernel_ir::AccessMode::ReadOnly,
        fe2o3_kernel_ir::AccessMode::ReadWrite,
    ] {
        types.push(Type::slice(
            Type::Scalar(ScalarType::U32),
            fe2o3_kernel_ir::AddressSpace::Global,
            access,
        ));
    }
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
