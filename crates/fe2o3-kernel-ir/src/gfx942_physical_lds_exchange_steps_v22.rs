//! Closed readiness and exact pointer/SSA proof at each actual canonical operation.
use super::state::{AddressChain, Cell, Fact, LdsRead, LdsWrite, Phase, Readiness, State, results};
use super::*;

pub(super) fn verify_step(state: &mut State, operation: &Operation, step: &Step) -> Result<()> {
    use Opcode as O;
    step.validate_shape()
        .map_err(|_| invalid("LDS exchange instruction shape"))?;
    let instruction = step.instruction;
    let opcode = instruction.opcode;
    let writes = instruction.result_registers();
    results(operation, &writes)?;
    match state.phase {
        Phase::Kernarg => require(
            matches!(opcode, O::LoadKernargPair | O::WaitLgkm0),
            "LDS exchange requires initial kernarg loads/wait",
        )?,
        Phase::Full => require(
            matches!(
                opcode,
                O::ScalarLshl32
                    | O::VectorAddU32
                    | O::VectorMove32
                    | O::VectorLshlrev64
                    | O::VectorAddCarry
                    | O::VectorAddCarryIn
                    | O::GlobalLoadDword
            ),
            "LDS exchange input-address phase",
        )?,
        Phase::ReadWait { .. } => require(
            opcode == O::WaitVm0,
            "LDS exchange global load requires immediate VM wait",
        )?,
        Phase::ReadyInput => require(
            matches!(opcode, O::VectorLshlrev32 | O::LdsWriteB32),
            "LDS exchange ready input requires local address/write",
        )?,
        Phase::LdsWriteWait | Phase::LdsReadWait { .. } => require(
            opcode == O::WaitLgkm0,
            "LDS exchange issue requires immediate LGKM wait",
        )?,
        Phase::PublishRequired => require(
            opcode == O::WorkgroupPublishBarrier,
            "LDS exchange waited write requires immediate publication",
        )?,
        Phase::Published => require(
            matches!(opcode, O::VectorXor32 | O::VectorLshlrev32 | O::LdsReadB32),
            "LDS exchange published peer-address/read phase",
        )?,
        Phase::LdsReady => require(
            matches!(
                opcode,
                O::VectorMove32
                    | O::VectorLshlrev64
                    | O::VectorAddCarry
                    | O::VectorAddCarryIn
                    | O::VectorCompareGtU64
                    | O::SaveAndMaskExec
            ),
            "LDS exchange ready output-address/mask phase",
        )?,
        Phase::Store { .. } => require(
            opcode == O::GlobalStoreDword,
            "LDS exchange masked suffix requires store",
        )?,
        Phase::StoreWait { .. } => require(
            opcode == O::WaitVm0,
            "LDS exchange masked suffix requires VM wait",
        )?,
        Phase::Restore { .. } => require(
            opcode == O::RestoreExec,
            "LDS exchange masked suffix requires restore",
        )?,
        Phase::End => return Err(invalid("LDS exchange instruction after restored tail")),
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
                "LDS exchange operand used before matching wait",
            )?;
            require(
                step.operands[slot] == Some(cell.id),
                "LDS exchange operand is not current exact SSA",
            )?;
            input[slot] = Some(cell);
        }
    }
    let get = |slot: usize| -> Result<Cell> {
        input
            .get(slot)
            .copied()
            .flatten()
            .ok_or(invalid("LDS exchange operand prefix"))
    };
    let id = |slot: usize| -> Result<ValueId> {
        operation
            .results
            .get(slot)
            .map(|definition| definition.id)
            .ok_or(invalid("LDS exchange result prefix"))
    };
    let mut facts = [None; 4];
    let mut readiness = Readiness::Ready;
    match opcode {
        O::LoadKernargPair => {
            require(
                get(0)?.fact == Fact::KernargLow && get(1)?.fact == Fact::KernargHigh,
                "LDS exchange kernarg base provenance",
            )?;
            let slot = instruction.immediate / 8;
            let bit = 1u8 << slot;
            require(
                state.kernarg_roster & bit == 0,
                "LDS exchange repeated kernarg component",
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
        O::WaitLgkm0 => match state.phase {
            Phase::Kernarg => {
                require(
                    state.kernarg_roster == 15,
                    "LDS exchange requires four kernarg pairs",
                )?;
                require(
                    state
                        .cells
                        .iter()
                        .flatten()
                        .filter(|cell| cell.readiness == Readiness::LgkmPending)
                        .count()
                        == 8,
                    "LDS exchange exact pending kernarg roster",
                )?;
                for cell in state.cells.iter_mut().flatten() {
                    if cell.readiness == Readiness::LgkmPending {
                        cell.readiness = Readiness::Ready;
                    }
                }
                state.phase = Phase::Full;
            }
            Phase::LdsWriteWait => {
                let write = state
                    .lds_write
                    .ok_or(invalid("LDS exchange missing pending write"))?;
                require(state.ready() && state.get(Register::Vgpr(0))?.id == write.local
                    && step.site.occurrence == write.site.occurrence + 1
                    && state.cells.iter().flatten().any(|c| c.id == write.address
                        && matches!(c.fact, Fact::LocalLdsOffset { local, shift } if local == write.local && shift == write.address))
                    && state.cells.iter().flatten().any(|c| c.id == write.data && matches!(c.fact, Fact::Loaded { load, .. } if load == write.data)),
                    "LDS exchange pending write exact address/data/site")?;
                state.phase = Phase::PublishRequired;
            }
            Phase::LdsReadWait { load, destination } => {
                let read = state
                    .lds_read
                    .ok_or(invalid("LDS exchange missing pending read"))?;
                let register = Register::Vgpr(destination);
                let cell = state.get(register)?;
                require(
                    read.load == load
                        && step.site.occurrence == read.site.occurrence + 1
                        && cell.id == load
                        && cell.readiness == Readiness::LdsPending(load)
                        && cell.fact
                            == Fact::LdsLoaded {
                                load,
                                peer: read.peer,
                                frame: state.declaration,
                                publish: read.publish,
                            }
                        && state.publication == Some(read.publish),
                    "LDS exchange read wait exact issue/result/publication",
                )?;
                let index = register
                    .state_index()
                    .ok_or(invalid("LDS exchange pending read unit"))?;
                state.cells[index] = Some(Cell {
                    readiness: Readiness::Ready,
                    ..cell
                });
                state.phase = Phase::LdsReady;
            }
            _ => return Err(invalid("LDS exchange unmatched LGKM wait")),
        },
        O::ScalarLshl32 => {
            require(
                get(0)?.fact == Fact::GroupX,
                "LDS exchange group index source",
            )?;
            facts[0] = Some(Fact::GroupBase);
            facts[1] = Some(Fact::OtherScc);
        }
        O::VectorAddU32 => {
            require(
                get(0)?.fact == Fact::GroupBase && get(1)?.fact == Fact::LaneX,
                "LDS exchange actual launch index sources",
            )?;
            facts[0] = Some(Fact::Index(id(0)?));
        }
        O::VectorMove32 => {
            facts[0] = Some(if instruction.source0 == 255 {
                Fact::Zero
            } else {
                match get(0)?.fact {
                    value @ Fact::PointerHigh { .. } => value,
                    _ => return Err(invalid("LDS exchange scalar move must retain pointer high")),
                }
            });
        }
        O::VectorLshlrev64 => {
            let Fact::Index(index) = get(0)?.fact else {
                return Err(invalid("LDS exchange scaled offset index"));
            };
            require(
                get(1)?.fact == Fact::Zero,
                "LDS exchange index high must be zero",
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
                return Err(invalid("LDS exchange pointer low provenance"));
            };
            let Fact::OffsetLow { index, shift } = get(1)?.fact else {
                return Err(invalid("LDS exchange pointer low displacement"));
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
                return Err(invalid("LDS exchange pointer high provenance"));
            };
            let Fact::OffsetHigh { index, shift } = get(1)?.fact else {
                return Err(invalid("LDS exchange pointer high displacement"));
            };
            let Fact::LowCarry(chain) = get(3)?.fact else {
                return Err(invalid("LDS exchange exact low carry generation"));
            };
            require(
                chain.root == root
                    && chain.pointer == pointer
                    && chain.index == index
                    && chain.shift == shift,
                "LDS exchange pointer root/half/carry/offset join",
            )?;
            require(
                state.cells.iter().flatten().any(|cell| {
                    cell.id == chain.low_add
                        && cell.fact == Fact::AddressLow(chain)
                        && cell.readiness == Readiness::Ready
                }),
                "LDS exchange low address clobbered before carry completion",
            )?;
            facts[0] = Some(Fact::AddressHigh(chain));
            facts[1] = Some(Fact::HighCarry(chain));
        }
        O::GlobalLoadDword => {
            require(state.read.is_none(), "LDS exchange exactly one global load")?;
            let chain = address(get(0)?, get(1)?)?;
            require(chain.root == 0, "LDS exchange read must use input root")?;
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
                    "LDS exchange exact pending load/result/destination",
                )?;
                let index = register
                    .state_index()
                    .ok_or(invalid("LDS exchange pending unit"))?;
                state.cells[index] = Some(Cell {
                    readiness: Readiness::Ready,
                    ..cell
                });
                state.phase = Phase::ReadyInput;
            }
            Phase::StoreWait { mask } => state.phase = Phase::Restore { mask },
            _ => return Err(invalid("LDS exchange unmatched VM wait")),
        },
        O::VectorCompareGtU64 => {
            let Fact::LengthLow {
                root: 1,
                generation,
            } = get(0)?.fact
            else {
                return Err(invalid("LDS exchange output length low provenance"));
            };
            require(
                get(1)?.fact
                    == Fact::LengthHigh {
                        root: 1,
                        generation,
                    },
                "LDS exchange output length high generation",
            )?;
            let Fact::Index(index) = get(2)?.fact else {
                return Err(invalid("LDS exchange bounds index"));
            };
            require(get(3)?.fact == Fact::Zero, "LDS exchange bounds index high")?;
            require(
                state
                    .read
                    .is_some_and(|(_, read_index)| read_index == index),
                "LDS exchange output guard must use actual read index",
            )?;
            facts[0] = Some(Fact::BoundsMask(index));
        }
        O::SaveAndMaskExec => {
            let Fact::BoundsMask(index) = get(0)?.fact else {
                return Err(invalid(
                    "LDS exchange mask requires bounds not symbolic carry",
                ));
            };
            require(state.ready(), "LDS exchange mask with pending load")?;
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
                return Err(invalid("LDS exchange store phase"));
            };
            let chain = address(get(0)?, get(1)?)?;
            require(
                chain.root == 1 && chain.index == index,
                "LDS exchange store output root/index",
            )?;
            let read = state
                .lds_read
                .ok_or(invalid("LDS exchange missing LDS read result"))?;
            require(
                get(2)?.id == read.load
                    && get(2)?.fact
                        == Fact::LdsLoaded {
                            load: read.load,
                            peer: read.peer,
                            frame: state.declaration,
                            publish: read.publish,
                        }
                    && state.publication == Some(read.publish)
                    && state
                        .read
                        .is_some_and(|(_, original_index)| original_index == index),
                "LDS exchange output requires exact ready peer LDS result",
            )?;
            require(
                get(3)?.fact == Fact::MaskedExec { mask, index },
                "LDS exchange store EXEC generation",
            )?;
            state.phase = Phase::StoreWait { mask };
        }
        O::RestoreExec => {
            let Phase::Restore { mask } = state.phase else {
                return Err(invalid("LDS exchange restore phase"));
            };
            let Fact::SavedExecLow { mask: saved, entry } = get(0)?.fact else {
                return Err(invalid("LDS exchange saved EXEC low"));
            };
            require(
                saved == mask && get(1)?.fact == Fact::SavedExecHigh { mask, entry },
                "LDS exchange exact saved EXEC pair",
            )?;
            facts[0] = Some(Fact::FullExec(entry));
            state.phase = Phase::End;
        }
        O::VectorLshlrev32 => {
            let source = get(0)?;
            facts[0] = Some(match (state.phase, source.fact) {
                (Phase::ReadyInput, Fact::LaneX) => Fact::LocalLdsOffset {
                    local: source.id,
                    shift: id(0)?,
                },
                (Phase::Published, Fact::PeerX { local, xor }) if source.id == xor => {
                    Fact::PeerLdsOffset {
                        local,
                        xor,
                        shift: id(0)?,
                    }
                }
                _ => return Err(invalid("LDS exchange exact local/peer shift source")),
            });
        }
        O::VectorXor32 => {
            require(
                state.phase == Phase::Published
                    && get(0)?.fact == Fact::LaneX
                    && get(0)?.id == state.get(Register::Vgpr(0))?.id,
                "LDS exchange xor64 actual localX",
            )?;
            facts[0] = Some(Fact::PeerX {
                local: get(0)?.id,
                xor: id(0)?,
            });
        }
        O::LdsWriteB32 => {
            state.full()?;
            require(
                state.phase == Phase::ReadyInput
                    && state.lds_write.is_none()
                    && state.publication.is_none(),
                "LDS exchange unique prepublication write",
            )?;
            let Fact::LocalLdsOffset { local, shift } = get(0)?.fact else {
                return Err(invalid("LDS exchange local write offset"));
            };
            let (load, index) = state
                .read
                .ok_or(invalid("LDS exchange missing global input load"))?;
            require(
                local == state.get(Register::Vgpr(0))?.id
                    && shift == get(0)?.id
                    && get(1)?.id == load
                    && get(1)?.fact == Fact::Loaded { load, index },
                "LDS exchange write requires exact local offset and ready input",
            )?;
            state.lds_write = Some(LdsWrite {
                site: step.site,
                local,
                address: shift,
                data: load,
            });
            state.phase = Phase::LdsWriteWait;
        }
        O::WorkgroupPublishBarrier => {
            state.full()?;
            let write = state
                .lds_write
                .ok_or(invalid("LDS exchange publication without write"))?;
            require(
                state.phase == Phase::PublishRequired
                    && state.ready()
                    && state.publication.is_none()
                    && step.site.occurrence == write.site.occurrence + 2,
                "LDS exchange unique waited full-participation publication",
            )?;
            state.publication = Some(step.site);
            state.phase = Phase::Published;
        }
        O::LdsReadB32 => {
            state.full()?;
            require(
                state.phase == Phase::Published && state.lds_read.is_none(),
                "LDS exchange unique published read",
            )?;
            let publish = state
                .publication
                .ok_or(invalid("LDS exchange read before publication"))?;
            let Fact::PeerLdsOffset { local, xor, shift } = get(0)?.fact else {
                return Err(invalid("LDS exchange exact peer read offset"));
            };
            require(
                local == state.get(Register::Vgpr(0))?.id && shift == get(0)?.id,
                "LDS exchange current peer shift provenance",
            )?;
            let load = id(0)?;
            facts[0] = Some(Fact::LdsLoaded {
                load,
                peer: xor,
                frame: state.declaration,
                publish,
            });
            readiness = Readiness::LdsPending(load);
            state.lds_read = Some(LdsRead {
                site: step.site,
                load,
                peer: xor,
                publish,
            });
            state.phase = Phase::LdsReadWait {
                load,
                destination: instruction.destination,
            };
        }
        O::Endpgm0 => return Err(invalid("LDS exchange end must be actual Return terminator")),
    }
    for (slot, register) in writes.into_iter().enumerate() {
        if let Some(register) = register {
            state.set(
                register,
                Cell {
                    id: id(slot)?,
                    fact: facts[slot].ok_or(invalid("LDS exchange missing result fact"))?,
                    readiness,
                },
            )?;
        }
    }
    Ok(())
}
fn address(low: Cell, high: Cell) -> Result<AddressChain> {
    let Fact::AddressLow(chain) = low.fact else {
        return Err(invalid("LDS exchange address low"));
    };
    require(
        low.id == chain.low_add && high.fact == Fact::AddressHigh(chain),
        "LDS exchange exact completed address pair",
    )?;
    Ok(chain)
}
