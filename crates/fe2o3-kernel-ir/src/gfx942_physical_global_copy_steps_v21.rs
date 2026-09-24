//! Closed readiness and exact pointer/SSA proof at each actual canonical operation.
use super::state::{AddressChain, Cell, Fact, Phase, Readiness, State, results};
use super::*;

pub(super) fn verify_step(state: &mut State, operation: &Operation, step: &Step) -> Result<()> {
    use Opcode as O;
    step.validate_shape()
        .map_err(|_| invalid("global copy instruction shape"))?;
    let instruction = step.instruction;
    let opcode = instruction.opcode;
    let writes = instruction.result_registers();
    results(operation, &writes)?;
    match state.phase {
        Phase::Kernarg => require(
            matches!(opcode, O::LoadKernargPair | O::WaitLgkm0),
            "global copy requires exact initial kernarg loads/wait",
        )?,
        Phase::Full => require(
            !matches!(
                opcode,
                O::LoadKernargPair
                    | O::WaitLgkm0
                    | O::WaitVm0
                    | O::GlobalStoreDword
                    | O::RestoreExec
            ),
            "global copy wrong phase or unmatched wait",
        )?,
        Phase::ReadWait { .. } => require(
            opcode == O::WaitVm0,
            "global copy read requires immediate VM wait",
        )?,
        Phase::Store { .. } => require(
            opcode == O::GlobalStoreDword,
            "global copy masked suffix requires store",
        )?,
        Phase::StoreWait { .. } => require(
            opcode == O::WaitVm0,
            "global copy masked suffix requires store wait",
        )?,
        Phase::Restore { .. } => require(
            opcode == O::RestoreExec,
            "global copy masked suffix requires restore",
        )?,
        Phase::End => return Err(invalid("global copy instruction after restored tail")),
    }
    if opcode.is_vector_definition() {
        state.full()?;
    }
    let mut input = [None; 5];
    for (slot, register) in instruction.operand_registers().iter().enumerate() {
        if let Some(register) = register {
            let cell = state.get(*register)?;
            require(
                cell.readiness == Readiness::Ready,
                "global copy operand used before matching wait",
            )?;
            require(
                step.operands[slot] == Some(cell.id),
                "global copy operand is not current exact SSA",
            )?;
            input[slot] = Some(cell);
        }
    }
    let get = |slot: usize| -> Result<Cell> {
        input
            .get(slot)
            .copied()
            .flatten()
            .ok_or(invalid("global copy operand prefix"))
    };
    let id = |slot: usize| -> Result<ValueId> {
        operation
            .results
            .get(slot)
            .map(|definition| definition.id)
            .ok_or(invalid("global copy result prefix"))
    };
    let mut facts = [None; 4];
    let mut readiness = Readiness::Ready;
    match opcode {
        O::LoadKernargPair => {
            require(
                get(0)?.fact == Fact::KernargLow && get(1)?.fact == Fact::KernargHigh,
                "global copy kernarg base provenance",
            )?;
            let slot = instruction.immediate / 8;
            let bit = 1u8 << slot;
            require(
                state.kernarg_roster & bit == 0,
                "global copy repeated kernarg component",
            )?;
            state.kernarg_roster |= bit;
            let root = (slot / 2) as u8;
            let generation = id(0)?;
            let (low, high) = if slot.is_multiple_of(2) {
                (
                    Fact::PointerLow { root, generation },
                    Fact::PointerHigh { root, generation },
                )
            } else {
                (
                    Fact::LengthLow { root, generation },
                    Fact::LengthHigh { root, generation },
                )
            };
            facts[0] = Some(low);
            facts[1] = Some(high);
            readiness = Readiness::LgkmPending;
        }
        O::WaitLgkm0 => {
            require(
                state.kernarg_roster == 15,
                "global copy requires four exact kernarg pairs",
            )?;
            require(
                state
                    .cells
                    .iter()
                    .flatten()
                    .filter(|cell| cell.readiness == Readiness::LgkmPending)
                    .count()
                    == 8,
                "global copy incomplete initial pending kernarg roster",
            )?;
            for cell in state.cells.iter_mut().flatten() {
                if cell.readiness == Readiness::LgkmPending {
                    cell.readiness = Readiness::Ready;
                }
            }
            state.phase = Phase::Full;
        }
        O::ScalarLshl32 => {
            require(
                get(0)?.fact == Fact::GroupX,
                "global copy group index source",
            )?;
            facts[0] = Some(Fact::GroupBase);
            facts[1] = Some(Fact::OtherScc);
        }
        O::VectorAddU32 => {
            require(
                get(0)?.fact == Fact::GroupBase && get(1)?.fact == Fact::LaneX,
                "global copy actual launch index sources",
            )?;
            facts[0] = Some(Fact::Index(id(0)?));
        }
        O::VectorMove32 => {
            facts[0] = Some(if instruction.source0 == 255 {
                Fact::Zero
            } else {
                match get(0)?.fact {
                    value @ Fact::PointerHigh { .. } => value,
                    _ => return Err(invalid("global copy scalar move must retain pointer high")),
                }
            });
        }
        O::VectorLshlrev64 => {
            let Fact::Index(index) = get(0)?.fact else {
                return Err(invalid("global copy scaled offset index"));
            };
            require(
                get(1)?.fact == Fact::Zero,
                "global copy index high must be zero",
            )?;
            let shift = id(0)?;
            facts[0] = Some(Fact::OffsetLow { index, shift });
            facts[1] = Some(Fact::OffsetHigh { index, shift });
        }
        O::VectorAddCarry => {
            let Fact::PointerLow {
                root,
                generation: pointer,
            } = get(0)?.fact
            else {
                return Err(invalid("global copy pointer low provenance"));
            };
            let Fact::OffsetLow { index, shift } = get(1)?.fact else {
                return Err(invalid("global copy pointer low displacement"));
            };
            let chain = AddressChain {
                root,
                pointer,
                index,
                shift,
                low_add: id(0)?,
            };
            facts[0] = Some(Fact::AddressLow(chain));
            facts[1] = Some(Fact::LowCarry(chain));
        }
        O::VectorAddCarryIn => {
            let Fact::PointerHigh {
                root,
                generation: pointer,
            } = get(0)?.fact
            else {
                return Err(invalid("global copy pointer high provenance"));
            };
            let Fact::OffsetHigh { index, shift } = get(1)?.fact else {
                return Err(invalid("global copy pointer high displacement"));
            };
            let Fact::LowCarry(chain) = get(3)?.fact else {
                return Err(invalid("global copy exact low carry generation"));
            };
            require(
                chain.root == root
                    && chain.pointer == pointer
                    && chain.index == index
                    && chain.shift == shift,
                "global copy pointer root/half/carry/offset join",
            )?;
            require(
                state.cells.iter().flatten().any(|cell| {
                    cell.id == chain.low_add
                        && cell.fact == Fact::AddressLow(chain)
                        && cell.readiness == Readiness::Ready
                }),
                "global copy low address clobbered before carry completion",
            )?;
            facts[0] = Some(Fact::AddressHigh(chain));
            facts[1] = Some(Fact::HighCarry(chain));
        }
        O::GlobalLoadDword => {
            require(state.read.is_none(), "global copy exactly one global load")?;
            let chain = address(get(0)?, get(1)?)?;
            require(chain.root == 0, "global copy read must use input root")?;
            let load = id(0)?;
            facts[0] = Some(Fact::Loaded {
                load,
                index: chain.index,
            });
            readiness = Readiness::VmPending(load);
            state.read = Some((load, chain.index));
            state.phase = Phase::ReadWait {
                load,
                destination: instruction.destination,
            };
        }
        O::WaitVm0 => match state.phase {
            Phase::ReadWait { load, destination } => {
                let register = Register::Vgpr(destination);
                let cell = state.get(register)?;
                require(
                    cell.id == load
                        && cell.readiness == Readiness::VmPending(load)
                        && matches!(cell.fact,Fact::Loaded{load:actual,index} if actual==load && state.read==Some((load,index))),
                    "global copy exact pending load/result/destination",
                )?;
                let index = register
                    .state_index()
                    .ok_or(invalid("global copy pending unit"))?;
                state.cells[index] = Some(Cell {
                    readiness: Readiness::Ready,
                    ..cell
                });
                state.phase = Phase::Full;
            }
            Phase::StoreWait { mask } => state.phase = Phase::Restore { mask },
            _ => return Err(invalid("global copy unmatched VM wait")),
        },
        O::VectorCompareGtU64 => {
            let Fact::LengthLow {
                root: 1,
                generation,
            } = get(0)?.fact
            else {
                return Err(invalid("global copy output length low provenance"));
            };
            require(
                get(1)?.fact
                    == Fact::LengthHigh {
                        root: 1,
                        generation,
                    },
                "global copy output length high generation",
            )?;
            let Fact::Index(index) = get(2)?.fact else {
                return Err(invalid("global copy bounds index"));
            };
            require(get(3)?.fact == Fact::Zero, "global copy bounds index high")?;
            require(
                state
                    .read
                    .is_some_and(|(_, read_index)| read_index == index),
                "global copy output guard must use actual read index",
            )?;
            facts[0] = Some(Fact::BoundsMask(index));
        }
        O::SaveAndMaskExec => {
            let Fact::BoundsMask(index) = get(0)?.fact else {
                return Err(invalid(
                    "global copy mask requires bounds not symbolic carry",
                ));
            };
            require(state.ready(), "global copy mask with pending load")?;
            let entry = state.full()?;
            let mask = id(2)?;
            facts[0] = Some(Fact::SavedExecLow { mask, entry });
            facts[1] = Some(Fact::SavedExecHigh { mask, entry });
            facts[2] = Some(Fact::MaskedExec { mask, index });
            facts[3] = Some(Fact::OtherScc);
            state.phase = Phase::Store { mask, index };
        }
        O::GlobalStoreDword => {
            let Phase::Store { mask, index } = state.phase else {
                return Err(invalid("global copy store phase"));
            };
            let chain = address(get(0)?, get(1)?)?;
            require(
                chain.root == 1 && chain.index == index,
                "global copy store output root/index",
            )?;
            let Fact::Loaded {
                load,
                index: read_index,
            } = get(2)?.fact
            else {
                return Err(invalid("global copy store must consume actual loaded U32"));
            };
            require(
                get(2)?.id == load && read_index == index && state.read == Some((load, index)),
                "global copy exact original read result/index",
            )?;
            require(
                get(3)?.fact == Fact::MaskedExec { mask, index },
                "global copy store EXEC generation",
            )?;
            state.phase = Phase::StoreWait { mask };
        }
        O::RestoreExec => {
            let Phase::Restore { mask } = state.phase else {
                return Err(invalid("global copy restore phase"));
            };
            let Fact::SavedExecLow { mask: saved, entry } = get(0)?.fact else {
                return Err(invalid("global copy saved EXEC low"));
            };
            require(
                saved == mask && get(1)?.fact == Fact::SavedExecHigh { mask, entry },
                "global copy exact saved EXEC pair",
            )?;
            facts[0] = Some(Fact::FullExec(entry));
            state.phase = Phase::End;
        }
        O::Endpgm0 => return Err(invalid("global copy end must be actual Return terminator")),
    }
    for (slot, register) in writes.into_iter().enumerate() {
        if let Some(register) = register {
            state.set(
                register,
                Cell {
                    id: id(slot)?,
                    fact: facts[slot].ok_or(invalid("global copy missing result fact"))?,
                    readiness,
                },
            )?;
        }
    }
    Ok(())
}
fn address(low: Cell, high: Cell) -> Result<AddressChain> {
    let Fact::AddressLow(chain) = low.fact else {
        return Err(invalid("global copy address low"));
    };
    require(
        low.id == chain.low_add && high.fact == Fact::AddressHigh(chain),
        "global copy exact completed address pair",
    )?;
    Ok(chain)
}
