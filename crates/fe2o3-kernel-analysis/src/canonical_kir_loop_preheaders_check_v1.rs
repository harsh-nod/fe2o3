use super::*;

struct FunctionState {
    last_block: Option<u32>,
    last_value: Option<u32>,
    added: usize,
}

pub(super) fn check<'a>(
    input: &'a Owner,
    output: &'a Owner,
    rows: &'a [Row],
    limits: Limits,
    meter: &mut Meter<'_, '_>,
) -> Result<(CheckedCanonicalKirLoopPreheadersV1<'a>, Storage)> {
    type Checked<'a> = CheckedCanonicalKirLoopPreheadersV1<'a>;
    meter.reserve(size_of::<Checked<'_>>())?;
    let (a, a_size) = meter.derive(|b| Ok(Inventory::derive(input, b)?))?;
    meter.reserve(a_size.retained_storage())?;
    let (b, b_size) = meter.derive(|b| Ok(Inventory::derive(output, b)?))?;
    meter.reserve(b_size.retained_storage())?;
    let (loops, loops_size) = meter.derive(|b| Ok(Loops::derive(&a, limits, b)?))?;
    meter.reserve(loops_size.retained_storage())?;
    meter.derive(|b| Ok(loops.replay(&a, limits, b)?))?;
    meter.work(1)?;
    if rows.len() > loops.loop_count() {
        return Err(Error::Mismatch("bounded row roster"));
    }
    // Disjoint deep equality visits below are covered once by both wire sizes.
    meter.work(
        input
            .canonical()
            .canonical_bytes()
            .len()
            .checked_add(output.canonical().canonical_bytes().len())
            .ok_or(Resource::Arithmetic)?,
    )?;
    module_headers(input.module(), output.module())?;
    let (mut states, states_size) = meter.table(a.functions().len())?;
    for function in a.functions() {
        let mut state = FunctionState {
            last_block: None,
            last_value: None,
            added: 0,
        };
        for block in &a.blocks()[function.blocks.clone()] {
            meter.work(2)?;
            state.last_block = Some(
                state
                    .last_block
                    .map_or(block.block.id.0, |old| old.max(block.block.id.0)),
            );
        }
        for definition in &a.definitions()[function.definitions.clone()] {
            meter.work(2)?;
            if let Some(value) = definition.value {
                state.last_value = Some(state.last_value.map_or(value.0, |old| old.max(value.0)));
            }
        }
        meter.push(&mut states, state)?;
    }
    let (mut redirects, redirects_size) = meter.table::<Option<BlockId>>(a.edges().len())?;
    for _ in a.edges() {
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
        let header = fact.header();
        let function = &a.functions()[header.function.0 as usize];
        let state = &mut states[header.function.0 as usize];
        let appended = function
            .blocks
            .len()
            .checked_add(state.added)
            .ok_or(Resource::Arithmetic)?;
        let expected = Row {
            header,
            preheader: Block {
                function: header.function,
                block: u32::try_from(appended).map_err(|_| Resource::Arithmetic)?,
            },
        };
        if rows.get(selected) != Some(&expected) {
            return Err(Error::Mismatch("complete ordered preheader roster"));
        }
        let old = block(&a, header, meter)?;
        let new = block(&b, expected.preheader, meter)?;
        let id = BlockId(next_id(&mut state.last_block)?);
        meter.work(5)?;
        if new.id != id
            || !new.operations.is_empty()
            || new.parameters.len() != old.parameters.len()
        {
            return Err(Error::Mismatch("empty appended preheader"));
        }
        let Some(Terminator::Branch { target, arguments }) = &new.terminator else {
            return Err(Error::Mismatch("unconditional preheader branch"));
        };
        if *target != old.id || arguments.len() != old.parameters.len() {
            return Err(Error::Mismatch("exact original header target/arity"));
        }
        for ((old, new), argument) in old.parameters.iter().zip(&new.parameters).zip(arguments) {
            meter.work(4)?;
            let value = ValueId(next_id(&mut state.last_value)?);
            if new.id != value || new.ty != old.ty || *argument != value {
                return Err(Error::Mismatch("fresh ordered typed forwarding parameters"));
            }
        }
        for edge in external {
            let at = edge_ordinal(&a, *edge, meter)?;
            if a.edges()[at].target != header || redirects[at].replace(id).is_some() {
                return Err(Error::Mismatch("unique exact external edge occurrence"));
            }
        }
        state.added = state.added.checked_add(1).ok_or(Resource::Arithmetic)?;
        selected = selected.checked_add(1).ok_or(Resource::Arithmetic)?;
    }
    if selected != rows.len() {
        return Err(Error::Mismatch("extra selected row"));
    }
    for (index, (old, new)) in a.functions().iter().zip(b.functions()).enumerate() {
        meter.work(3)?;
        if new.blocks.len()
            != old
                .blocks
                .len()
                .checked_add(states[index].added)
                .ok_or(Resource::Arithmetic)?
        {
            return Err(Error::Mismatch("exact appended block count"));
        }
        for at in old.blocks.clone() {
            let original = &a.blocks()[at];
            let final_block = block(&b, original.coordinate, meter)?;
            meter.work(3)?;
            if original.block.id != final_block.id
                || original.block.parameters != final_block.parameters
                || original.block.operations != final_block.operations
            {
                return Err(Error::Mismatch("unchanged original block payload"));
            }
            control(
                original.terminator,
                final_block
                    .terminator
                    .as_ref()
                    .ok_or(Error::Mismatch("terminator"))?,
            )?;
            let final_row = &b.blocks()[new.blocks.start + original.coordinate.block as usize];
            if original.edges.len() != final_row.edges.len() {
                return Err(Error::Mismatch("edge occurrence count"));
            }
            for (old_at, new_at) in original.edges.clone().zip(final_row.edges.clone()) {
                meter.work(5)?;
                let x = &a.edges()[old_at];
                let y = &b.edges()[new_at];
                if x.coordinate != y.coordinate
                    || x.arguments != y.arguments
                    || y.target_id != redirects[old_at].unwrap_or(x.target_id)
                {
                    return Err(Error::Mismatch("exact edge redirection/arguments"));
                }
            }
        }
    }
    if rows.is_empty() {
        meter.work(
            input
                .canonical()
                .canonical_bytes()
                .len()
                .checked_add(output.canonical().canonical_bytes().len())
                .ok_or(Resource::Arithmetic)?,
        )?;
        if input.canonical().canonical_bytes() != output.canonical().canonical_bytes() {
            return Err(Error::Mismatch("byte-exact no-op"));
        }
    }
    // New inner preheaders may join outer natural loops: never reuse old members.
    let (output_loops, output_loops_size) =
        meter.derive(|budget| Ok(Loops::derive(&b, limits, budget)?))?;
    meter.reserve(output_loops_size.retained_storage())?;
    meter.derive(|budget| Ok(output_loops.replay(&b, limits, budget)?))?;
    drop(output_loops);
    meter.release(output_loops_size.retained_storage())?;
    drop(redirects);
    meter.release(redirects_size)?;
    drop(states);
    meter.release(states_size)?;
    drop(loops);
    meter.release(loops_size.retained_storage())?;
    drop(b);
    meter.release(b_size.retained_storage())?;
    drop(a);
    meter.release(a_size.retained_storage())?;
    Ok((
        Checked {
            input,
            output,
            rows,
        },
        CanonicalKirLoopPreheadersStorageV1(size_of::<Checked<'_>>()),
    ))
}

fn next_id(last: &mut Option<u32>) -> Result<u32> {
    let value = match *last {
        None => 0,
        Some(value) => value.checked_add(1).ok_or(Resource::Arithmetic)?,
    };
    *last = Some(value);
    Ok(value)
}
fn block<'a>(
    inventory: &'a Inventory<'_>,
    coordinate: Block,
    meter: &mut Meter<'_, '_>,
) -> Result<&'a fe2o3_kernel_ir::BasicBlock> {
    meter.work(5)?;
    let function = inventory
        .functions()
        .get(coordinate.function.0 as usize)
        .ok_or(Error::Mismatch("function coordinate"))?;
    let at = function
        .blocks
        .start
        .checked_add(coordinate.block as usize)
        .filter(|at| *at < function.blocks.end)
        .ok_or(Error::Mismatch("block coordinate"))?;
    let row = &inventory.blocks()[at];
    if row.coordinate != coordinate {
        return Err(Error::Mismatch("exact block coordinate"));
    }
    Ok(row.block)
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
        .ok_or(Error::Mismatch("edge function"))?;
    let block = function
        .blocks
        .start
        .checked_add(coordinate.source.block as usize)
        .filter(|at| *at < function.blocks.end)
        .and_then(|at| inventory.blocks().get(at))
        .ok_or(Error::Mismatch("edge block"))?;
    let at = block
        .edges
        .start
        .checked_add(coordinate.successor as usize)
        .filter(|at| *at < block.edges.end)
        .ok_or(Error::Mismatch("edge occurrence"))?;
    if inventory.edges()[at].coordinate != coordinate {
        return Err(Error::Mismatch("exact edge coordinate"));
    }
    Ok(at)
}
fn module_headers(a: &Module, b: &Module) -> Result<()> {
    let Module {
        id,
        functions,
        kernels,
        required_capabilities,
    } = a;
    if id != &b.id
        || kernels != &b.kernels
        || required_capabilities != &b.required_capabilities
        || functions.len() != b.functions.len()
    {
        return Err(Error::Mismatch("module payload"));
    }
    for (a, b) in functions.iter().zip(&b.functions) {
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
        match (body, &b.body) {
            (None, None) => {}
            (Some(a), Some(b)) if a.parameters == b.parameters => {}
            _ => return Err(Error::Mismatch("body parameters/declaration")),
        }
    }
    Ok(())
}
fn control(a: &Terminator, b: &Terminator) -> Result<()> {
    let same = match (a, b) {
        (Terminator::Branch { .. }, Terminator::Branch { .. }) => true,
        (
            Terminator::ConditionalBranch { condition: a, .. },
            Terminator::ConditionalBranch { condition: b, .. },
        ) => a == b,
        (
            Terminator::Switch {
                selector: a,
                cases: x,
                ..
            },
            Terminator::Switch {
                selector: b,
                cases: y,
                ..
            },
        ) => a == b && x.len() == y.len() && x.iter().zip(y).all(|(x, y)| x.value == y.value),
        (
            Terminator::IntegerSwitch {
                selector: a,
                cases: x,
                ..
            },
            Terminator::IntegerSwitch {
                selector: b,
                cases: y,
                ..
            },
        ) => a == b && x.len() == y.len() && x.iter().zip(y).all(|(x, y)| x.value == y.value),
        (Terminator::Return { values: a }, Terminator::Return { values: b }) => a == b,
        (Terminator::Unreachable, Terminator::Unreachable) => true,
        _ => false,
    };
    if same {
        Ok(())
    } else {
        Err(Error::Mismatch("unchanged terminator control payload"))
    }
}
