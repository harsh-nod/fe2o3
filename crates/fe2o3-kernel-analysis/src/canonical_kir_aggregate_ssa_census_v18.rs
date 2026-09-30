use super::*;

#[path = "canonical_kir_aggregate_layout_worklist_v36.rs"]
mod layout_worklist;

#[derive(Clone, Copy)]
pub(super) struct Allocation {
    pub operation: usize,
    pub eligible: bool,
}
#[derive(Clone, Copy, Eq, PartialEq)]
pub(super) struct Address {
    pub allocation: usize,
    pub layout: StorageLayoutIdV1,
    pub offset: u64,
    pub alignment: u32,
}
#[derive(Clone, Copy)]
pub(super) struct Cell {
    pub address: Address,
    pub ty: LeafType,
}
#[derive(Clone, Copy)]
pub(super) enum Event {
    None,
    Allocate(usize),
    Project(usize),
    Read(usize),
    Write(usize, ValueId),
}
pub(super) struct Census {
    pub allocations: Vec<Allocation>,
    pub cells: Vec<Cell>,
    pub events: Vec<Event>,
}

fn filled<T: Copy>(count: usize, value: T, meter: &mut Meter<'_, '_>) -> Result<Vec<T>> {
    let (mut rows, _) = meter.table(count)?;
    meter.work(count)?;
    rows.resize(count, value);
    Ok(rows)
}
fn operand(i: &Inventory<'_>, op: usize, n: usize) -> Result<usize> {
    let row = &i.operations()[op];
    if n >= row.operands.len() {
        return Err(Error::Inconsistent("inventory operand"));
    }
    Ok(i.uses()[row.operands.start + n].definition)
}
fn literal(i: &Inventory<'_>, producers: &[Option<usize>], definition: usize) -> Option<u64> {
    let op = i.operations().get(producers[definition]?)?;
    if op.results.len() != 1 || *i.definitions()[definition].ty != Type::INDEX {
        return None;
    }
    match op.operation.kind {
        Kind::Constant(Constant::Index(n)) => Some(n),
        _ => None,
    }
}
fn root_type(ty: &Type, layout: StorageLayoutIdV1) -> bool {
    matches!(ty, Type::Pointer(p) if p.address_space == AddressSpace::Private
        && p.access == AccessMode::ReadWrite && *p.pointee == Type::StorageObject(layout))
}
fn safe_layouts(i: &Inventory<'_>, meter: &mut Meter<'_, '_>) -> Result<Vec<Option<bool>>> {
    layout_worklist::derive(&i.owner().module().storage_layouts, meter)
}
fn project(
    i: &Inventory<'_>,
    producers: &[Option<usize>],
    op: usize,
    base: Address,
    meter: &mut Meter<'_, '_>,
) -> Result<Option<Address>> {
    meter.work(8)?;
    let row = &i.operations()[op];
    if row.results.len() != 1 || !row.compiler_ordering().is_empty() || !row.effects.is_empty() {
        return Ok(None);
    }
    let Kind::Storage(Storage::Project { step, .. }) = row.operation.kind else {
        return Ok(None);
    };
    let layout = &i.owner().module().storage_layouts[base.layout.0 as usize];
    let (child, offset) = match (&layout.kind, step) {
        (LayoutKind::Record(fields), Projection::Field(n)) => {
            let Some(field) = fields.get(n as usize) else {
                return Ok(None);
            };
            (field.layout, field.offset)
        }
        (
            LayoutKind::Array {
                element,
                length,
                stride,
            },
            Projection::ArrayIndex(_),
        ) => {
            let Some(index) = literal(i, producers, operand(i, op, 1)?) else {
                return Ok(None);
            };
            if index >= *length {
                return Ok(None);
            }
            (
                *element,
                index.checked_mul(*stride).ok_or(Resource::Arithmetic)?,
            )
        }
        _ => return Ok(None),
    };
    if !root_type(&row.operation.results[0].ty, child) {
        return Ok(None);
    }
    let alignment = fe2o3_kernel_ir::StorageFieldV1 {
        offset,
        layout: child,
    }
    .placement_alignment(base.alignment)
    .ok_or(Error::Inconsistent("admitted private placement alignment"))?;
    Ok(Some(Address {
        layout: child,
        offset: base
            .offset
            .checked_add(offset)
            .ok_or(Resource::Arithmetic)?,
        alignment,
        ..base
    }))
}
fn access_ok(access: MemoryAccess, address: Address) -> bool {
    access.address_space == AddressSpace::Private
        && !access.volatile
        && access.alignment <= address.alignment
}

pub(super) fn derive(i: &Inventory<'_>, meter: &mut Meter<'_, '_>) -> Result<Census> {
    type ScratchHeaders = (
        Vec<Option<usize>>,
        Vec<Option<Address>>,
        Vec<bool>,
        Vec<Option<bool>>,
        Vec<usize>,
        Vec<Allocation>,
        Vec<Cell>,
        Vec<Event>,
    );
    meter
        .reserve(size_of::<Census>() + size_of::<ScratchHeaders>() + size_of::<Result<Census>>())?;
    let count = i.definitions().len();
    let mut producers = filled(count, None, meter)?;
    for (op, row) in i.operations().iter().enumerate() {
        for definition in row.results.clone() {
            meter.work(1)?;
            producers[definition] = Some(op);
        }
    }
    let safe = safe_layouts(i, meter)?;
    let mut aliases = filled(count, None, meter)?;
    let mut done = filled(count, false, meter)?;
    let (mut allocations, _) = meter.table(i.operations().len())?;
    let mut events = filled(i.operations().len(), Event::None, meter)?;
    for (op, row) in i.operations().iter().enumerate() {
        meter.work(8)?;
        let Kind::Alloca {
            element: Type::StorageObject(layout),
            count,
            address_space: AddressSpace::Private,
            alignment,
        } = row.operation.kind
        else {
            continue;
        };
        if row.results.len() != 1
            || !row.compiler_ordering().is_empty()
            || !root_type(&row.operation.results[0].ty, layout)
            || safe[layout.0 as usize] != Some(true)
            || alignment < i.owner().module().storage_layouts[layout.0 as usize].alignment
            || (count.is_some() && literal(i, &producers, operand(i, op, 0)?) != Some(1))
        {
            continue;
        }
        let allocation = allocations.len();
        meter.push(
            &mut allocations,
            Allocation {
                operation: op,
                eligible: true,
            },
        )?;
        aliases[row.results.start] = Some(Address {
            allocation,
            layout,
            offset: 0,
            alignment,
        });
        done[row.results.start] = true;
        events[op] = Event::Allocate(allocation);
    }
    // Resolve each original projection chain once, independent of physical
    // block order. Unsupported edges remain visible to the complete use census.
    let (mut stack, _) = meter.table::<usize>(count)?;
    for definition in 0..count {
        let mut at = definition;
        while !done[at] {
            meter.work(2)?;
            done[at] = true;
            let Some(op) = producers[at] else {
                break;
            };
            if !matches!(
                i.operations()[op].operation.kind,
                Kind::Storage(Storage::Project { .. })
            ) {
                break;
            }
            meter.push(&mut stack, at)?;
            at = operand(i, op, 0)?;
        }
        while let Some(at) = stack.pop() {
            meter.work(3)?;
            let op = producers[at].ok_or(Error::Inconsistent("projection producer"))?;
            let base = operand(i, op, 0)?;
            if let Some(base) = aliases[base] {
                aliases[at] = project(i, &producers, op, base, meter)?;
                if aliases[at].is_some() {
                    events[op] = Event::Project(base.allocation);
                }
            }
        }
    }
    let (mut cells, _) = meter.table::<Cell>(i.operations().len())?;
    for (op, row) in i.operations().iter().enumerate() {
        meter.work(6)?;
        let (access, write) = match row.operation.kind {
            Kind::Storage(Storage::ReadValue { access, .. }) => (access, None),
            Kind::Storage(Storage::WriteValue { access, value, .. }) => (access, Some(value)),
            _ => continue,
        };
        let Some(address) = aliases[operand(i, op, 0)?] else {
            continue;
        };
        let leaf = &i.owner().module().storage_layouts[address.layout.0 as usize];
        let ty = match leaf.kind {
            LayoutKind::Scalar(s) => LeafType::Scalar(s),
            LayoutKind::Vector(v) => LeafType::Vector(v),
            _ => continue,
        };
        if !access_ok(access, address) || !row.compiler_ordering().is_empty() {
            continue;
        }
        let actual_type = if write.is_some() {
            i.definitions()[operand(i, op, 1)?].ty
        } else if row.results.len() == 1 {
            &row.operation.results[0].ty
        } else {
            continue;
        };
        if *actual_type != ty.ty() {
            continue;
        }
        meter.work(cells.len())?;
        let cell = match cells.iter().position(|cell| {
            cell.address.allocation == address.allocation
                && cell.address.offset == address.offset
                && cell.address.layout == address.layout
        }) {
            Some(cell) => cell,
            None => {
                let cell = cells.len();
                meter.push(&mut cells, Cell { address, ty })?;
                cell
            }
        };
        events[op] = match write {
            Some(value) => Event::Write(cell, value),
            None => Event::Read(cell),
        };
    }
    for (op, row) in i.operations().iter().enumerate() {
        for (position, use_index) in row.operands.clone().enumerate() {
            meter.work(3)?;
            let Some(address) = aliases[i.uses()[use_index].definition] else {
                continue;
            };
            let allowed = position == 0
                && match events[op] {
                    Event::Project(a) => a == address.allocation,
                    Event::Read(c) | Event::Write(c, _) => {
                        cells[c].address.allocation == address.allocation
                    }
                    _ => false,
                };
            if !allowed {
                allocations[address.allocation].eligible = false;
            }
        }
    }
    for block in i.blocks() {
        for use_index in block.terminator_uses.clone() {
            meter.work(2)?;
            if let Some(address) = aliases[i.uses()[use_index].definition] {
                allocations[address.allocation].eligible = false;
            }
        }
    }
    for edge in i.edge_arguments() {
        meter.work(2)?;
        if let Some(address) = aliases[edge.incoming_definition] {
            allocations[address.allocation].eligible = false;
        }
    }
    for row in i.operations() {
        meter.work(2)?;
        if matches!(
            row.operation.kind,
            Kind::Call { .. } | Kind::InlineAssembly(_)
        ) {
            for allocation in &mut allocations {
                meter.work(1)?;
                if i.operations()[allocation.operation]
                    .coordinate
                    .block
                    .function
                    == row.coordinate.block.function
                {
                    allocation.eligible = false;
                }
            }
        }
    }
    Ok(Census {
        allocations,
        cells,
        events,
    })
}
