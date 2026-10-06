use super::*;
use census::{Census, Event};

struct Input {
    graph: SsaConstructionInputV1,
    operations: Vec<Vec<usize>>,
    cells: Vec<usize>,
}
fn variable(n: usize) -> Result<SsaVariableIdV1> {
    Ok(SsaVariableIdV1::new(
        n.try_into().map_err(|_| Resource::Arithmetic)?,
    ))
}
fn input(
    i: &Inventory<'_>,
    c: &Census,
    function: usize,
    meter: &mut Meter<'_, '_>,
) -> Result<Input> {
    type ScratchHeaders = (
        Vec<usize>,
        Vec<bool>,
        Vec<SsaBlockInputV1>,
        Vec<Vec<usize>>,
        Vec<SsaEventV1>,
        Vec<usize>,
        Vec<SsaEdgeInputV1>,
        Result<Input>,
    );
    meter.reserve(size_of::<ScratchHeaders>())?;
    let f = &i.functions()[function];
    let mut cells = Vec::new();
    for (index, cell) in c.cells.iter().enumerate() {
        meter.work(2)?;
        let allocation = &c.allocations[cell.address.allocation];
        if allocation.eligible && f.operations.contains(&allocation.operation) {
            append(&mut cells, index, meter)?;
        }
    }
    let (mut promotable, _) = meter.table(cells.len())?;
    meter.work(cells.len())?;
    promotable.resize(cells.len(), true);
    let (mut blocks, _) = meter.table(f.blocks.len())?;
    let (mut operations, _) = meter.table(f.blocks.len())?;
    for block in &i.blocks()[f.blocks.clone()] {
        let mut events = Vec::new();
        let mut map = Vec::new();
        for op in block.operations.clone() {
            meter.work(2)?;
            match c.events[op] {
                Event::Allocate(allocation) if c.allocations[allocation].eligible => {
                    for (cell, original) in cells.iter().enumerate() {
                        meter.work(1)?;
                        if c.cells[*original].address.allocation == allocation {
                            append(&mut events, SsaEventV1::Kill(variable(cell)?), meter)?;
                            append(&mut map, op, meter)?;
                        }
                    }
                }
                Event::Read(original) | Event::Write(original, _)
                    if c.allocations[c.cells[original].address.allocation].eligible =>
                {
                    meter.work(usize::BITS as usize - cells.len().leading_zeros() as usize + 1)?;
                    let cell = cells
                        .binary_search(&original)
                        .map_err(|_| Error::Inconsistent("function-local cell index"))?;
                    let event = if matches!(c.events[op], Event::Read(_)) {
                        SsaEventV1::Use(variable(cell)?)
                    } else {
                        SsaEventV1::Define(variable(cell)?)
                    };
                    append(&mut events, event, meter)?;
                    append(&mut map, op, meter)?;
                }
                _ => {}
            }
        }
        let (mut edges, _) = meter.table(block.edges.len())?;
        for edge in &i.edges()[block.edges.clone()] {
            meter.work(3)?;
            // Role identifies the successor occurrence; parallel target edges
            // are never merged. The planner only requires a nonzero role tag.
            meter.push(
                &mut edges,
                SsaEdgeInputV1::new(
                    SsaEdgeRoleV1::new(1),
                    SsaBlockIdV1::new(edge.target.block),
                    Vec::new(),
                ),
            )?;
        }
        meter.push(&mut blocks, SsaBlockInputV1::new(events, edges))?;
        meter.push(&mut operations, map)?;
    }
    Ok(Input {
        graph: SsaConstructionInputV1::new(
            SsaBlockIdV1::new(0),
            cells.len().try_into().map_err(|_| Resource::Arithmetic)?,
            promotable,
            Vec::new(),
            blocks,
        ),
        operations,
        cells,
    })
}

// The existing planner enforces its own logical word/work ceilings before each
// allocation/step. Prepay those exact closed ceilings on the caller's ledger;
// never run first and retroactively debit a successful resource report. These
// conservative input-shaped caps never exceed the planner's existing defaults.
fn planner(input: &Input, meter: &mut Meter<'_, '_>) -> Result<SsaConstructionPlanV1> {
    let graph = &input.graph;
    let b = graph.blocks().len();
    let v = graph.variable_count() as usize;
    let (mut e, mut events) = (0usize, 0usize);
    for block in graph.blocks() {
        meter.work(1)?;
        e = add(e, block.edges().len())?;
        events = add(events, block.events().len())?;
    }
    let shape = add(add(add(b, e)?, events)?, add(v, 1)?)?;
    let width = add(add(b, v)?, 1)?;
    let defaults = SsaPlannerLimitsV1::default();
    let storage = shape
        .checked_mul(width)
        .and_then(|n| n.checked_mul(64))
        .ok_or(Resource::Arithmetic)?
        .min(defaults.max_storage_words());
    let work = shape
        .checked_mul(width)
        .and_then(|n| n.checked_mul(width))
        .and_then(|n| n.checked_mul(128))
        .ok_or(Resource::Arithmetic)?
        .min(defaults.max_work_units());
    let output = shape
        .checked_mul(add(v, 1)?)
        .ok_or(Resource::Arithmetic)?
        .min(defaults.max_output_items());
    let limits = SsaPlannerLimitsV1::try_new(v, b, e, events, 0, output, storage, work)?;
    meter.reserve(add(
        storage
            .checked_mul(size_of::<u64>())
            .ok_or(Resource::Arithmetic)?,
        size_of::<SsaConstructionPlanV1>()
            + size_of::<std::result::Result<SsaConstructionPlanV1, SsaPlannerErrorV1>>(),
    )?)?;
    meter.work(work)?;
    Ok(plan_ssa_with_limits_v1(graph, limits)?)
}
fn undefined(error: &SsaPlannerErrorV1) -> Option<usize> {
    match error {
        SsaPlannerErrorV1::UndefinedAtUse { variable, .. }
        | SsaPlannerErrorV1::UndefinedAtEdge { variable, .. }
        | SsaPlannerErrorV1::UndefinedAtEntry { variable } => Some(variable.get() as usize),
        _ => None,
    }
}
fn new_value(next: &mut u64) -> Result<ValueId> {
    let id = ValueId((*next).try_into().map_err(|_| Resource::Arithmetic)?);
    *next = next.checked_add(1).ok_or(Resource::Arithmetic)?;
    Ok(id)
}
fn resolve(
    value: SsaValueV1,
    definitions: &[Option<ValueId>],
    parameters: &[Option<ValueId>],
    cells: usize,
) -> Result<ValueId> {
    let found = match value {
        SsaValueV1::Definition(id) => definitions.get(id.get() as usize).copied().flatten(),
        SsaValueV1::BlockArgument { block, variable } => parameters
            .get(
                (block.get() as usize)
                    .checked_mul(cells)
                    .and_then(|n| n.checked_add(variable.get() as usize))
                    .ok_or(Resource::Arithmetic)?,
            )
            .copied()
            .flatten(),
    };
    found.ok_or(Error::Inconsistent("resolved original SSA value"))
}
fn emit(
    i: &Inventory<'_>,
    c: &Census,
    function: usize,
    input: &Input,
    plan: &SsaConstructionPlanV1,
    witness: &mut Witness,
    meter: &mut Meter<'_, '_>,
) -> Result<()> {
    meter.reserve(
        size_of::<(Vec<Option<ValueId>>, Vec<Option<ValueId>>)>() + size_of::<Result<()>>(),
    )?;
    let f = &i.functions()[function];
    let mut next = 0u64;
    for d in &i.definitions()[f.definitions.clone()] {
        meter.work(1)?;
        if let Some(value) = d.value {
            next = next.max(u64::from(value.0) + 1);
        }
    }
    let (mut definitions, _) = meter.table(plan.definition_count())?;
    meter.work(plan.definition_count())?;
    definitions.resize(plan.definition_count(), None);
    let slots = f
        .blocks
        .len()
        .checked_mul(input.cells.len())
        .ok_or(Resource::Arithmetic)?;
    let (mut parameters, _) = meter.table(slots)?;
    meter.work(slots)?;
    parameters.resize(slots, None);
    for (local, block) in i.blocks()[f.blocks.clone()].iter().enumerate() {
        meter.work(2)?;
        if let Some(variables) = plan.transport_variables(SsaBlockIdV1::new(local as u32)) {
            for variable in variables {
                let cell = variable.get() as usize;
                let value = new_value(&mut next)?;
                parameters[local * input.cells.len() + cell] = Some(value);
                append(
                    &mut witness.memory_parameters,
                    CanonicalKirAggregateSsaMemoryParameterV18 {
                        parameter: witness.parameters.len(),
                        slot: input.cells[cell],
                    },
                    meter,
                )?;
                append(
                    &mut witness.parameters,
                    Parameter {
                        block: f.blocks.start + local,
                        value,
                        ty: c.cells[input.cells[cell]].ty,
                    },
                    meter,
                )?;
            }
        }
        if let Some(events) = plan.resolved_events(SsaBlockIdV1::new(local as u32)) {
            for (event, resolved) in events {
                meter.work(3)?;
                if let SsaResolvedEventV1::Define {
                    value: SsaValueV1::Definition(id),
                    ..
                } = resolved
                {
                    let op = input.operations[local][*event as usize];
                    let Event::Write(_, original) = c.events[op] else {
                        return Err(Error::Inconsistent("SSA definition is original store"));
                    };
                    definitions[id.get() as usize] = Some(original);
                }
            }
        }
        // The existing graph remains in its original block/operation order.
        let _ = block;
    }
    for (local, block) in i.blocks()[f.blocks.clone()].iter().enumerate() {
        if let Some(events) = plan.resolved_events(SsaBlockIdV1::new(local as u32)) {
            for (event, resolved) in events {
                meter.work(2)?;
                if let SsaResolvedEventV1::Use { value, .. } = resolved {
                    let op = input.operations[local][*event as usize];
                    if !matches!(c.events[op], Event::Read(_)) {
                        return Err(Error::Inconsistent("SSA use is original read"));
                    }
                    witness.actions[op] = Action::Copy(resolve(
                        *value,
                        &definitions,
                        &parameters,
                        input.cells.len(),
                    )?);
                    if witness.conditions[function].is_none() {
                        witness.conditions[function] = Some(new_value(&mut next)?);
                    }
                }
            }
        }
        for (ordinal, edge) in block.edges.clone().enumerate() {
            if let Some(arguments) = plan.edge_arguments(SsaEdgeIdV1::new(
                SsaBlockIdV1::new(local as u32),
                ordinal as u32,
            )) {
                for argument in arguments {
                    append(
                        &mut witness.arguments,
                        EdgeArgument {
                            edge,
                            value: resolve(
                                argument.value(),
                                &definitions,
                                &parameters,
                                input.cells.len(),
                            )?,
                        },
                        meter,
                    )?;
                }
            }
        }
    }
    append(&mut witness.planners, (function, plan.identity()), meter)
}

pub(super) fn derive(i: &Inventory<'_>, meter: &mut Meter<'_, '_>) -> Result<Witness> {
    meter.reserve(size_of::<Witness>() + size_of::<Input>() + size_of::<SsaPlannerLimitsV1>())?;
    let mut census = census::derive(i, meter)?;
    // A no-access allocation is not silently treated as a scalar variable.
    for (a, allocation) in census.allocations.iter_mut().enumerate() {
        meter.work(census.cells.len())?;
        allocation.eligible &= census.cells.iter().any(|c| c.address.allocation == a);
    }
    let (mut actions, _) = meter.table(i.operations().len())?;
    meter.work(i.operations().len())?;
    actions.resize(i.operations().len(), Action::Retain);
    let (mut conditions, _) = meter.table(i.functions().len())?;
    meter.work(i.functions().len())?;
    conditions.resize(i.functions().len(), None);
    let mut witness = Witness {
        actions,
        conditions,
        parameters: Vec::new(),
        arguments: Vec::new(),
        selected: Vec::new(),
        planners: Vec::new(),
        memory_slots: Vec::new(),
        memory_events: Vec::new(),
        memory_parameters: Vec::new(),
    };
    for function in 0..i.functions().len() {
        let f = &i.functions()[function];
        if f.blocks.is_empty() {
            continue;
        }
        // Each failed initialization/reachability attempt permanently excludes
        // at least one original allocation. Work is cumulative across retries.
        for attempt in 0..=census.allocations.len() {
            meter.work(census.allocations.len())?;
            if !census
                .allocations
                .iter()
                .any(|a| a.eligible && f.operations.contains(&a.operation))
            {
                break;
            }
            if attempt == census.allocations.len() {
                return Err(Error::Inconsistent("bounded allocation exclusions"));
            }
            let input = input(i, &census, function, meter)?;
            let plan = match planner(&input, meter) {
                Ok(plan) => plan,
                Err(Error::Planner(error)) if undefined(&error).is_some() => {
                    let cell =
                        undefined(&error).ok_or(Error::Inconsistent("undefined variable"))?;
                    let original = *input
                        .cells
                        .get(cell)
                        .ok_or(Error::Inconsistent("undefined function-local cell"))?;
                    let allocation = census
                        .cells
                        .get(original)
                        .ok_or(Error::Inconsistent("undefined cell"))?
                        .address
                        .allocation;
                    if !census.allocations[allocation].eligible {
                        return Err(Error::Inconsistent("repeat undefined allocation"));
                    }
                    census.allocations[allocation].eligible = false;
                    continue;
                }
                Err(error) => return Err(error),
            };
            let mut excluded = false;
            for (local, block) in i.blocks()[f.blocks.clone()].iter().enumerate() {
                if plan.is_reachable(SsaBlockIdV1::new(local as u32)) {
                    continue;
                }
                for op in block.operations.clone() {
                    meter.work(2)?;
                    let allocation = match census.events[op] {
                        Event::Allocate(a) | Event::Project(a) => Some(a),
                        Event::Read(c) | Event::Write(c, _) => {
                            Some(census.cells[c].address.allocation)
                        }
                        Event::None => None,
                    };
                    if let Some(a) = allocation {
                        excluded |= census.allocations[a].eligible;
                        census.allocations[a].eligible = false;
                    }
                }
            }
            if excluded {
                continue;
            }
            emit(i, &census, function, &input, &plan, &mut witness, meter)?;
            break;
        }
    }
    for cell in &census.cells {
        meter.work(1)?;
        let allocation = &census.allocations[cell.address.allocation];
        let slot = allocation
            .eligible
            .then_some(CanonicalKirAggregateSsaMemorySlotV18 {
                allocation: allocation.operation,
                layout: cell.address.layout,
                offset: cell.address.offset,
                ty: cell.ty,
            });
        append(&mut witness.memory_slots, slot, meter)?;
    }
    for (op, event) in census.events.iter().enumerate() {
        meter.work(2)?;
        let a = match event {
            Event::Allocate(a) | Event::Project(a) => Some(*a),
            Event::Read(c) | Event::Write(c, _) => Some(census.cells[*c].address.allocation),
            Event::None => None,
        };
        let mut memory_event = CanonicalKirAggregateSsaMemoryEventV18::None;
        if a.is_some_and(|a| census.allocations[a].eligible) {
            if matches!(event, Event::Read(_)) {
                if !matches!(witness.actions[op], Action::Copy(_)) {
                    return Err(Error::Inconsistent("selected read complete SSA resolution"));
                }
            } else {
                witness.actions[op] = Action::Remove;
            }
            if matches!(event, Event::Allocate(_)) {
                append(&mut witness.selected, op, meter)?;
            }
            use CanonicalKirAggregateSsaMemoryEventV18 as Memory;
            memory_event = match *event {
                Event::Allocate(a) => Memory::Allocate {
                    allocation: census.allocations[a].operation,
                },
                Event::Project(a) => Memory::Project {
                    allocation: census.allocations[a].operation,
                },
                Event::Write(slot, value) => Memory::Write { slot, value },
                Event::Read(slot) => {
                    let Action::Copy(replacement) = witness.actions[op] else {
                        return Err(Error::Inconsistent("memory read has exact replacement"));
                    };
                    Memory::Read {
                        slot,
                        output: i.operations()[op].operation.results[0].id,
                        replacement,
                    }
                }
                Event::None => return Err(Error::Inconsistent("selected memory event")),
            };
        }
        append(&mut witness.memory_events, memory_event, meter)?;
    }
    Ok(witness)
}
