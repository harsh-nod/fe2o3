use super::*;

struct State {
    count: usize,
    last_block: Option<u32>,
    last_value: Option<u32>,
}

pub(super) fn apply(
    inventory: &Inventory<'_>,
    loops: &Loops<'_, '_>,
    candidate: &mut Module,
    meter: &mut Meter<'_, '_>,
) -> Result<(Vec<Row>, usize)> {
    // Equality and eventual destruction are separate full candidate traversals.
    meter.work(
        inventory
            .owner()
            .canonical()
            .canonical_bytes()
            .len()
            .checked_mul(2)
            .ok_or(Resource::Arithmetic)?,
    )?;
    if !loops.belongs_to(inventory) || candidate != inventory.owner().module() {
        return Err(Error::Recipe("exact original candidate and loop owner"));
    }
    let (mut states, states_bytes) = meter.table(inventory.functions().len())?;
    for function in inventory.functions() {
        let mut state = State {
            count: 0,
            last_block: None,
            last_value: None,
        };
        for block in &inventory.blocks()[function.blocks.clone()] {
            meter.work(2)?;
            state.last_block = Some(
                state
                    .last_block
                    .map_or(block.block.id.0, |old| old.max(block.block.id.0)),
            );
        }
        for definition in &inventory.definitions()[function.definitions.clone()] {
            meter.work(2)?;
            if let Some(value) = definition.value {
                state.last_value = Some(state.last_value.map_or(value.0, |old| old.max(value.0)));
            }
        }
        meter.push(&mut states, state)?;
    }
    let (mut rows, _) = meter.table(loops.loop_count())?;
    for index in 0..loops.loop_count() {
        let fact = meter.derive(|b| Ok(loops.natural_loop(index, b)?))?;
        let external = meter.derive(|b| Ok(loops.external_header_edges(index, b)?))?;
        meter.work(4)?;
        if fact.header().block == 0
            || !fact.is_single_entry()
            || fact.unconditional_preheader().is_some()
            || external.is_empty()
        {
            continue;
        }
        let header = fact.header();
        let f = header.function.0 as usize;
        let appended = inventory.functions()[f]
            .blocks
            .len()
            .checked_add(states[f].count)
            .ok_or(Resource::Arithmetic)?;
        meter.push(
            &mut rows,
            Row {
                header,
                preheader: Block {
                    function: header.function,
                    block: u32::try_from(appended).map_err(|_| Resource::Arithmetic)?,
                },
            },
        )?;
        states[f].count = states[f].count.checked_add(1).ok_or(Resource::Arithmetic)?;
    }
    let mut added_bytes = 0usize;
    for (function, state) in candidate.functions.iter_mut().zip(&states) {
        meter.work(1)?;
        if state.count == 0 {
            continue;
        }
        let body = function
            .body
            .as_mut()
            .ok_or(Error::Recipe("defined selected function"))?;
        let capacity = body
            .blocks
            .len()
            .checked_add(state.count)
            .ok_or(Resource::Arithmetic)?;
        // Pay a complete replacement before moving any old blocks. The original
        // conservative copy receipt remains reserved until candidate destruction.
        let (mut replacement, bytes) = meter.table(capacity)?;
        added_bytes = added_bytes.checked_add(bytes).ok_or(Resource::Arithmetic)?;
        meter.work(
            body.blocks
                .len()
                .checked_mul(size_of::<BasicBlock>())
                .ok_or(Resource::Arithmetic)?,
        )?;
        for block in std::mem::take(&mut body.blocks) {
            meter.push(&mut replacement, block)?;
        }
        body.blocks = replacement;
    }
    let (mut redirects, redirects_bytes) =
        meter.table::<Option<BlockId>>(inventory.edges().len())?;
    for _ in inventory.edges() {
        meter.push(&mut redirects, None)?;
    }
    let mut selected = 0usize;
    for index in 0..loops.loop_count() {
        let fact = meter.derive(|b| Ok(loops.natural_loop(index, b)?))?;
        let external = meter.derive(|b| Ok(loops.external_header_edges(index, b)?))?;
        meter.work(4)?;
        if fact.header().block == 0
            || !fact.is_single_entry()
            || fact.unconditional_preheader().is_some()
            || external.is_empty()
        {
            continue;
        }
        let row = rows.get(selected).ok_or(Error::Recipe("prepared row"))?;
        let f = row.header.function.0 as usize;
        let source =
            &inventory.blocks()[inventory.functions()[f].blocks.start + row.header.block as usize];
        let state = &mut states[f];
        let id = BlockId(next_id(&mut state.last_block)?);
        let (mut parameters, pbytes) = meter.table(source.block.parameters.len())?;
        let (mut arguments, abytes) = meter.table(source.block.parameters.len())?;
        added_bytes = added_bytes
            .checked_add(pbytes)
            .and_then(|n| n.checked_add(abytes))
            .ok_or(Resource::Arithmetic)?;
        for parameter in &source.block.parameters {
            meter.work(2)?;
            let id = ValueId(next_id(&mut state.last_value)?);
            let (ty, boxes) = clone_type(&parameter.ty, meter)?;
            added_bytes = added_bytes.checked_add(boxes).ok_or(Resource::Arithmetic)?;
            meter.push(&mut parameters, ValueDef::new(id, ty))?;
            meter.push(&mut arguments, id)?;
        }
        let mut preheader = BasicBlock::new(id);
        preheader.parameters = parameters;
        preheader.terminator = Some(Terminator::Branch {
            target: source.block.id,
            arguments,
        });
        let body = candidate.functions[f]
            .body
            .as_mut()
            .ok_or(Error::Recipe("selected body"))?;
        if body.blocks.len() != row.preheader.block as usize {
            return Err(Error::Recipe("ordered append coordinate"));
        }
        meter.push(&mut body.blocks, preheader)?;
        for edge in external {
            let at = edge_ordinal(inventory, *edge, meter)?;
            if inventory.edges()[at].target != row.header || redirects[at].replace(id).is_some() {
                return Err(Error::Recipe("unique external occurrence"));
            }
        }
        selected = selected.checked_add(1).ok_or(Resource::Arithmetic)?;
    }
    if selected != rows.len() {
        return Err(Error::Recipe("complete selected rows"));
    }
    for original in inventory.blocks() {
        meter.work(3)?;
        let body = candidate.functions[original.coordinate.function.0 as usize]
            .body
            .as_mut()
            .ok_or(Error::Recipe("original defined body"))?;
        let terminator = body.blocks[original.coordinate.block as usize]
            .terminator
            .as_mut()
            .ok_or(Error::Recipe("original terminator"))?;
        let mut cursor = original.edges.start;
        redirect(terminator, &redirects, &mut cursor, meter)?;
        if cursor != original.edges.end {
            return Err(Error::Recipe("exact edge traversal"));
        }
    }
    drop(redirects);
    meter.release(redirects_bytes)?;
    drop(states);
    meter.release(states_bytes)?;
    Ok((rows, added_bytes))
}

fn next_id(last: &mut Option<u32>) -> Result<u32> {
    let value = match *last {
        None => 0,
        Some(value) => value.checked_add(1).ok_or(Resource::Arithmetic)?,
    };
    *last = Some(value);
    Ok(value)
}
fn clone_type(ty: &Type, meter: &mut Meter<'_, '_>) -> Result<(Type, usize)> {
    let mut current = ty;
    let mut bytes = 0usize;
    loop {
        // Prepay cloning and eventual destruction of each fixed-size wrapper.
        meter.work(
            size_of::<Type>()
                .checked_mul(2)
                .ok_or(Resource::Arithmetic)?,
        )?;
        current = match current {
            Type::Pointer(pointer) => &pointer.pointee,
            Type::Slice(slice) => &slice.element,
            Type::Unit | Type::Scalar(_) | Type::Execution(_) | Type::Vector(_) => break,
        };
        meter.reserve(size_of::<Type>())?;
        bytes = bytes
            .checked_add(size_of::<Type>())
            .ok_or(Resource::Arithmetic)?;
    }
    Ok((ty.clone(), bytes))
}
fn edge_ordinal(
    inventory: &Inventory<'_>,
    coordinate: Edge,
    meter: &mut Meter<'_, '_>,
) -> Result<usize> {
    meter.work(6)?;
    let function = inventory
        .functions()
        .get(coordinate.source.function.0 as usize)
        .ok_or(Error::Recipe("edge function"))?;
    let block = function
        .blocks
        .start
        .checked_add(coordinate.source.block as usize)
        .filter(|at| *at < function.blocks.end)
        .and_then(|at| inventory.blocks().get(at))
        .ok_or(Error::Recipe("edge block"))?;
    let at = block
        .edges
        .start
        .checked_add(coordinate.successor as usize)
        .filter(|at| *at < block.edges.end)
        .ok_or(Error::Recipe("edge occurrence"))?;
    if inventory.edges()[at].coordinate != coordinate {
        return Err(Error::Recipe("exact edge coordinate"));
    }
    Ok(at)
}
fn redirect(
    terminator: &mut Terminator,
    targets: &[Option<BlockId>],
    cursor: &mut usize,
    meter: &mut Meter<'_, '_>,
) -> Result<()> {
    let mut visit = |target: &mut BlockId| -> Result<()> {
        meter.work(3)?;
        if let Some(replacement) = targets.get(*cursor).ok_or(Error::Recipe("edge map"))? {
            *target = *replacement;
        }
        *cursor = cursor.checked_add(1).ok_or(Resource::Arithmetic)?;
        Ok(())
    };
    match terminator {
        Terminator::Branch { target, .. } => visit(target)?,
        Terminator::ConditionalBranch {
            then_target,
            else_target,
            ..
        } => {
            visit(then_target)?;
            visit(else_target)?;
        }
        Terminator::Switch {
            cases,
            default_target,
            ..
        } => {
            for case in cases {
                visit(&mut case.target)?;
            }
            visit(default_target)?;
        }
        Terminator::IntegerSwitch {
            cases,
            default_target,
            ..
        } => {
            for case in cases {
                visit(&mut case.target)?;
            }
            visit(default_target)?;
        }
        Terminator::Return { .. } | Terminator::Unreachable => {}
    }
    Ok(())
}
