use super::*;

#[derive(Clone, Copy)]
struct Alias {
    allocation: usize,
    offset: u64,
    alignment: u32,
    direct: bool,
}
struct Candidate {
    row: Allocation,
    bytes: u32,
    eligible: bool,
    first: usize,
    last: usize,
}
#[derive(Clone, Copy)]
enum Memory {
    Store(ValueId),
    Load(ValueId),
}
#[derive(Clone, Copy)]
struct Observed {
    allocation: usize,
    offset: u64,
    ordinal: usize,
    pointer: ValueId,
    memory: Memory,
}
impl Observed {
    fn key(self) -> (usize, u64, usize) {
        (self.allocation, self.offset, self.ordinal)
    }
}
struct Scratch {
    producers: Vec<Option<usize>>,
    aliases: Vec<Option<Alias>>,
    candidates: Vec<Candidate>,
    addresses: Vec<(usize, Address)>,
    accesses: Vec<Observed>,
    barriers: Vec<usize>,
}

fn literal(
    inventory: &Inventory<'_>,
    producers: &[Option<usize>],
    definition: usize,
) -> Option<u64> {
    let ordinal = producers[definition]?;
    let row = &inventory.operations()[ordinal];
    if !row.effects.is_empty()
        || !row.compiler_ordering().is_empty()
        || row.results.len() != 1
        || *inventory.definitions()[definition].ty != Type::INDEX
    {
        return None;
    }
    match row.operation.kind {
        Kind::Constant(Constant::Index(value)) => Some(value),
        _ => None,
    }
}
fn operand(inventory: &Inventory<'_>, ordinal: usize, position: usize) -> Result<usize> {
    let row = &inventory.operations()[ordinal];
    if position >= row.operands.len() {
        return Err(Error::InconsistentInventory);
    }
    Ok(inventory.uses()[row.operands.start + position].definition)
}
fn effect(inventory: &Inventory<'_>, ordinal: usize, expected: u8) -> bool {
    let row = &inventory.operations()[ordinal];
    if row.effects.len() != 1 || !row.compiler_ordering().is_empty() {
        return false;
    }
    matches!(
        (expected, inventory.effects()[row.effects.start].effect),
        (0, Effect::Allocate(AddressSpace::Private))
            | (1, Effect::Read(AddressSpace::Private))
            | (2, Effect::Write(AddressSpace::Private))
    )
}

pub(super) fn derive<'i, 'g>(
    inventory: &'i Inventory<'g>,
    limits: Limits,
    meter: &mut Meter<'_, '_>,
) -> Result<(CanonicalKirPrivateCellCensusV1<'i, 'g>, Storage)> {
    type Census<'i, 'g> = CanonicalKirPrivateCellCensusV1<'i, 'g>;
    meter.work(3)?;
    meter.reserve(size_of::<Scratch>() + size_of::<Census<'_, '_>>())?;
    let definitions = inventory.definitions().len();
    let operations = inventory.operations().len();
    let (mut producers, pbytes) = meter.table(definitions)?;
    let (mut aliases, abytes) = meter.table(definitions)?;
    for _ in 0..definitions {
        meter.push(&mut producers, None)?;
        meter.push(&mut aliases, None)?;
    }
    let mut allocation_count = 0usize;
    let mut address_count = 0usize;
    let mut access_count = 0usize;
    for (ordinal, row) in inventory.operations().iter().enumerate() {
        meter.work(2)?;
        for definition in row.results.clone() {
            meter.work(2)?;
            if producers[definition].replace(ordinal).is_some() {
                return Err(Error::InconsistentInventory);
            }
        }
        match row.operation.kind {
            Kind::Alloca { .. } => {
                allocation_count = allocation_count
                    .checked_add(1)
                    .ok_or(Resource::Arithmetic)?;
                address_count = address_count.checked_add(1).ok_or(Resource::Arithmetic)?;
            }
            Kind::GetElementPointer { .. } => {
                address_count = address_count.checked_add(1).ok_or(Resource::Arithmetic)?;
            }
            Kind::Load { .. } | Kind::Store { .. } => {
                access_count = access_count.checked_add(1).ok_or(Resource::Arithmetic)?;
            }
            _ => {}
        }
    }
    let (candidates, cbytes) = meter.table(allocation_count)?;
    let (addresses, dbytes) = meter.table(address_count)?;
    let (accesses, ubytes) = meter.table(access_count)?;
    let (barriers, bbytes) = meter.table(operations.checked_add(1).ok_or(Resource::Arithmetic)?)?;
    let mut scratch = Scratch {
        producers,
        aliases,
        candidates,
        addresses,
        accesses,
        barriers,
    };
    discover_allocations(inventory, limits, &mut scratch, meter)?;
    discover_addresses(inventory, &mut scratch, meter)?;
    complete_uses(inventory, &mut scratch, meter)?;
    check_initialization(inventory, &mut scratch, meter)?;
    check_dominance(inventory, limits, &mut scratch, meter)?;

    let (mut allocations, retained_a) = meter.table(scratch.candidates.len())?;
    let (mut addresses, retained_d) = meter.table(scratch.addresses.len())?;
    let (mut accesses, retained_u) = meter.table(scratch.accesses.len())?;
    for candidate in &scratch.candidates {
        meter.work(1)?;
        if candidate.eligible {
            meter.push(&mut allocations, candidate.row)?;
        }
    }
    for (allocation, row) in &scratch.addresses {
        meter.work(1)?;
        if scratch.candidates[*allocation].eligible {
            meter.push(&mut addresses, *row)?;
        }
    }
    let mut latest: Option<(usize, u64, Coordinate, ValueId)> = None;
    for access in &scratch.accesses {
        meter.work(4)?;
        let candidate = &scratch.candidates[access.allocation];
        if !candidate.eligible {
            continue;
        }
        let coordinate = inventory.operations()[access.ordinal].coordinate;
        let kind = match access.memory {
            Memory::Store(value) => {
                latest = Some((access.allocation, access.offset, coordinate, value));
                AccessKind::Store { value }
            }
            Memory::Load(result) => {
                let Some((a, o, previous_store, stored_value)) = latest else {
                    return Err(Error::InconsistentInventory);
                };
                if (a, o) != (access.allocation, access.offset) {
                    return Err(Error::InconsistentInventory);
                }
                AccessKind::Load {
                    result,
                    previous_store,
                    stored_value,
                }
            }
        };
        meter.push(
            &mut accesses,
            Access {
                allocation: candidate.row.allocation,
                operation: coordinate,
                pointer: access.pointer,
                element_offset: access.offset,
                kind,
            },
        )?;
    }
    let scratch_bytes = [
        size_of::<Scratch>(),
        pbytes,
        abytes,
        cbytes,
        dbytes,
        ubytes,
        bbytes,
    ]
    .into_iter()
    .try_fold(0usize, |a, b| a.checked_add(b))
    .ok_or(Resource::Arithmetic)?;
    drop(scratch);
    meter.release(scratch_bytes)?;
    let retained = [
        size_of::<Census<'_, '_>>(),
        retained_a,
        retained_d,
        retained_u,
    ]
    .into_iter()
    .try_fold(0usize, |a, b| a.checked_add(b))
    .ok_or(Resource::Arithmetic)?;
    Ok((
        Census {
            inventory,
            allocations,
            addresses,
            accesses,
        },
        CanonicalKirPrivateCellCensusStorageV1(retained),
    ))
}

fn discover_allocations(
    inventory: &Inventory<'_>,
    limits: Limits,
    s: &mut Scratch,
    m: &mut Meter<'_, '_>,
) -> Result<()> {
    for (ordinal, op) in inventory.operations().iter().enumerate() {
        m.work(8)?;
        let Kind::Alloca {
            element: ref ty,
            count,
            address_space: AddressSpace::Private,
            alignment,
        } = op.operation.kind
        else {
            continue;
        };
        let Some((scalar, bytes)) = element(ty) else {
            continue;
        };
        if op.results.len() != 1
            || !effect(inventory, ordinal, 0)
            || !alignment.is_power_of_two()
            || alignment < bytes
        {
            continue;
        }
        let definition = op.results.start;
        if !pointer(inventory.definitions()[definition].ty, scalar) {
            continue;
        }
        let count = if count.is_some() {
            m.work(3)?;
            let Some(count) = literal(inventory, &s.producers, operand(inventory, ordinal, 0)?)
            else {
                continue;
            };
            count
        } else {
            1
        };
        if count == 0 || count > limits.array_count || count.checked_mul(u64::from(bytes)).is_none()
        {
            continue;
        }
        let value = inventory.definitions()[definition]
            .value
            .ok_or(Error::InconsistentInventory)?;
        let index = s.candidates.len();
        m.push(
            &mut s.candidates,
            Candidate {
                row: Allocation {
                    allocation: op.coordinate,
                    value,
                    element: scalar,
                    count,
                    alignment,
                    access_block: None,
                },
                bytes,
                eligible: true,
                first: usize::MAX,
                last: 0,
            },
        )?;
        s.aliases[definition] = Some(Alias {
            allocation: index,
            offset: 0,
            alignment,
            direct: true,
        });
        m.push(
            &mut s.addresses,
            (
                index,
                Address {
                    allocation: op.coordinate,
                    producer: op.coordinate,
                    value,
                    element_offset: 0,
                    alignment,
                },
            ),
        )?;
    }
    Ok(())
}
fn discover_addresses(
    inventory: &Inventory<'_>,
    s: &mut Scratch,
    m: &mut Meter<'_, '_>,
) -> Result<()> {
    // Every direct allocation was indexed first, independent of stored block order.
    // Nested GEPs never become aliases; their use of a first-level alias is later
    // rejected by the complete-use scan, including a backwards-stored block.
    for (ordinal, op) in inventory.operations().iter().enumerate() {
        m.work(5)?;
        let Kind::GetElementPointer { .. } = op.operation.kind else {
            continue;
        };
        let base = operand(inventory, ordinal, 0)?;
        let Some(alias) = s.aliases[base].filter(|a| a.direct) else {
            continue;
        };
        let candidate = &mut s.candidates[alias.allocation];
        let offset = literal(inventory, &s.producers, operand(inventory, ordinal, 1)?);
        let Some(offset) = offset.filter(|&o| o < candidate.row.count) else {
            candidate.eligible = false;
            continue;
        };
        m.work(5)?;
        if op.results.len() != 1
            || !op.effects.is_empty()
            || !op.compiler_ordering().is_empty()
            || !pointer(
                inventory.definitions()[op.results.start].ty,
                candidate.row.element,
            )
        {
            candidate.eligible = false;
            continue;
        }
        let displacement = offset
            .checked_mul(u64::from(candidate.bytes))
            .ok_or(Resource::Arithmetic)?;
        let alignment = if displacement == 0 {
            candidate.row.alignment
        } else {
            candidate
                .row
                .alignment
                .min(1u32 << displacement.trailing_zeros().min(31))
        };
        let value = inventory.definitions()[op.results.start]
            .value
            .ok_or(Error::InconsistentInventory)?;
        s.aliases[op.results.start] = Some(Alias {
            allocation: alias.allocation,
            offset,
            alignment,
            direct: false,
        });
        m.push(
            &mut s.addresses,
            (
                alias.allocation,
                Address {
                    allocation: candidate.row.allocation,
                    producer: op.coordinate,
                    value,
                    element_offset: offset,
                    alignment,
                },
            ),
        )?;
    }
    Ok(())
}

fn memory(
    inventory: &Inventory<'_>,
    ordinal: usize,
    s: &Scratch,
) -> Result<Option<(Alias, ValueId, Memory)>> {
    let op = &inventory.operations()[ordinal];
    let (ptr, access, kind) = match op.operation.kind {
        Kind::Load { pointer, access } if op.results.len() == 1 => (
            pointer,
            access,
            Memory::Load(
                inventory.definitions()[op.results.start]
                    .value
                    .ok_or(Error::InconsistentInventory)?,
            ),
        ),
        Kind::Store {
            pointer,
            value,
            access,
        } if op.results.is_empty() => (pointer, access, Memory::Store(value)),
        _ => return Ok(None),
    };
    let Some(alias) = s.aliases[operand(inventory, ordinal, 0)?] else {
        return Ok(None);
    };
    let candidate = &s.candidates[alias.allocation];
    let ty = match kind {
        Memory::Load(_) => inventory.definitions()[op.results.start].ty,
        Memory::Store(_) => inventory.definitions()[operand(inventory, ordinal, 1)?].ty,
    };
    let expected_effect = if matches!(kind, Memory::Load(_)) {
        1
    } else {
        2
    };
    if *ty != Type::Scalar(candidate.row.element)
        || access.address_space != AddressSpace::Private
        || access.volatile
        || !access.alignment.is_power_of_two()
        || access.alignment < candidate.bytes
        || access.alignment > alias.alignment
        || !effect(inventory, ordinal, expected_effect)
    {
        return Ok(None);
    }
    Ok(Some((alias, ptr, kind)))
}

fn complete_uses(inventory: &Inventory<'_>, s: &mut Scratch, m: &mut Meter<'_, '_>) -> Result<()> {
    let mut barriers = 0usize;
    m.push(&mut s.barriers, barriers)?;
    for (ordinal, op) in inventory.operations().iter().enumerate() {
        m.work(12)?;
        let memory = memory(inventory, ordinal, s)?;
        let valid_gep = matches!(op.operation.kind, Kind::GetElementPointer { .. })
            && op.results.len() == 1
            && s.aliases[op.results.start].is_some();
        for (position, usage) in inventory.uses()[op.operands.clone()].iter().enumerate() {
            m.work(3)?;
            if let Some(alias) = s.aliases[usage.definition] {
                let allowed = position == 0 && (memory.is_some() || (valid_gep && alias.direct));
                if !allowed {
                    s.candidates[alias.allocation].eligible = false;
                }
            }
        }
        if let Some((alias, ptr, memory)) = memory {
            let candidate = &mut s.candidates[alias.allocation];
            if candidate
                .row
                .access_block
                .is_some_and(|b| b != op.coordinate.block)
            {
                candidate.eligible = false;
            } else {
                candidate.row.access_block = Some(op.coordinate.block);
            }
            candidate.first = candidate.first.min(ordinal);
            candidate.last = candidate.last.max(ordinal);
            m.push(
                &mut s.accesses,
                Observed {
                    allocation: alias.allocation,
                    offset: alias.offset,
                    ordinal,
                    pointer: ptr,
                    memory,
                },
            )?;
        }
        // Computations remain intact, including possibly trapping arithmetic.
        // This is only an interval fence census, never permission to delete them.
        let scalar_computation = op.effects.is_empty()
            && op.compiler_ordering().is_empty()
            && matches!(
                op.operation.kind,
                Kind::Constant(_)
                    | Kind::Unary { .. }
                    | Kind::Binary { .. }
                    | Kind::Compare { .. }
                    | Kind::Cast { .. }
                    | Kind::Select { .. }
            );
        let allocation = matches!(op.operation.kind, Kind::Alloca { .. })
            && op.results.len() == 1
            && s.aliases[op.results.start].is_some();
        if !(scalar_computation || allocation || valid_gep || memory.is_some()) {
            barriers = barriers.checked_add(1).ok_or(Resource::Arithmetic)?;
        }
        m.push(&mut s.barriers, barriers)?;
    }
    for block in inventory.blocks() {
        m.work(1)?;
        for usage in &inventory.uses()[block.terminator_uses.clone()] {
            m.work(2)?;
            if let Some(alias) = s.aliases[usage.definition] {
                s.candidates[alias.allocation].eligible = false;
            }
        }
    }
    for edge in inventory.edge_arguments() {
        m.work(2)?;
        if let Some(alias) = s.aliases[edge.incoming_definition] {
            s.candidates[alias.allocation].eligible = false;
        }
    }
    for candidate in &mut s.candidates {
        m.work(2)?;
        if candidate.first != usize::MAX
            && s.barriers[candidate.first] != s.barriers[candidate.last + 1]
        {
            candidate.eligible = false;
        }
    }
    Ok(())
}

fn check_initialization(
    inventory: &Inventory<'_>,
    s: &mut Scratch,
    m: &mut Meter<'_, '_>,
) -> Result<()> {
    sort(&mut s.accesses, m)?;
    let mut initialized = None;
    for access in &s.accesses {
        m.work(3)?;
        match access.memory {
            Memory::Store(_) => initialized = Some((access.allocation, access.offset)),
            Memory::Load(_) if initialized != Some((access.allocation, access.offset)) => {
                s.candidates[access.allocation].eligible = false
            }
            Memory::Load(_) => {}
        }
        if inventory.operations()[access.ordinal].coordinate.block
            != s.candidates[access.allocation]
                .row
                .access_block
                .ok_or(Error::InconsistentInventory)?
        {
            s.candidates[access.allocation].eligible = false;
        }
    }
    Ok(())
}
fn sort(rows: &mut [Observed], m: &mut Meter<'_, '_>) -> Result<()> {
    fn down(
        rows: &mut [Observed],
        mut root: usize,
        end: usize,
        m: &mut Meter<'_, '_>,
    ) -> Result<()> {
        loop {
            m.work(1)?;
            let child = root
                .checked_mul(2)
                .and_then(|n| n.checked_add(1))
                .ok_or(Resource::Arithmetic)?;
            if child >= end {
                break;
            }
            let mut selected = child;
            if child + 1 < end {
                m.work(1)?;
                if rows[child].key() < rows[child + 1].key() {
                    selected += 1;
                }
            }
            m.work(1)?;
            if rows[root].key() >= rows[selected].key() {
                break;
            }
            m.work(1)?;
            rows.swap(root, selected);
            root = selected;
        }
        Ok(())
    }
    let length = rows.len();
    for root in (0..length / 2).rev() {
        down(rows, root, length, m)?;
    }
    for end in (1..rows.len()).rev() {
        m.work(1)?;
        rows.swap(0, end);
        down(rows, 0, end, m)?;
    }
    Ok(())
}

fn check_dominance(
    inventory: &Inventory<'_>,
    limits: Limits,
    s: &mut Scratch,
    m: &mut Meter<'_, '_>,
) -> Result<()> {
    let mut candidate_start = 0;
    // Candidates are in actual global operation/function order; no per-function
    // rescan of all candidates or new editable CFG is needed.
    for function in inventory.functions() {
        m.work(1)?;
        let mut end = candidate_start;
        while end < s.candidates.len() {
            m.work(1)?;
            if s.candidates[end].row.allocation.block.function != function.coordinate {
                break;
            }
            end += 1;
        }
        if end == candidate_start {
            continue;
        }
        m.cfg(|budget| {
            with_canonical_kir_control_flow_v1(
                inventory.owner(),
                function.coordinate,
                limits.control_flow,
                budget,
                |view, budget| -> Result<()> {
                    for candidate in &mut s.candidates[candidate_start..end] {
                        budget.charge_work(2)?;
                        let allocation = candidate.row.allocation;
                        if !view.is_reachable(allocation.block, budget)? {
                            candidate.eligible = false;
                        }
                        if let Some(block) = candidate.row.access_block {
                            if !view.dominates(allocation.block, block, budget)? {
                                candidate.eligible = false;
                            }
                            if allocation.block == block
                                && allocation.operation
                                    >= inventory.operations()[candidate.first].coordinate.operation
                            {
                                candidate.eligible = false;
                            }
                        }
                    }
                    Ok(())
                },
            )
        })?;
        candidate_start = end;
    }
    if candidate_start != s.candidates.len() {
        return Err(Error::InconsistentInventory);
    }
    Ok(())
}
