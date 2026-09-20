use super::*;

#[derive(Clone, Copy)]
enum Action {
    Retained,
    Remove,
    Copy(usize),
}
struct Scratch {
    selected: Vec<bool>,
    actions: Vec<Action>,
}

pub(super) fn check<'a>(
    input: &'a Owner,
    output: &'a Owner,
    selected: &'a [Coordinate],
    origins: &'a [Origin],
    limits: Limits,
    meter: &mut Meter<'_, '_>,
) -> Result<(CheckedCanonicalKirPrivateCellPromotionV1<'a>, Storage)> {
    type Checked<'a> = CheckedCanonicalKirPrivateCellPromotionV1<'a>;
    meter.work(3)?;
    meter.reserve(size_of::<Checked<'_>>() + size_of::<Scratch>())?;
    let (a, a_size) = meter.derive(|budget| Ok(Inventory::derive(input, budget)?))?;
    meter.reserve(a_size.retained_storage())?;
    let (b, b_size) = meter.derive(|budget| Ok(Inventory::derive(output, budget)?))?;
    meter.reserve(b_size.retained_storage())?;
    meter.work(2)?;
    if selected.len() > a.operations().len() || origins.len() != b.operations().len() {
        return Err(Error::Mismatch("bounded complete claim rosters"));
    }
    let bytes = input
        .canonical()
        .canonical_bytes()
        .len()
        .checked_add(output.canonical().canonical_bytes().len())
        .ok_or(Resource::Arithmetic)?;
    // One disjoint header/operation equality traversal of both encoded payloads.
    meter.work(bytes)?;
    headers(input.module(), output.module(), meter)?;
    let (census, census_size) = meter.derive(|budget| Ok(Census::derive(&a, limits, budget)?))?;
    meter.reserve(census_size.retained_storage())?;
    let count = a.operations().len();
    let (selected_bits, selected_bytes) = meter.table(count)?;
    let (actions, action_bytes) = meter.table(count)?;
    let mut scratch = Scratch {
        selected: selected_bits,
        actions,
    };
    for _ in 0..count {
        meter.push(&mut scratch.selected, false)?;
        meter.push(&mut scratch.actions, Action::Retained)?;
    }
    let mut last = None;
    for coordinate in selected {
        let ordinal = ordinal(&a, *coordinate, meter)?;
        meter.work(2)?;
        if last.is_some_and(|last| last >= ordinal) {
            return Err(Error::Mismatch("ordered unique selected allocations"));
        }
        scratch.selected[ordinal] = true;
        last = Some(ordinal);
    }
    let mut found = 0usize;
    for row in census.allocations() {
        let at = ordinal(&a, row.allocation, meter)?;
        meter.work(1)?;
        if scratch.selected[at] {
            set(&mut scratch.actions, at, Action::Remove, meter)?;
            found = found.checked_add(1).ok_or(Resource::Arithmetic)?;
        }
    }
    if found != selected.len() {
        return Err(Error::Mismatch("eligible selected allocation"));
    }
    for row in census.addresses() {
        let allocation = ordinal(&a, row.allocation, meter)?;
        meter.work(2)?;
        if !scratch.selected[allocation] || row.producer == row.allocation {
            continue;
        }
        let at = ordinal(&a, row.producer, meter)?;
        set(&mut scratch.actions, at, Action::Remove, meter)?;
    }
    for (index, row) in census.accesses().iter().enumerate() {
        let allocation = ordinal(&a, row.allocation, meter)?;
        meter.work(1)?;
        if !scratch.selected[allocation] {
            continue;
        }
        let at = ordinal(&a, row.operation, meter)?;
        let action = match row.kind {
            AccessKind::Store { .. } => Action::Remove,
            AccessKind::Load { .. } => Action::Copy(index),
        };
        set(&mut scratch.actions, at, action, meter)?;
    }
    let mut mapped = 0;
    for (old, new) in a.blocks().iter().zip(b.blocks()) {
        meter.work(2)?;
        if old.coordinate != new.coordinate {
            return Err(Error::Mismatch("block coordinate"));
        }
        let mut out = new.operations.start;
        for at in old.operations.clone() {
            meter.work(4)?;
            let original = &a.operations()[at];
            if matches!(scratch.actions[at], Action::Remove) {
                continue;
            }
            let final_op = b
                .operations()
                .get(out)
                .filter(|_| out < new.operations.end)
                .ok_or(Error::Mismatch("missing output operation"))?;
            let kind = match scratch.actions[at] {
                Action::Retained => {
                    if original.operation != final_op.operation {
                        return Err(Error::Mismatch("retained operation payload"));
                    }
                    OriginKind::Retained
                }
                Action::Copy(index) => {
                    let row = &census.accesses()[index];
                    let AccessKind::Load {
                        result,
                        previous_store,
                        stored_value,
                    } = row.kind
                    else {
                        return Err(Error::Mismatch("fresh census Load action"));
                    };
                    copy(
                        original.operation,
                        final_op.operation,
                        row.pointer,
                        result,
                        stored_value,
                        meter,
                    )?;
                    OriginKind::LoadCopy {
                        allocation: row.allocation,
                        previous_store,
                        stored_value,
                    }
                }
                Action::Remove => return Err(Error::Mismatch("removed action cursor")),
            };
            meter.work(3)?;
            if origins.get(mapped)
                != Some(&Origin {
                    input: original.coordinate,
                    output: final_op.coordinate,
                    kind,
                })
            {
                return Err(Error::Mismatch("exact complete output origin"));
            }
            mapped += 1;
            out += 1;
        }
        if out != new.operations.end {
            return Err(Error::Mismatch("extra output operation"));
        }
    }
    if mapped != origins.len() {
        return Err(Error::Mismatch("unused output origin"));
    }
    drop(scratch);
    meter.release(
        size_of::<Scratch>()
            .checked_add(selected_bytes)
            .and_then(|n| n.checked_add(action_bytes))
            .ok_or(Resource::Arithmetic)?,
    )?;
    drop(census);
    meter.release(census_size.retained_storage())?;
    drop(b);
    meter.release(b_size.retained_storage())?;
    drop(a);
    meter.release(a_size.retained_storage())?;
    Ok((
        Checked {
            input,
            output,
            selected,
            origins,
        },
        CanonicalKirPrivateCellPromotionStorageV1(size_of::<Checked<'_>>()),
    ))
}

fn set(actions: &mut [Action], at: usize, next: Action, meter: &mut Meter<'_, '_>) -> Result<()> {
    meter.work(2)?;
    if !matches!(actions[at], Action::Retained) {
        return Err(Error::Mismatch("conflicting input action"));
    }
    actions[at] = next;
    Ok(())
}

fn ordinal(
    inventory: &Inventory<'_>,
    coordinate: Coordinate,
    meter: &mut Meter<'_, '_>,
) -> Result<usize> {
    meter.work(7)?;
    let function = inventory
        .functions()
        .get(coordinate.block.function.0 as usize)
        .filter(|row| row.coordinate == coordinate.block.function)
        .ok_or(Error::Mismatch("actual operation coordinate"))?;
    let block = function
        .blocks
        .start
        .checked_add(coordinate.block.block as usize)
        .filter(|at| *at < function.blocks.end)
        .and_then(|at| inventory.blocks().get(at))
        .filter(|row| row.coordinate == coordinate.block)
        .ok_or(Error::Mismatch("actual operation coordinate"))?;
    let at = block
        .operations
        .start
        .checked_add(coordinate.operation as usize)
        .filter(|at| *at < block.operations.end)
        .ok_or(Error::Mismatch("actual operation coordinate"))?;
    if inventory.operations()[at].coordinate != coordinate {
        return Err(Error::Mismatch("actual operation coordinate"));
    }
    Ok(at)
}

pub(super) fn copy(
    input: &Operation,
    output: &Operation,
    pointer: ValueId,
    result: ValueId,
    value: ValueId,
    meter: &mut Meter<'_, '_>,
) -> Result<()> {
    meter.work(8)?;
    if !matches!(input.kind, Kind::Load { pointer: actual, .. } if actual == pointer)
        || input.results.len() != 1
        || input.results[0].id != result
        || input.results != output.results
        || !matches!(output.kind, Kind::Binary { op: BinaryOp::BitOr, lhs, rhs } if lhs == value && rhs == value)
    {
        return Err(Error::Mismatch("exact retained Load copy recipe"));
    }
    Ok(())
}

fn headers(input: &Module, output: &Module, meter: &mut Meter<'_, '_>) -> Result<()> {
    let Module {
        id,
        functions,
        kernels,
        required_capabilities,
    } = input;
    if id != &output.id
        || kernels != &output.kernels
        || required_capabilities != &output.required_capabilities
        || functions.len() != output.functions.len()
    {
        return Err(Error::Mismatch("module payload"));
    }
    for (a, b) in functions.iter().zip(&output.functions) {
        meter.work(1)?;
        let fe2o3_kernel_ir::Function {
            id,
            signature,
            role,
            body,
            required_capabilities,
        } = a;
        if id != &b.id
            || signature != &b.signature
            || role != &b.role
            || required_capabilities != &b.required_capabilities
        {
            return Err(Error::Mismatch("function payload"));
        }
        let (a, b) = match (body, &b.body) {
            (None, None) => continue,
            (Some(a), Some(b)) => (a, b),
            _ => return Err(Error::Mismatch("function body")),
        };
        let fe2o3_kernel_ir::FunctionBody { parameters, blocks } = a;
        if parameters != &b.parameters || blocks.len() != b.blocks.len() {
            return Err(Error::Mismatch("body payload"));
        }
        for (a, b) in blocks.iter().zip(&b.blocks) {
            meter.work(1)?;
            let fe2o3_kernel_ir::BasicBlock {
                id,
                parameters,
                operations: _,
                terminator,
            } = a;
            if id != &b.id || parameters != &b.parameters || terminator != &b.terminator {
                return Err(Error::Mismatch("block payload"));
            }
        }
    }
    Ok(())
}
