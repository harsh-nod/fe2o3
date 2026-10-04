//! Bounded exact physical-unit and provenance state. No executable shadow graph.
use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct AddressChain {
    pub pointer: ValueId,
    pub index: ValueId,
    pub shift: ValueId,
    pub low_add: ValueId,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Fact {
    KernargLow,
    KernargHigh,
    GroupX,
    LaneX,
    FullExec(ValueId),
    PointerLow(ValueId),
    PointerHigh(ValueId),
    LengthLow(ValueId),
    LengthHigh(ValueId),
    Scalar(u8),
    SelectedScalar,
    Zero,
    GroupBase,
    Index(ValueId),
    OffsetLow { index: ValueId, shift: ValueId },
    OffsetHigh { index: ValueId, shift: ValueId },
    AddressLow(AddressChain),
    AddressHigh(AddressChain),
    LowCarry(AddressChain),
    HighCarry(AddressChain),
    BoundsMask(ValueId),
    SelectorZero,
    OtherScc,
    SavedExecLow { mask: ValueId, entry: ValueId },
    SavedExecHigh { mask: ValueId, entry: ValueId },
    MaskedExec { mask: ValueId, index: ValueId },
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Cell {
    pub id: ValueId,
    pub fact: Fact,
    pub ready: bool,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Phase {
    Full,
    Store { mask: ValueId, index: ValueId },
    Wait { mask: ValueId, index: ValueId },
    Restore { mask: ValueId },
    End,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct State {
    pub cells: [Option<Cell>; 131],
    pub phase: Phase,
}
impl State {
    pub const EMPTY: Self = Self {
        cells: [None; 131],
        phase: Phase::Full,
    };
    pub fn get(&self, register: Register) -> Result<Cell> {
        let index = register
            .state_index()
            .ok_or(invalid("physical register bound"))?;
        self.cells[index].ok_or(invalid("physical undefined register unit"))
    }
    pub fn set(&mut self, register: Register, cell: Cell) -> Result<()> {
        let index = register
            .state_index()
            .ok_or(invalid("physical register bound"))?;
        require(
            self.cells[index].is_none_or(|old| old.ready),
            "physical pending-load destination clobber",
        )?;
        self.cells[index] = Some(cell);
        Ok(())
    }
    pub fn ready(&self) -> bool {
        self.cells.iter().flatten().all(|cell| cell.ready)
    }
    pub fn full(&self) -> Result<ValueId> {
        match self.get(Register::Exec)?.fact {
            Fact::FullExec(entry) if self.phase == Phase::Full => Ok(entry),
            _ => Err(invalid("physical vector/control requires full EXEC")),
        }
    }
}

pub(super) fn results(operation: &Operation, registers: &[Option<Register>]) -> Result<()> {
    require(
        operation.results.len() == registers.iter().flatten().count(),
        "physical exact result count",
    )?;
    for (result, register) in operation.results.iter().zip(registers.iter().flatten()) {
        require(
            result.ty == Type::Scalar(register.scalar_type()),
            "physical exact result type",
        )?;
    }
    Ok(())
}
pub(super) fn initialize(operation: &Operation) -> Result<State> {
    let registers = crate::GFX942_PHYSICAL_ENTRY_REGISTERS_V20.map(Some);
    results(operation, &registers)?;
    let exec = operation.results[4].id;
    let facts = [
        Fact::KernargLow,
        Fact::KernargHigh,
        Fact::GroupX,
        Fact::LaneX,
        Fact::FullExec(exec),
    ];
    let mut state = State::EMPTY;
    for ((definition, register), fact) in operation
        .results
        .iter()
        .zip(registers.into_iter().flatten())
        .zip(facts)
    {
        state.set(
            register,
            Cell {
                id: definition.id,
                fact,
                ready: true,
            },
        )?;
    }
    Ok(state)
}

pub(super) fn join(
    arms: [&State; 2],
    block: &BasicBlock,
    arguments: [&[ValueId]; 2],
) -> Result<State> {
    require(
        arms.iter()
            .all(|arm| arm.phase == Phase::Full && arm.ready()),
        "physical unsafe join phase",
    )?;
    let mut result = State::EMPTY;
    let mut parameter = 0;
    for unit in 0..131 {
        let (Some(left), Some(right)) = (arms[0].cells[unit], arms[1].cells[unit]) else {
            continue;
        };
        if left.id == right.id {
            require(left == right, "physical same SSA has inconsistent facts")?;
            result.cells[unit] = Some(left);
            continue;
        }
        let fact = if left.fact == right.fact {
            left.fact
        } else {
            match (left.fact, right.fact) {
                // Block1 is the nonzero arm, block2 the zero arm.
                (Fact::Scalar(2), Fact::Scalar(1)) => Fact::SelectedScalar,
                _ => return Err(invalid("physical incompatible merge provenance")),
            }
        };
        let definition = block
            .parameters
            .get(parameter)
            .ok_or(invalid("physical missing merge parameter"))?;
        let register =
            Register::from_state_index(unit).ok_or(invalid("physical merge register index"))?;
        require(
            definition.ty == Type::Scalar(register.scalar_type()),
            "physical merge parameter type",
        )?;
        require(
            arguments[0].get(parameter) == Some(&left.id)
                && arguments[1].get(parameter) == Some(&right.id),
            "physical exact current register edge operands",
        )?;
        result.cells[unit] = Some(Cell {
            id: definition.id,
            fact,
            ready: true,
        });
        parameter += 1;
    }
    require(
        block.parameters.len() == parameter
            && arguments.iter().all(|values| values.len() == parameter),
        "physical exact merge parameter/edge roster",
    )?;
    Ok(result)
}
