//! Closed per-instruction state proof, independent of source packing and emitter.
use super::state::{AddressChain, Cell, Fact, Phase, State, results};
use super::*;

pub(super) fn verify_step(
    state: &mut State,
    operation: &Operation,
    step: &Step,
    select: bool,
    last_block: bool,
) -> Result<()> {
    use Opcode as O;
    step.validate_shape()
        .map_err(|_| invalid("physical instruction shape"))?;
    let instruction = step.instruction;
    let opcode = instruction.opcode;
    let writes = instruction.result_registers();
    results(operation, &writes)?;
    if opcode.is_vector_definition() {
        state.full()?;
    }
    match state.phase {
        Phase::Full => require(
            !matches!(opcode, O::GlobalStoreDword | O::WaitVm0 | O::RestoreExec),
            "physical tail operation before mask",
        )?,
        Phase::Store { .. } => require(
            opcode == O::GlobalStoreDword,
            "physical masked suffix requires single store",
        )?,
        Phase::Wait { .. } => require(
            opcode == O::WaitVm0,
            "physical masked suffix requires store wait",
        )?,
        Phase::Restore { .. } => require(
            opcode == O::RestoreExec,
            "physical masked suffix requires restore",
        )?,
        Phase::End => return Err(invalid("physical operation after restored tail")),
    }
    let mut input = [None; 6];
    for (slot, register) in instruction.operand_registers().iter().enumerate() {
        if let Some(register) = register {
            let cell = state.get(*register)?;
            require(cell.ready, "physical load used before lgkm wait")?;
            require(
                step.operands[slot] == Some(cell.id),
                "physical operand is not current exact SSA",
            )?;
            input[slot] = Some(cell);
        }
    }
    let get = |slot: usize| -> Result<Cell> {
        input
            .get(slot)
            .copied()
            .flatten()
            .ok_or(invalid("physical operand roster"))
    };
    let id = |slot: usize| -> Result<ValueId> {
        operation
            .results
            .get(slot)
            .map(|result| result.id)
            .ok_or(invalid("physical result roster"))
    };
    let mut facts = [None; 4];
    let mut ready = true;
    match opcode {
        O::LoadKernargPair => {
            require(
                get(0)?.fact == Fact::KernargLow && get(1)?.fact == Fact::KernargHigh,
                "physical kernarg base provenance",
            )?;
            let generation = id(0)?;
            let (low, high) = if instruction.immediate == 0 {
                (Fact::PointerLow(generation), Fact::PointerHigh(generation))
            } else {
                (Fact::LengthLow(generation), Fact::LengthHigh(generation))
            };
            facts[0] = Some(low);
            facts[1] = Some(high);
            ready = false;
        }
        O::LoadKernargDword => {
            require(
                get(0)?.fact == Fact::KernargLow && get(1)?.fact == Fact::KernargHigh,
                "physical kernarg base provenance",
            )?;
            facts[0] = Some(Fact::Scalar(((instruction.immediate - 16) / 4 + 1) as u8));
            ready = false;
        }
        O::WaitLgkm0 => {
            for cell in state.cells.iter_mut().flatten() {
                cell.ready = true;
            }
        }
        O::ScalarLshl32 => {
            require(get(0)?.fact == Fact::GroupX, "physical index group source")?;
            facts[0] = Some(Fact::GroupBase);
            facts[1] = Some(Fact::OtherScc);
        }
        O::VectorAddU32 => {
            require(
                get(0)?.fact == Fact::GroupBase && get(1)?.fact == Fact::LaneX,
                "physical exact launch index sources",
            )?;
            facts[0] = Some(Fact::Index(id(0)?));
        }
        O::VectorMove32 => {
            facts[0] = Some(if instruction.source0 == 255 {
                Fact::Zero
            } else {
                match get(0)?.fact {
                    value @ (Fact::Scalar(_) | Fact::PointerHigh(_)) => value,
                    _ => {
                        return Err(invalid(
                            "physical move escapes permitted scalar/pointer-high provenance",
                        ));
                    }
                }
            });
        }
        O::ScalarCompareEqZero => {
            require(
                get(0)?.fact == Fact::Scalar(4),
                "physical selector compare source",
            )?;
            facts[0] = Some(Fact::SelectorZero);
        }
        O::VectorLshlrev64 => {
            let Fact::Index(index) = get(0)?.fact else {
                return Err(invalid("physical scaled offset index"));
            };
            require(
                get(1)?.fact == Fact::Zero,
                "physical index high word must be zero",
            )?;
            let shift = id(0)?;
            facts[0] = Some(Fact::OffsetLow { index, shift });
            facts[1] = Some(Fact::OffsetHigh { index, shift });
        }
        O::VectorAddCarry => {
            let Fact::PointerLow(pointer) = get(0)?.fact else {
                return Err(invalid("physical pointer low origin"));
            };
            let Fact::OffsetLow { index, shift } = get(1)?.fact else {
                return Err(invalid("physical pointer low offset"));
            };
            let chain = AddressChain {
                pointer,
                index,
                shift,
                low_add: id(0)?,
            };
            facts[0] = Some(Fact::AddressLow(chain));
            facts[1] = Some(Fact::LowCarry(chain));
        }
        O::VectorAddCarryIn => {
            let Fact::PointerHigh(pointer) = get(0)?.fact else {
                return Err(invalid("physical pointer high origin"));
            };
            let Fact::OffsetHigh { index, shift } = get(1)?.fact else {
                return Err(invalid("physical pointer high offset"));
            };
            let Fact::LowCarry(chain) = get(3)?.fact else {
                return Err(invalid("physical exact low-add carry generation"));
            };
            require(
                chain.pointer == pointer && chain.index == index && chain.shift == shift,
                "physical pointer-half/carry/offset join",
            )?;
            // The low address must still occupy its actual defining physical unit.
            require(
                state.cells.iter().flatten().any(|cell| {
                    cell.id == chain.low_add && cell.fact == Fact::AddressLow(chain) && cell.ready
                }),
                "physical low address clobbered before carry completion",
            )?;
            facts[0] = Some(Fact::AddressHigh(chain));
            facts[1] = Some(Fact::HighCarry(chain));
        }
        O::VectorCompareGtU64 => {
            let Fact::LengthLow(length) = get(0)?.fact else {
                return Err(invalid("physical bounds length low"));
            };
            require(
                get(1)?.fact == Fact::LengthHigh(length),
                "physical length high generation",
            )?;
            let Fact::Index(index) = get(2)?.fact else {
                return Err(invalid("physical bounds index"));
            };
            require(get(3)?.fact == Fact::Zero, "physical bounds index high")?;
            facts[0] = Some(Fact::BoundsMask(index));
        }
        O::SaveAndMaskExec => {
            require(last_block, "physical mask only in final block")?;
            let Fact::BoundsMask(index) = get(0)?.fact else {
                return Err(invalid(
                    "physical mask requires bounds VCC, not pointer carry",
                ));
            };
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
                return Err(invalid("physical store phase"));
            };
            let Fact::AddressLow(chain) = get(0)?.fact else {
                return Err(invalid("physical store address low"));
            };
            require(
                get(0)?.id == chain.low_add
                    && get(1)?.fact == Fact::AddressHigh(chain)
                    && chain.index == index,
                "physical exact store address/carry/index chain",
            )?;
            require(
                get(3)?.fact == (Fact::MaskedExec { mask, index }),
                "physical store EXEC generation",
            )?;
            require(
                get(2)?.fact
                    == if select {
                        Fact::SelectedScalar
                    } else {
                        Fact::Scalar(1)
                    },
                "physical copy/select stored value provenance",
            )?;
            state.phase = Phase::Wait { mask, index };
        }
        O::WaitVm0 => {
            let Phase::Wait { mask, .. } = state.phase else {
                return Err(invalid("physical store wait phase"));
            };
            state.phase = Phase::Restore { mask };
        }
        O::RestoreExec => {
            let Phase::Restore { mask } = state.phase else {
                return Err(invalid("physical restore phase"));
            };
            let Fact::SavedExecLow { mask: saved, entry } = get(0)?.fact else {
                return Err(invalid("physical saved EXEC low"));
            };
            require(
                saved == mask && get(1)?.fact == (Fact::SavedExecHigh { mask, entry }),
                "physical exact saved EXEC pair/generation",
            )?;
            facts[0] = Some(Fact::FullExec(entry));
            state.phase = Phase::End;
        }
        O::BranchScc1 | O::Branch | O::Endpgm0 | O::Fallthrough => {
            return Err(invalid("physical control must use actual CFG terminator"));
        }
    }
    for (slot, register) in writes.into_iter().enumerate() {
        if let Some(register) = register {
            state.set(
                register,
                Cell {
                    id: id(slot)?,
                    fact: facts[slot].ok_or(invalid("physical missing derived result fact"))?,
                    ready,
                },
            )?;
        }
    }
    Ok(())
}
