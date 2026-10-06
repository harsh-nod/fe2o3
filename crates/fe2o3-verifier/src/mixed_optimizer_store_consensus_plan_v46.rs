//! Concrete private-cell proof dependencies from the actual input MemorySSA.
//! The representative Store coordinate is only a key locator. Every incoming
//! edge is independently visited, and every required phi must reach a Store.
use super::*;
use fe2o3_kernel_analysis::{
    CanonicalKirMemorySsaInputSourceV1 as MemoryInput, CanonicalKirMemorySsaNodeIdV1 as NodeId,
    CanonicalKirMemorySsaNodeV1 as Node, CanonicalKirMemorySsaStorageV1,
    CanonicalKirMemorySsaV18 as Memory, CheckedCanonicalKirCrossBlockForwardingV18 as Pair,
};
use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, KirLocalMemoryEffectRefV1 as Effect, MemoryAccess,
    StorageLayoutKindV1 as LayoutKind, StorageOperationV1 as Storage,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Action {
    Other,
    Allocate { slot: usize },
    Load { slot: usize },
    Store { slot: usize, value: usize },
}

#[derive(Clone, Copy)]
pub(super) struct Slot {
    pub pointer: usize,
    pub allocation: usize,
    pub bits: u16,
}

pub(super) struct MemoryPlan {
    pub slots: Vec<Slot>,
    pub actions: Vec<Action>,
    /// Input operation ordinal of the exact required Store class, or NONE.
    pub entry: Vec<usize>,
    pub exit: Vec<usize>,
    /// Per-operation actual replacement value definition, or NONE.
    pub replacements: Vec<usize>,
    /// Required Store class immediately before each actual operation.
    pub before: Vec<usize>,
}

fn filled<T: Clone>(count: usize, value: T, out: &mut Writer<'_, '_>) -> Result<Vec<T>> {
    let bytes = count
        .checked_mul(size_of::<T>())
        .ok_or(Resource::Arithmetic)?;
    out.budget.reserve_storage(bytes)?;
    out.budget.charge_work(count)?;
    let mut rows = Vec::new();
    rows.try_reserve_exact(count)
        .map_err(|_| Resource::Allocation)?;
    out.budget.reserve_storage(
        rows.capacity()
            .checked_sub(count)
            .ok_or(Resource::Accounting)?
            .checked_mul(size_of::<T>())
            .ok_or(Resource::Arithmetic)?,
    )?;
    rows.resize(count, value);
    Ok(rows)
}

fn bytes<T>(rows: &[T], capacity: usize) -> Result<usize> {
    if rows.len() > capacity {
        return Err(Resource::Accounting.into());
    }
    capacity
        .checked_mul(size_of::<T>())
        .ok_or(Resource::Arithmetic.into())
}

fn integer_bits(ty: &Type) -> Result<u16> {
    let Type::Scalar(
        scalar @ (ScalarType::U8
        | ScalarType::U16
        | ScalarType::U32
        | ScalarType::U64
        | ScalarType::I8
        | ScalarType::I16
        | ScalarType::I32
        | ScalarType::I64),
    ) = ty
    else {
        return Err(Error::Statement("consensus concrete fixed integer cell"));
    };
    Ok(scalar
        .bit_width()
        .ok_or(Error::Statement("consensus fixed cell width"))?)
}

fn cell_scalar(inv: &Inventory<'_>, ty: &Type, out: &mut Writer<'_, '_>) -> Result<ScalarType> {
    out.budget.charge_work(5)?;
    match ty {
        Type::Scalar(scalar) => {
            integer_bits(ty)?;
            Ok(*scalar)
        }
        Type::StorageObject(id) => {
            let layout = inv
                .owner()
                .module()
                .storage_layouts
                .get(id.0 as usize)
                .ok_or(Error::Statement("consensus owned scalar storage layout"))?;
            let LayoutKind::Scalar(scalar) = layout.kind else {
                return Err(Error::Statement("consensus whole scalar storage only"));
            };
            if layout.size != u64::from(integer_bits(&Type::Scalar(scalar))? / 8) {
                return Err(Error::Statement("consensus exact scalar storage extent"));
            }
            Ok(scalar)
        }
        _ => Err(Error::Statement("consensus scalar cell representation")),
    }
}

fn access(
    inv: &Inventory<'_>,
    ordinal: usize,
    out: &mut Writer<'_, '_>,
) -> Result<(usize, MemoryAccess, Option<usize>)> {
    out.budget.charge_work(10)?;
    let row = inv
        .operations()
        .get(ordinal)
        .ok_or(Error::Statement("consensus actual access coordinate"))?;
    let (pointer, memory, stored) = match row.operation.kind {
        OperationKind::Load { pointer, access }
        | OperationKind::Storage(Storage::ReadValue {
            address: pointer,
            access,
        }) if row.operands.len() == 1 && row.results.len() == 1 => (pointer, access, None),
        OperationKind::Store {
            pointer,
            value,
            access,
        }
        | OperationKind::Storage(Storage::WriteValue {
            address: pointer,
            value,
            access,
        }) if row.operands.len() == 2 && row.results.is_empty() => (pointer, access, Some(value)),
        _ => return Err(Error::Statement("consensus actual Load or Store")),
    };
    if memory.volatile
        || memory.address_space != AddressSpace::Private
        || !memory.alignment.is_power_of_two()
        || !row.compiler_ordering().is_empty()
        || row.effects.len() != 1
    {
        return Err(Error::Statement("consensus ordinary private access"));
    }
    let uses = &inv.uses()[row.operands.clone()];
    if uses[0].value != pointer {
        return Err(Error::Statement("consensus exact pointer occurrence"));
    }
    let definition = uses[0].definition;
    let Type::Pointer(pointer_type) = inv.definitions()[definition].ty else {
        return Err(Error::Statement("consensus private pointer type"));
    };
    if pointer_type.address_space != AddressSpace::Private
        || pointer_type.access != AccessMode::ReadWrite
    {
        return Err(Error::Statement("consensus private pointer access rights"));
    }
    let value_type = Type::Scalar(cell_scalar(inv, &pointer_type.pointee, out)?);
    let value = if let Some(value) = stored {
        if uses[1].value != value
            || inv.definitions()[uses[1].definition].ty != &value_type
            || !matches!(
                inv.effects()[row.effects.start].effect,
                Effect::Write(AddressSpace::Private)
            )
        {
            return Err(Error::Statement("consensus exact Store value and effect"));
        }
        Some(uses[1].definition)
    } else {
        if inv.definitions()[row.results.start].ty != &value_type
            || !matches!(
                inv.effects()[row.effects.start].effect,
                Effect::Read(AddressSpace::Private)
            )
        {
            return Err(Error::Statement("consensus exact Load type and effect"));
        }
        None
    };
    Ok((definition, memory, value))
}

fn allocation(inv: &Inventory<'_>, pointer: usize, out: &mut Writer<'_, '_>) -> Result<Slot> {
    out.budget.charge_work(8)?;
    let Definition::Result {
        operation,
        result: 0,
    } = inv.definitions()[pointer].coordinate
    else {
        return Err(Error::Statement(
            "consensus direct Alloca pointer definition",
        ));
    };
    let at = operation_index(inv, operation)?;
    let row = &inv.operations()[at];
    let OperationKind::Alloca {
        element,
        count: None,
        address_space: AddressSpace::Private,
        alignment,
    } = &row.operation.kind
    else {
        return Err(Error::Statement(
            "consensus exact scalar private allocation",
        ));
    };
    let bits = integer_bits(&Type::Scalar(cell_scalar(inv, element, out)?))?;
    if row.results.len() != 1
        || row.results.start != pointer
        || !row.operands.is_empty()
        || !alignment.is_power_of_two()
        || *alignment < u32::from(bits / 8)
        || row.effects.len() != 1
        || !row.compiler_ordering().is_empty()
        || !matches!(
            inv.effects()[row.effects.start].effect,
            Effect::Allocate(AddressSpace::Private)
        )
    {
        return Err(Error::Statement("consensus exact allocation shape"));
    }
    Ok(Slot {
        pointer,
        allocation: at,
        bits,
    })
}

fn same_store(
    inv: &Inventory<'_>,
    left: usize,
    right: usize,
    out: &mut Writer<'_, '_>,
) -> Result<bool> {
    let a = access(inv, left, out)?;
    let b = access(inv, right, out)?;
    Ok(a.2.is_some() && a == b)
}

fn require(
    inv: &Inventory<'_>,
    node: NodeId,
    store: usize,
    labels: &mut [usize],
    queue: &mut [Option<NodeId>],
    tail: &mut usize,
    out: &mut Writer<'_, '_>,
) -> Result<()> {
    out.budget.charge_work(3)?;
    let label = labels
        .get_mut(node.index())
        .ok_or(Error::Statement("consensus bounded memory node"))?;
    if *label == NONE {
        *label = store;
        *queue
            .get_mut(*tail)
            .ok_or(Error::Statement("consensus bounded closure queue"))? = Some(node);
        *tail = tail.checked_add(1).ok_or(Resource::Arithmetic)?;
    } else if !same_store(inv, *label, store, out)? {
        return Err(Error::Statement(
            "consensus incompatible memory requirements",
        ));
    }
    Ok(())
}

impl MemoryPlan {
    pub(super) fn build(
        input: &Inventory<'_>,
        output: &Inventory<'_>,
        pair: &Pair<'_>,
        out: &mut Writer<'_, '_>,
    ) -> Result<Self> {
        if !input.belongs_to(pair.input())
            || !output.belongs_to(pair.output())
            || pair.origins().len() != input.operations().len()
            || input.operations().len() != output.operations().len()
            || input.definitions().len() != output.definitions().len()
            || input.blocks().len() != output.blocks().len()
        {
            return Err(Error::Statement("consensus exact actual pair census"));
        }
        let header = size_of::<Self>() + size_of::<Result<Self>>();
        out.budget.reserve_storage(header)?;
        let scratch_header = size_of::<(
            Memory<'_, '_>,
            CanonicalKirMemorySsaStorageV1,
            Vec<usize>,
            Vec<Option<NodeId>>,
            Vec<usize>,
            Vec<usize>,
            Vec<usize>,
            Vec<usize>,
            Vec<usize>,
            Vec<usize>,
        )>();
        out.budget.reserve_storage(scratch_header)?;
        let mut pointers = allocate(input.definitions().len(), out)?;
        let mut count = 0;
        for (ordinal, origin) in pair.origins().iter().enumerate() {
            out.budget.charge_work(4)?;
            if origin.input != input.operations()[ordinal].coordinate
                || origin.output != output.operations()[ordinal].coordinate
            {
                return Err(Error::Statement(
                    "consensus unchanged exact operation coordinates",
                ));
            }
            if origin.store.is_none() {
                continue;
            }
            let (pointer, _, stored) = access(input, ordinal, out)?;
            if stored.is_some() {
                return Err(Error::Statement("consensus selected operation is a Load"));
            }
            if pointers[pointer] == NONE {
                allocation(input, pointer, out)?;
                pointers[pointer] = count;
                count = count.checked_add(1).ok_or(Resource::Arithmetic)?;
            }
        }
        let mut plan = Self {
            slots: filled(
                count,
                Slot {
                    pointer: NONE,
                    allocation: NONE,
                    bits: 0,
                },
                out,
            )?,
            actions: filled(input.operations().len(), Action::Other, out)?,
            entry: allocate(input.blocks().len(), out)?,
            exit: allocate(input.blocks().len(), out)?,
            replacements: allocate(input.operations().len(), out)?,
            before: allocate(input.operations().len(), out)?,
        };
        for (pointer, &slot) in pointers.iter().enumerate() {
            out.budget.charge_work(1)?;
            if slot == NONE {
                continue;
            }
            let row = allocation(input, pointer, out)?;
            plan.actions[row.allocation] = Action::Allocate { slot };
            plan.slots[slot] = row;
        }
        for (ordinal, row) in input.operations().iter().enumerate() {
            for usage in &input.uses()[row.operands.clone()] {
                out.budget.charge_work(3)?;
                let slot = pointers[usage.definition];
                if slot == NONE {
                    continue;
                }
                let (pointer, memory, stored) = access(input, ordinal, out)?;
                if pointer != usage.definition {
                    return Err(Error::Statement(
                        "consensus private pointer cannot escape through a value",
                    ));
                }
                let OperationKind::Alloca { alignment, .. } = input.operations()
                    [plan.slots[slot].allocation]
                    .operation
                    .kind
                else {
                    unreachable!()
                };
                if memory.alignment > alignment {
                    return Err(Error::Statement(
                        "consensus access alignment exceeds allocation",
                    ));
                }
                plan.actions[ordinal] = if let Some(value) = stored {
                    Action::Store { slot, value }
                } else {
                    Action::Load { slot }
                };
            }
        }
        for block in input.blocks() {
            for usage in &input.uses()[block.terminator_uses.clone()] {
                out.budget.charge_work(1)?;
                if pointers[usage.definition] != NONE {
                    return Err(Error::Statement(
                        "consensus private pointer escapes through control",
                    ));
                }
            }
        }
        let (memory, memory_storage) = Memory::derive_v18(input, Default::default(), out.budget)?;
        out.budget
            .reserve_storage(memory_storage.retained_storage())?;
        let mut labels = allocate(memory.node_count(), out)?;
        let mut queue = filled(memory.node_count(), None::<NodeId>, out)?;
        let mut tail = 0;
        for (ordinal, origin) in pair.origins().iter().enumerate() {
            let Some(store) = origin.store else {
                continue;
            };
            let store = operation_index(input, store)?;
            let (pointer, access, _) = access(input, ordinal, out)?;
            let (stored_pointer, stored_access, value) = self::access(input, store, out)?;
            let value = value.ok_or(Error::Statement(
                "consensus representative is an actual Store",
            ))?;
            // Each actual access is separately bounded by the allocation above;
            // an alignment annotation is not part of the stored value identity.
            if pointer != stored_pointer
                || access.address_space != stored_access.address_space
                || access.volatile != stored_access.volatile
            {
                return Err(Error::Statement("consensus exact selected access class"));
            }
            let actual = &output.operations()[ordinal];
            if !matches!(actual.operation.kind, OperationKind::Binary { op: BinaryOp::BitOr, lhs, rhs }
                if Some(lhs) == input.definitions()[value].value && lhs == rhs)
                || actual.operands.len() != 2
                || actual.results.len() != 1
                || output.uses()[actual.operands.start].definition != value
                || output.uses()[actual.operands.start + 1].definition != value
            {
                return Err(Error::Statement(
                    "consensus exact actual replacement operands",
                ));
            }
            plan.replacements[ordinal] = value;
            let id = memory
                .operation(origin.input, out.budget)?
                .ok_or(Error::Statement("consensus selected memory Use"))?;
            let Node::Use {
                operation,
                incoming,
            } = memory.node(id, out.budget)?
            else {
                return Err(Error::Statement("consensus actual Load memory Use"));
            };
            if *operation != origin.input {
                return Err(Error::Statement("consensus same-owner Load occurrence"));
            }
            require(
                input,
                *incoming,
                store,
                &mut labels,
                &mut queue,
                &mut tail,
                out,
            )?;
        }
        let mut head = 0;
        let mut incidence = 0usize;
        while head < tail {
            out.budget.charge_work(2)?;
            let node =
                queue[head].ok_or(Error::Statement("consensus complete closure worklist"))?;
            head += 1;
            let store = labels[node.index()];
            match memory.node(node, out.budget)? {
                Node::Def { operation, .. } => {
                    if !same_store(input, operation_index(input, *operation)?, store, out)? {
                        return Err(Error::Statement("consensus clobbered incoming memory path"));
                    }
                }
                Node::Phi { .. } => {
                    let inputs = memory.phi_inputs(node, out.budget)?;
                    if inputs.is_empty() {
                        return Err(Error::Statement("consensus ungrounded empty memory phi"));
                    }
                    incidence = incidence
                        .checked_add(inputs.len())
                        .ok_or(Resource::Arithmetic)?;
                    for input in inputs {
                        out.budget.charge_work(2)?;
                        if matches!(input.source(), MemoryInput::Entry(_)) {
                            return Err(Error::Statement("consensus uninitialized entry path"));
                        }
                        require(
                            memory.inventory(),
                            input.state(),
                            store,
                            &mut labels,
                            &mut queue,
                            &mut tail,
                            out,
                        )?;
                    }
                }
                Node::LiveOnEntry { .. } | Node::Use { .. } => {
                    return Err(Error::Statement("consensus non-store memory terminal"));
                }
            }
        }
        // Reverse only the exact edges already visited. A cycle without any
        // actual matching Store cannot bootstrap its own initialization.
        let mut parents = allocate(memory.node_count(), out)?;
        let mut next = allocate(incidence, out)?;
        let mut target = allocate(incidence, out)?;
        let mut grounded = allocate(memory.node_count(), out)?;
        let mut pending = allocate(memory.node_count(), out)?;
        let mut edges = 0;
        let mut pending_end = 0;
        for id in queue[..tail].iter().flatten().copied() {
            out.budget.charge_work(2)?;
            match memory.node(id, out.budget)? {
                Node::Def { .. } => {
                    grounded[id.index()] = 1;
                    pending[pending_end] = id.index();
                    pending_end += 1;
                }
                Node::Phi { .. } => {
                    for input in memory.phi_inputs(id, out.budget)? {
                        out.budget.charge_work(3)?;
                        next[edges] = parents[input.state().index()];
                        target[edges] = id.index();
                        parents[input.state().index()] = edges;
                        edges += 1;
                    }
                }
                _ => return Err(Error::Statement("consensus unexpected closure node")),
            }
        }
        let mut pending_head = 0;
        while pending_head < pending_end {
            out.budget.charge_work(2)?;
            let mut edge = parents[pending[pending_head]];
            pending_head += 1;
            while edge != NONE {
                out.budget.charge_work(3)?;
                let parent = target[edge];
                if grounded[parent] == NONE {
                    grounded[parent] = 1;
                    pending[pending_end] = parent;
                    pending_end += 1;
                }
                edge = next[edge];
            }
        }
        for id in queue[..tail].iter().flatten() {
            out.budget.charge_work(1)?;
            if grounded[id.index()] == NONE {
                return Err(Error::Statement("consensus ungrounded memory cycle"));
            }
        }
        for (block, row) in input.blocks().iter().enumerate() {
            let mut node = memory.block_entry(row.coordinate, out.budget)?;
            plan.entry[block] = labels[node.index()];
            if plan.entry[block] != NONE {
                let value = access(input, plan.entry[block], out)?
                    .2
                    .ok_or(Error::Statement("consensus entry Store value"))?;
                out.budget.charge_work(1)?;
                if matches!(input.definitions()[value].coordinate,
                    Definition::BlockArgument { block: defined, .. } if defined == row.coordinate)
                {
                    return Err(Error::Statement(
                        "consensus entry value is rebound by block transport",
                    ));
                }
            }
            for ordinal in row.operations.clone() {
                out.budget.charge_work(2)?;
                plan.before[ordinal] = labels[node.index()];
                if plan.replacements[ordinal] != NONE && plan.before[ordinal] == NONE {
                    return Err(Error::Statement(
                        "consensus selected Load has no required cut",
                    ));
                }
                if plan.before[ordinal] != NONE {
                    let value = access(input, plan.before[ordinal], out)?
                        .2
                        .ok_or(Error::Statement("consensus operation Store value"))?;
                    out.budget.charge_work(1)?;
                    if input.operations()[ordinal].results.contains(&value) {
                        return Err(Error::Statement(
                            "consensus required Store value is redefined",
                        ));
                    }
                }
                if let Some(observed) =
                    memory.operation(input.operations()[ordinal].coordinate, out.budget)?
                {
                    match memory.node(observed, out.budget)? {
                        Node::Def { operation, .. }
                            if *operation == input.operations()[ordinal].coordinate =>
                        {
                            node = observed;
                        }
                        Node::Use {
                            operation,
                            incoming,
                        } if *operation == input.operations()[ordinal].coordinate
                            && *incoming == node => {}
                        _ => return Err(Error::Statement("consensus complete ordered memory cut")),
                    }
                }
            }
            plan.exit[block] = labels[node.index()];
        }
        let release = [
            scratch_header,
            memory_storage.retained_storage(),
            bytes(&pointers, pointers.capacity())?,
            bytes(&labels, labels.capacity())?,
            bytes(&queue, queue.capacity())?,
            bytes(&parents, parents.capacity())?,
            bytes(&next, next.capacity())?,
            bytes(&target, target.capacity())?,
            bytes(&grounded, grounded.capacity())?,
            bytes(&pending, pending.capacity())?,
        ]
        .into_iter()
        .try_fold(0usize, |sum, value| {
            sum.checked_add(value).ok_or(Resource::Arithmetic)
        })?;
        drop((
            memory, pointers, labels, queue, parents, next, target, grounded, pending,
        ));
        out.budget.release_storage(release)?;
        Ok(plan)
    }
}
