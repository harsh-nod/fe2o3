//! One private-slot eligibility and reaching-store algorithm for both owners.
use super::*;

#[derive(Clone, Copy)]
pub(in crate::formal_memory_obligations) struct Slot {
    pub value: ValueId,
    pub location: FunctionOperationLocation,
    pub escape: Option<(FunctionOperationLocation, ValueId)>,
}
#[derive(Clone, Copy)]
pub(in crate::formal_memory_obligations) struct LoadSource {
    pub value: ValueId,
    pub source: ValueId,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Cell {
    Uninitialized,
    Exact(ValueId),
    Unknown,
}
impl Cell {
    fn join(self, other: Self) -> Self {
        if self == other { self } else { Self::Unknown }
    }
}
#[derive(Clone, Copy)]
struct BlockRow<'source> {
    block: &'source crate::BasicBlock,
    original: usize,
}
#[derive(Clone, Copy)]
struct Edge {
    from: usize,
    to: usize,
}

pub(in crate::formal_memory_obligations) trait Environment<'source> {
    type Error;
    fn source(&self) -> &'source Function;
    fn step(&mut self, work: usize) -> Result<(), Self::Error>;
    fn arithmetic(&self) -> Self::Error;
    fn reachable(&mut self, block: BlockId) -> Result<bool, Self::Error>;
    fn block(&mut self, block: BlockId) -> Result<Option<&'source crate::BasicBlock>, Self::Error>;
    fn origin(&mut self, value: ValueId) -> Result<Option<ValueId>, Self::Error>;
    fn ty(&mut self, value: ValueId) -> Result<Option<&'source Type>, Self::Error>;
    fn equal(&mut self, left: &Type, right: &Type) -> Result<bool, Self::Error>;
    fn empty<T: Copy>(&mut self) -> Result<Vec<T>, Self::Error>;
    fn filled<T: Copy>(&mut self, count: usize, value: T) -> Result<Vec<T>, Self::Error>;
    fn push<T: Copy>(&mut self, rows: &mut Vec<T>, value: T) -> Result<(), Self::Error>;
    fn sort<T: Copy>(
        &mut self,
        rows: &mut [T],
        key: impl Fn(&T) -> u32 + Copy,
    ) -> Result<(), Self::Error>;
    fn find<T>(
        &mut self,
        rows: &[T],
        compare: impl Fn(&T) -> std::cmp::Ordering,
    ) -> Result<Option<usize>, Self::Error>;
}

fn exact_slot<'s, E: Environment<'s>>(
    env: &mut E,
    slots: &[Slot],
    value: ValueId,
) -> Result<Option<usize>, E::Error> {
    let Some(origin) = env.origin(value)? else {
        return Ok(None);
    };
    env.find(slots, |slot| slot.value.cmp(&origin))
}
fn private_access<'s, E: Environment<'s>>(
    env: &mut E,
    pointer: ValueId,
    access: MemoryAccess,
) -> Result<bool, E::Error> {
    Ok(matches!(env.ty(pointer)?, Some(Type::Pointer(ty))
        if ty.address_space == access.address_space
        && matches!(ty.address_space, AddressSpace::Private | AddressSpace::Generic)))
}
fn record(slots: &mut [Slot], slot: usize, location: FunctionOperationLocation, value: ValueId) {
    slots[slot].escape.get_or_insert((location, value));
}
fn terminal_escape<'s, E: Environment<'s>>(
    env: &mut E,
    slots: &mut [Slot],
    value: ValueId,
) -> Result<(), E::Error> {
    env.step(1)?;
    if let Some(slot) = exact_slot(env, slots, value)? {
        let location = slots[slot].location;
        record(slots, slot, location, value);
    }
    Ok(())
}
fn edge_escapes<'s, E: Environment<'s>>(
    env: &mut E,
    slots: &mut [Slot],
    target: BlockId,
    arguments: &[ValueId],
) -> Result<(), E::Error> {
    env.step(1)?;
    let parameters = env.block(target)?.map(|block| block.parameters.as_slice());
    let mut exact = parameters.is_some_and(|parameters| arguments.len() == parameters.len());
    if exact {
        for (argument, parameter) in arguments.iter().zip(parameters.unwrap()) {
            env.step(1)?;
            let Some(ty) = env.ty(*argument)? else {
                exact = false;
                break;
            };
            if !env.equal(ty, &parameter.ty)? {
                exact = false;
                break;
            }
        }
    }
    for (index, argument) in arguments.iter().copied().enumerate() {
        env.step(1)?;
        let Some(slot) = exact_slot(env, slots, argument)? else {
            continue;
        };
        let transport = if exact {
            if let Some(parameter) = parameters.and_then(|rows| rows.get(index)) {
                exact_slot(env, slots, parameter.id)? == Some(slot)
            } else {
                false
            }
        } else {
            false
        };
        if !transport {
            let location = slots[slot].location;
            record(slots, slot, location, argument);
        }
    }
    Ok(())
}

pub(in crate::formal_memory_obligations) fn classify<'s, E: Environment<'s>>(
    env: &mut E,
) -> Result<Vec<Slot>, E::Error> {
    let source = env.source();
    let body = source.body.as_ref().expect("verified entry is defined");
    let mut slots = env.empty()?;
    for block in &body.blocks {
        env.step(1)?;
        if !env.reachable(block.id)? {
            continue;
        }
        for (ordinal, operation) in block.operations.iter().enumerate() {
            env.step(1)?;
            if matches!(
                operation.kind,
                OperationKind::Alloca {
                    count: None,
                    address_space: AddressSpace::Private,
                    ..
                }
            ) {
                for result in &operation.results {
                    env.step(1)?;
                    env.push(
                        &mut slots,
                        Slot {
                            value: result.id,
                            location: FunctionOperationLocation::new(block.id, ordinal),
                            escape: None,
                        },
                    )?;
                }
            }
        }
    }
    env.sort(&mut slots, |slot| slot.value.0)?;
    for block in &body.blocks {
        env.step(1)?;
        if !env.reachable(block.id)? {
            continue;
        }
        for (ordinal, operation) in block.operations.iter().enumerate() {
            env.step(1)?;
            let location = FunctionOperationLocation::new(block.id, ordinal);
            operation
                .kind
                .try_visit_operands(|operand| -> Result<(), E::Error> {
                    env.step(1)?;
                    let Some(slot) = exact_slot(env, &slots, operand)? else {
                        return Ok(());
                    };
                    let exact = match &operation.kind {
                        OperationKind::Load { pointer, access } => {
                            *pointer == operand
                                && private_access(env, *pointer, *access)?
                                && exact_slot(env, &slots, *pointer)? == Some(slot)
                        }
                        OperationKind::Store {
                            pointer,
                            value,
                            access,
                        } => {
                            *pointer == operand
                                && private_access(env, *pointer, *access)?
                                && exact_slot(env, &slots, *pointer)? == Some(slot)
                                && exact_slot(env, &slots, *value)? != Some(slot)
                        }
                        OperationKind::Cast {
                            kind: CastKind::RestrictPointerAccess | CastKind::PointerToGeneric,
                            value,
                            ..
                        } => {
                            *value == operand
                                && if let [result] = operation.results.as_slice() {
                                    exact_slot(env, &slots, result.id)? == Some(slot)
                                } else {
                                    false
                                }
                        }
                        _ => false,
                    };
                    if !exact {
                        record(&mut slots, slot, location, operand);
                    }
                    Ok(())
                })?;
        }
        env.step(1)?;
        let Some(terminator) = &block.terminator else {
            continue;
        };
        match terminator {
            Terminator::ConditionalBranch { condition, .. } => {
                terminal_escape(env, &mut slots, *condition)?
            }
            Terminator::Switch { selector, .. } | Terminator::IntegerSwitch { selector, .. } => {
                terminal_escape(env, &mut slots, *selector)?
            }
            Terminator::Return { values } => {
                for value in values {
                    terminal_escape(env, &mut slots, *value)?;
                }
            }
            Terminator::Branch { .. } | Terminator::Unreachable => {}
        }
        terminator.try_visit_edges_v1(|target, arguments| {
            edge_escapes(env, &mut slots, target, arguments)
        })?;
    }
    Ok(slots)
}

fn eligible_slot<'s, E: Environment<'s>>(
    env: &mut E,
    slots: &[Slot],
    pointer: ValueId,
    access: MemoryAccess,
) -> Result<Option<usize>, E::Error> {
    if !private_access(env, pointer, access)? {
        return Ok(None);
    }
    Ok(exact_slot(env, slots, pointer)?.filter(|index| slots[*index].escape.is_none()))
}
fn transfer<'s, E: Environment<'s>>(
    env: &mut E,
    slots: &[Slot],
    operation: &Operation,
    state: &mut [Cell],
) -> Result<(), E::Error> {
    env.step(1)?;
    if let OperationKind::Store {
        pointer,
        value,
        access,
    } = operation.kind
        && let Some(slot) = eligible_slot(env, slots, pointer, access)?
    {
        state[slot] = env.origin(value)?.map_or(Cell::Unknown, Cell::Exact);
    }
    Ok(())
}

pub(in crate::formal_memory_obligations) fn loads<'s, E: Environment<'s>>(
    env: &mut E,
    candidates: &[Slot],
) -> Result<Vec<LoadSource>, E::Error> {
    let body = env
        .source()
        .body
        .as_ref()
        .expect("verified entry is defined");
    let mut loads = env.empty()?;
    let mut eligible = env.empty()?;
    for slot in candidates {
        env.step(1)?;
        if slot.escape.is_none() {
            env.push(&mut eligible, *slot)?;
        }
    }
    let slots = eligible.as_slice();
    if slots.is_empty() || body.blocks.is_empty() {
        return Ok(loads);
    }
    let mut blocks = env.empty()?;
    for block in &body.blocks {
        env.step(1)?;
        if env.reachable(block.id)? {
            let original = blocks.len();
            env.push(&mut blocks, BlockRow { block, original })?;
        }
    }
    let count = blocks.len();
    let mut index = env.empty()?;
    for row in &blocks {
        env.step(1)?;
        env.push(&mut index, *row)?;
    }
    env.sort(&mut index, |row| row.block.id.0)?;
    let mut edges = env.empty()?;
    for (from, row) in blocks.iter().enumerate() {
        env.step(1)?;
        if let Some(terminator) = &row.block.terminator {
            terminator.try_visit_edges_v1(|target, _| -> Result<(), E::Error> {
                env.step(1)?;
                if let Some(to) = env.find(&index, |row| row.block.id.cmp(&target))? {
                    env.push(
                        &mut edges,
                        Edge {
                            from,
                            to: index[to].original,
                        },
                    )?;
                }
                Ok(())
            })?;
        }
    }
    // Source BlockId ordering is preserved for predecessor joins. Sorting by
    // destination is stable; duplicate edges have the same idempotent join.
    env.sort(&mut edges, |edge| blocks[edge.from].block.id.0)?;
    env.sort(&mut edges, |edge| blocks[edge.to].block.id.0)?;
    let mut offsets = env.filled(count, (0usize, 0usize))?;
    for (ordinal, edge) in edges.iter().enumerate() {
        env.step(1)?;
        if offsets[edge.to].1 == 0 {
            offsets[edge.to].0 = ordinal;
        }
        offsets[edge.to].1 = ordinal.checked_add(1).ok_or_else(|| env.arithmetic())?;
    }
    let cells = count
        .checked_mul(slots.len())
        .ok_or_else(|| env.arithmetic())?;
    let mut incoming = env.filled(cells, Cell::Uninitialized)?;
    let mut outgoing = env.filled(cells, Cell::Uninitialized)?;
    let mut in_present = env.filled(count, false)?;
    let mut out_present = env.filled(count, false)?;
    let mut state = env.filled(slots.len(), Cell::Uninitialized)?;
    let width = slots.len();
    loop {
        env.step(1)?;
        let mut changed = false;
        for (index, row) in blocks.iter().enumerate() {
            env.step(1)?;
            let start = index.checked_mul(width).ok_or_else(|| env.arithmetic())?;
            let end = start.checked_add(width).ok_or_else(|| env.arithmetic())?;
            let mut present = row.block.id == body.blocks[0].id;
            if present {
                env.step(width)?;
                state.fill(Cell::Uninitialized);
            } else {
                let (first, last) = offsets[index];
                for edge in &edges[first..last] {
                    env.step(1)?;
                    if !out_present[edge.from] {
                        continue;
                    }
                    let pred = edge
                        .from
                        .checked_mul(width)
                        .ok_or_else(|| env.arithmetic())?;
                    env.step(width)?;
                    for (slot, value) in state.iter_mut().enumerate() {
                        *value = if present {
                            value.join(outgoing[pred + slot])
                        } else {
                            outgoing[pred + slot]
                        };
                    }
                    present = true;
                }
            }
            if !present {
                continue;
            }
            env.step(width)?;
            changed |= !in_present[index] || incoming[start..end] != state[..];
            env.step(width)?;
            incoming[start..end].copy_from_slice(&state);
            in_present[index] = true;
            for operation in &row.block.operations {
                transfer(env, slots, operation, &mut state)?;
            }
            env.step(width)?;
            changed |= !out_present[index] || outgoing[start..end] != state[..];
            env.step(width)?;
            outgoing[start..end].copy_from_slice(&state);
            out_present[index] = true;
        }
        if !changed {
            break;
        }
    }
    for (index, row) in blocks.iter().enumerate() {
        env.step(1)?;
        if !in_present[index] {
            continue;
        }
        let start = index.checked_mul(width).ok_or_else(|| env.arithmetic())?;
        let end = start.checked_add(width).ok_or_else(|| env.arithmetic())?;
        env.step(width)?;
        state.copy_from_slice(&incoming[start..end]);
        for operation in &row.block.operations {
            env.step(1)?;
            if let OperationKind::Load { pointer, access } = operation.kind
                && let Some(slot) = eligible_slot(env, slots, pointer, access)?
                && let Cell::Exact(source) = state[slot]
            {
                for result in &operation.results {
                    env.step(1)?;
                    env.push(
                        &mut loads,
                        LoadSource {
                            value: result.id,
                            source,
                        },
                    )?;
                }
            }
            transfer(env, slots, operation, &mut state)?;
        }
    }
    env.sort(&mut loads, |row| row.value.0)?;
    Ok(loads)
}
