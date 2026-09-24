//! Single bounded state for the actual canonical block; not an executable graph.
use super::*;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct AddressChain {
    pub root: u8,
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
    PointerLow { root: u8, generation: ValueId },
    PointerHigh { root: u8, generation: ValueId },
    LengthLow { root: u8, generation: ValueId },
    LengthHigh { root: u8, generation: ValueId },
    Zero,
    GroupBase,
    Index(ValueId),
    OffsetLow { index: ValueId, shift: ValueId },
    OffsetHigh { index: ValueId, shift: ValueId },
    AddressLow(AddressChain),
    AddressHigh(AddressChain),
    LowCarry(AddressChain),
    HighCarry(AddressChain),
    Loaded { load: ValueId, index: ValueId },
    BoundsMask(ValueId),
    OtherScc,
    SavedExecLow { mask: ValueId, entry: ValueId },
    SavedExecHigh { mask: ValueId, entry: ValueId },
    MaskedExec { mask: ValueId, index: ValueId },
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Readiness {
    Ready,
    LgkmPending,
    VmPending(ValueId),
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Cell {
    pub id: ValueId,
    pub fact: Fact,
    pub readiness: Readiness,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Phase {
    Kernarg,
    Full,
    ReadWait { load: ValueId, destination: u8 },
    Store { mask: ValueId, index: ValueId },
    StoreWait { mask: ValueId },
    Restore { mask: ValueId },
    End,
}
pub(super) struct State {
    pub cells: [Option<Cell>; 131],
    pub phase: Phase,
    pub kernarg_roster: u8,
    pub read: Option<(ValueId, ValueId)>,
}
impl State {
    pub fn get(&self, register: Register) -> Result<Cell> {
        let index = register
            .state_index()
            .ok_or(invalid("global copy register bound"))?;
        self.cells[index].ok_or(invalid("global copy undefined physical unit"))
    }
    pub fn set(&mut self, register: Register, cell: Cell) -> Result<()> {
        let index = register
            .state_index()
            .ok_or(invalid("global copy register bound"))?;
        require(
            self.cells[index].is_none_or(|old| old.readiness == Readiness::Ready),
            "global copy pending destination overwrite",
        )?;
        self.cells[index] = Some(cell);
        Ok(())
    }
    pub fn ready(&self) -> bool {
        self.cells
            .iter()
            .flatten()
            .all(|cell| cell.readiness == Readiness::Ready)
    }
    pub fn full(&self) -> Result<ValueId> {
        match self.get(Register::Exec)?.fact {
            Fact::FullExec(entry) if self.phase == Phase::Full => Ok(entry),
            _ => Err(invalid("global copy vector requires full EXEC")),
        }
    }
}
pub(super) fn results(operation: &Operation, registers: &[Option<Register>]) -> Result<()> {
    require(
        operation.results.len() == registers.iter().flatten().count(),
        "global copy result count",
    )?;
    for (definition, register) in operation.results.iter().zip(registers.iter().flatten()) {
        require(
            definition.ty == Type::Scalar(register.scalar_type()),
            "global copy result type",
        )?;
    }
    Ok(())
}
pub(super) fn initialize(operation: &Operation) -> Result<State> {
    let registers = crate::GFX942_PHYSICAL_ENTRY_REGISTERS_V20.map(Some);
    results(operation, &registers)?;
    let entry = operation.results[4].id;
    let facts = [
        Fact::KernargLow,
        Fact::KernargHigh,
        Fact::GroupX,
        Fact::LaneX,
        Fact::FullExec(entry),
    ];
    let mut state = State {
        cells: [None; 131],
        phase: Phase::Kernarg,
        kernarg_roster: 0,
        read: None,
    };
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
                readiness: Readiness::Ready,
            },
        )?;
    }
    Ok(state)
}
