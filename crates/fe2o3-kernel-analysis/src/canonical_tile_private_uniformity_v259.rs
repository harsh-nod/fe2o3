//! Exact single-writer scalar spills for lane uniformity, not memory authority.
use super::*;
use fe2o3_kernel_ir::{
    AccessMode, CastKind, MemoryAccess, StorageLayoutKindV1, StorageOperationV1 as Storage, Type,
};

#[derive(Clone, Copy)]
pub(super) struct Read {
    pub(super) value: usize,
    pub(super) writer_block: usize,
}

fn operand(graph: &Graph<'_, '_>, operation: usize, ordinal: usize) -> Result<usize> {
    let row = &graph.inventory.operations()[operation];
    let index = row
        .operands
        .start
        .checked_add(ordinal)
        .ok_or(Resource::Arithmetic)?;
    if !row.operands.contains(&index) {
        return Err(Error::Inventory);
    }
    let definition = graph.inventory.uses()[index].definition;
    graph.definition(definition)?;
    Ok(definition)
}

fn access_valid(access: MemoryAccess, alignment: u32) -> bool {
    access.address_space == AddressSpace::Private
        && !access.volatile
        && access.alignment.is_power_of_two()
        && access.alignment <= alignment
}

// All pointers in this set denote the same complete allocation. No projection,
// pointer-bit equality, multi-incoming phi, or memory-loaded pointer is admitted.
fn restriction(graph: &Graph<'_, '_>, index: usize, aliases: &[bool]) -> Result<bool> {
    let row = &graph.inventory.operations()[index];
    let Kind::Cast {
        kind: CastKind::RestrictPointerAccess,
        to,
        ..
    } = &row.operation.kind
    else {
        return Ok(false);
    };
    if row.results.len() != 1 || row.operands.len() != 1 {
        return Ok(false);
    }
    let source = operand(graph, index, 0)?;
    if !aliases[graph.definition(source)?] {
        return Ok(false);
    }
    let Type::Pointer(from) = graph.inventory.definitions()[source].ty else {
        return Ok(false);
    };
    let Type::Pointer(to) = to else {
        return Ok(false);
    };
    Ok(from.address_space == AddressSpace::Private
        && to.address_space == AddressSpace::Private
        && from.access == AccessMode::ReadWrite
        && to.access == AccessMode::ReadOnly
        && from.pointee == to.pointee
        && graph.inventory.definitions()[row.results.start].ty == &row.operation.results[0].ty)
}

pub(super) fn derive(
    graph: &Graph<'_, '_>,
    reachable: &[bool],
    acyclic: bool,
    meter: &mut Meter<'_, '_, Error>,
) -> Result<Vec<Option<Read>>> {
    // Result/scratch headers, iteration state, borrowed graph rows and callback
    // frames are reserved independently of every vector backing below.
    meter.reserve(
        size_of::<Vec<Option<Read>>>()
            + 3 * size_of::<Vec<bool>>()
            + 2 * size_of::<Vec<usize>>()
            + size_of::<Option<(usize, usize)>>()
            + size_of::<Read>()
            + 32 * size_of::<usize>()
            + 12 * size_of::<&()>()
            + 8 * size_of::<&[usize]>()
            + 6 * size_of::<std::ops::Range<usize>>()
            + size_of::<MemoryAccess>()
            + size_of::<Result<bool>>()
            + size_of::<Result<usize>>(),
    )?;
    let mut reads = filled(meter, graph.function.operations.len(), None)?;
    if !acyclic {
        return Ok(reads);
    }
    let mut incoming = filled(meter, graph.blocks, 0usize)?;
    for (block, live) in reachable.iter().copied().enumerate() {
        meter.work(1)?;
        if live {
            for edge in graph.inventory.blocks()[graph.function.blocks.start + block]
                .edges
                .clone()
            {
                meter.work(1)?;
                let target = graph.target(edge)?;
                incoming[target] = incoming[target]
                    .checked_add(1)
                    .ok_or(Resource::Arithmetic)?;
            }
        }
    }
    let mut aliases = filled(meter, graph.definitions, false)?;
    let mut bypass = filled(meter, graph.blocks, false)?;
    let mut queue = meter.table(graph.blocks)?.0;
    for allocation in graph.function.operations.clone() {
        meter.work(1)?;
        let row = &graph.inventory.operations()[allocation];
        let Kind::Alloca {
            element: Type::StorageObject(layout),
            count: None,
            address_space: AddressSpace::Private,
            alignment,
        } = row.operation.kind
        else {
            continue;
        };
        if row.coordinate.block.block != 0 || row.results.len() != 1 {
            continue;
        }
        meter.work(1)?;
        let layout = graph
            .inventory
            .owner()
            .module()
            .storage_layouts
            .get(layout.0 as usize)
            .ok_or(Error::Inventory)?;
        let StorageLayoutKindV1::Scalar(scalar) = layout.kind else {
            continue;
        };
        if scalar
            .bit_width()
            .is_none_or(|bits| u64::from(bits.div_ceil(8)) != layout.size)
            || !alignment.is_power_of_two()
            || alignment < layout.alignment
        {
            continue;
        }
        meter.work(aliases.len())?;
        aliases.fill(false);
        aliases[graph.definition(row.results.start)?] = true;
        // Each successful round adds at least one definition; the bound also
        // covers canonical inventory order that differs from CFG order.
        for _ in 0..graph.definitions {
            let mut changed = false;
            for (block, live) in reachable.iter().copied().enumerate() {
                meter.work(1)?;
                if !live {
                    continue;
                }
                let block = &graph.inventory.blocks()[graph.function.blocks.start + block];
                for index in block.operations.clone() {
                    meter.work(1)?;
                    if restriction(graph, index, &aliases)? {
                        let result =
                            graph.definition(graph.inventory.operations()[index].results.start)?;
                        changed |= !aliases[result];
                        aliases[result] = true;
                    }
                }
                if !matches!(block.terminator, Terminator::Branch { .. }) {
                    continue;
                }
                for edge in block.edges.clone() {
                    meter.work(1)?;
                    if incoming[graph.target(edge)?] != 1 {
                        continue;
                    }
                    for binding in &graph.inventory.edge_arguments()
                        [graph.inventory.edges()[edge].bindings.clone()]
                    {
                        meter.work(1)?;
                        let source = graph.definition(binding.incoming_definition)?;
                        let target = graph.definition(binding.target_definition)?;
                        if aliases[source]
                            && graph.inventory.definitions()[binding.incoming_definition].ty
                                == graph.inventory.definitions()[binding.target_definition].ty
                        {
                            changed |= !aliases[target];
                            aliases[target] = true;
                        }
                    }
                }
            }
            if !changed {
                break;
            }
        }
        let mut writer = None;
        let mut valid = true;
        for (block, live) in reachable.iter().copied().enumerate() {
            meter.work(1)?;
            if !live {
                continue;
            }
            let row = &graph.inventory.blocks()[graph.function.blocks.start + block];
            for index in row.operations.clone() {
                meter.work(1)?;
                let operation = &graph.inventory.operations()[index];
                for (ordinal, used) in graph.inventory.uses()[operation.operands.clone()]
                    .iter()
                    .enumerate()
                {
                    meter.work(1)?;
                    if !aliases[graph.definition(used.definition)?] {
                        continue;
                    }
                    let permitted = match operation.operation.kind {
                        Kind::Storage(Storage::WriteValue { access, .. }) => {
                            if ordinal != 0
                                || operation.operands.len() != 2
                                || !operation.results.is_empty()
                                || !access_valid(access, alignment)
                                || graph.inventory.definitions()[operand(graph, index, 1)?].ty
                                    != &Type::Scalar(scalar)
                                || writer.is_some()
                            {
                                false
                            } else {
                                writer = Some((index, operand(graph, index, 1)?));
                                true
                            }
                        }
                        Kind::Storage(Storage::ReadValue { access, .. }) => {
                            ordinal == 0
                                && operation.operands.len() == 1
                                && operation.results.len() == 1
                                && access_valid(access, alignment)
                                && graph.inventory.definitions()[operation.results.start].ty
                                    == &Type::Scalar(scalar)
                        }
                        _ => restriction(graph, index, &aliases)?,
                    };
                    valid &= permitted;
                }
            }
            for used in &graph.inventory.uses()[row.terminator_uses.clone()] {
                meter.work(1)?;
                if !aliases[graph.definition(used.definition)?] {
                    continue;
                }
                if !matches!(row.terminator, Terminator::Branch { .. }) || row.edges.len() != 1 {
                    valid = false;
                    continue;
                }
                let edge = &graph.inventory.edges()[row.edges.start];
                for binding in &graph.inventory.edge_arguments()[edge.bindings.clone()] {
                    meter.work(1)?;
                    if binding.incoming_definition == used.definition
                        && !aliases[graph.definition(binding.target_definition)?]
                    {
                        valid = false;
                    }
                }
            }
        }
        let Some((writer, value)) = writer else {
            continue;
        };
        if !valid {
            continue;
        }
        let writer_row = &graph.inventory.operations()[writer];
        let writer_block = writer_row.coordinate.block.block as usize;
        meter.work(bypass.len())?;
        bypass.fill(false);
        queue.clear();
        if writer_block != 0 {
            bypass[0] = true;
            meter.push(&mut queue, 0)?;
        }
        let mut head = 0;
        while head < queue.len() {
            meter.work(1)?;
            let block = queue[head];
            head += 1;
            for edge in graph.inventory.blocks()[graph.function.blocks.start + block]
                .edges
                .clone()
            {
                meter.work(1)?;
                let target = graph.target(edge)?;
                if target != writer_block && !bypass[target] {
                    bypass[target] = true;
                    meter.push(&mut queue, target)?;
                }
            }
        }
        // Prove domination for every reachable read before publishing any fact.
        for index in graph.function.operations.clone() {
            meter.work(1)?;
            let row = &graph.inventory.operations()[index];
            let block = row.coordinate.block.block as usize;
            if reachable[block]
                && matches!(row.operation.kind, Kind::Storage(Storage::ReadValue { .. }))
                && aliases[graph.definition(operand(graph, index, 0)?)?]
                && (bypass[block] || (block == writer_block && index <= writer))
            {
                valid = false;
            }
        }
        if !valid {
            continue;
        }
        for index in graph.function.operations.clone() {
            meter.work(1)?;
            let row = &graph.inventory.operations()[index];
            if reachable[row.coordinate.block.block as usize]
                && matches!(row.operation.kind, Kind::Storage(Storage::ReadValue { .. }))
                && aliases[graph.definition(operand(graph, index, 0)?)?]
            {
                reads[index - graph.function.operations.start] = Some(Read {
                    value,
                    writer_block,
                });
            }
        }
    }
    Ok(reads)
}
