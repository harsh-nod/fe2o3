//! Scalar segment equations over complete source bodies and actual KIR paths.
//! Suspended caller values are explicit invariant components at every callee cut.
use super::super::super::{
    ExpressionV30, NodeV30, ScalarV30, canonical,
    control::{Branch, SourceBlock},
    relation,
    target_trace::ConcreteTrace,
};
use super::*;
use fe2o3_kernel_ir::{
    CanonicalKirBlockCoordinateV1 as Block, CanonicalKirDefinitionCoordinateV1 as Definition,
};

pub(super) fn suspended(
    original: &InvocationBodies<'_, '_, '_, '_>,
    bodies: &[Option<ActualBody>],
    index: usize,
    out: &mut Writer<'_, '_>,
) -> Result<Vec<Suspended>> {
    out.budget.reserve_storage(headers())?;
    let body = original.bodies[index].as_ref().ok_or_else(mismatch)?;
    let Some(transfer) = body.returned else {
        return vector(0, out);
    };
    let transfer = original.transfer(transfer, out)?;
    let plan = original.transfers.plan(out)?;
    let (parent, site) = plan
        .instance(body.root, body.instance, out)?
        .incoming
        .ok_or_else(mismatch)?;
    let parent = original.roots[body.root]
        .start
        .checked_add(parent)
        .ok_or(Resource::Arithmetic)?;
    let caller = original
        .bodies
        .get(parent)
        .and_then(Option::as_ref)
        .ok_or_else(mismatch)?;
    let actual = bodies
        .get(parent)
        .and_then(Option::as_ref)
        .ok_or_else(mismatch)?;
    let site = site.index() as usize;
    let source = caller
        .control
        .blocks
        .get(site)
        .and_then(Option::as_ref)
        .ok_or_else(mismatch)?;
    let binding = actual
        .bindings
        .get(site)
        .and_then(Option::as_ref)
        .ok_or_else(mismatch)?;
    let next = transfer
        .returned
        .continuation
        .checked_sub(caller.blocks.start)
        .ok_or_else(mismatch)?;
    let continuation = caller
        .control
        .blocks
        .get(next)
        .and_then(Option::as_ref)
        .ok_or_else(mismatch)?;
    let capacity = actual
        .suspended
        .len()
        .checked_add(continuation.live.len())
        .ok_or(Resource::Arithmetic)?;
    let mut result = vector(capacity, out)?;
    for &row in &actual.suspended {
        out.budget.charge_work(1)?;
        result.push(row);
    }
    let mut definitions = vector(caller.control.locals, out)?;
    out.budget.charge_work(caller.control.locals)?;
    definitions.resize(caller.control.locals, None);
    for &(local, definition) in &binding.live {
        out.budget.charge_work(2)?;
        *definitions.get_mut(local as usize).ok_or_else(mismatch)? = Some(definition);
    }
    if source.program.assignments.len() != binding.assignments.len() {
        return Err(mismatch());
    }
    for (assignment, &definition) in source.program.assignments.iter().zip(&binding.assignments) {
        out.budget.charge_work(2)?;
        *definitions
            .get_mut(assignment.destination as usize)
            .ok_or_else(mismatch)? = Some(definition);
    }
    for live in &continuation.live {
        out.budget.charge_work(4)?;
        let local = caller
            .locals
            .start
            .checked_add(live.local as usize)
            .ok_or(Resource::Arithmetic)?;
        if local == transfer.returned.destination {
            continue;
        }
        if source
            .program
            .locals
            .get(live.local as usize)
            .copied()
            .flatten()
            .is_none()
        {
            return Err(mismatch());
        }
        let definition = definitions
            .get(live.local as usize)
            .copied()
            .flatten()
            .ok_or_else(mismatch)?;
        let scalar = *caller
            .control
            .types
            .get(live.local as usize)
            .ok_or_else(mismatch)?;
        if definition.is_none() && scalar != ScalarV30::Unit {
            return Err(mismatch());
        }
        result.push(Suspended {
            local,
            definition,
            scalar,
        });
    }
    let log = usize::BITS as usize - result.len().max(1).leading_zeros() as usize;
    out.budget.charge_work(
        result
            .len()
            .checked_mul(log + 2)
            .ok_or(Resource::Arithmetic)?,
    )?;
    result.sort_unstable_by_key(|row| row.local);
    if result.windows(2).any(|rows| rows[0].local == rows[1].local) {
        return Err(mismatch());
    }
    Ok(result)
}

pub(super) fn check(
    model: &ActualInvocations<'_, '_, '_, '_, '_, '_>,
    out: &mut Writer<'_, '_>,
) -> Result<()> {
    replay(model, false, out)
}

pub(super) fn emit(
    model: &ActualInvocations<'_, '_, '_, '_, '_, '_>,
    out: &mut Writer<'_, '_>,
) -> Result<()> {
    replay(model, true, out)
}

fn replay(
    model: &ActualInvocations<'_, '_, '_, '_, '_, '_>,
    emit: bool,
    out: &mut Writer<'_, '_>,
) -> Result<()> {
    out.budget.reserve_storage(headers())?;
    for (root, actual) in model.roots.iter().enumerate() {
        let mut covered = vector(actual.targets.len(), out)?;
        out.budget.charge_work(actual.targets.len())?;
        covered.resize(actual.targets.len(), false);
        entry(model, root, &mut covered, emit, out)?;
        for index in model.original.roots[root].clone() {
            out.budget.charge_work(1)?;
            let Some(body) = model.original.bodies[index].as_ref() else {
                continue;
            };
            for (block, source) in body.control.blocks.iter().enumerate() {
                out.budget.charge_work(1)?;
                let Some(source) = source else {
                    continue;
                };
                let edges = match &source.branch {
                    Branch::Switch { cases, .. } => {
                        cases.len().checked_add(1).ok_or(Resource::Arithmetic)?
                    }
                    _ => 1,
                };
                for edge in 0..edges {
                    segment(model, index, block, edge, &mut covered, emit, out)?;
                }
            }
        }
        out.budget.charge_work(covered.len())?;
        if covered.iter().any(|seen| !seen) {
            return Err(mismatch());
        }
    }
    Ok(())
}

fn entry(
    model: &ActualInvocations<'_, '_, '_, '_, '_, '_>,
    root: usize,
    covered: &mut [bool],
    emit: bool,
    out: &mut Writer<'_, '_>,
) -> Result<()> {
    let actual = &model.roots[root];
    let index = model.original.roots[root].start;
    let body = model.original.bodies[index].as_ref().ok_or_else(mismatch)?;
    let bindings = model.bodies[index].as_ref().ok_or_else(mismatch)?;
    let entry = bindings
        .bindings
        .get(body.control.entry.get() as usize)
        .and_then(Option::as_ref)
        .ok_or_else(mismatch)?;
    let function = &model.inventory.functions()[actual.physical];
    if function.function.signature.parameters.len() != body.control.arguments {
        return Err(mismatch());
    }
    let mut trace =
        ConcreteTrace::arguments(model.inventory, actual.physical, actual.capacity, out)?;
    let (end, path) = trace.connectors(
        Block {
            function: function.coordinate,
            block: 0,
        },
        &actual.targets,
        &actual.boundaries,
        out,
    )?;
    if end != entry.physical {
        return Err(mismatch());
    }
    cover(&path, covered, out)?;
    let classes = relation::classes([&trace.nodes, &[]], out)?;
    let mut arguments = vector(body.control.locals, out)?;
    out.budget.charge_work(body.control.locals)?;
    arguments.resize(body.control.locals, None);
    for &(local, argument) in &body.control.initial {
        out.budget.charge_work(2)?;
        *arguments.get_mut(local as usize).ok_or_else(mismatch)? = Some(argument as usize);
    }
    for &(local, definition) in bindings.inputs.iter().chain(&entry.live) {
        out.budget.charge_work(3)?;
        let expected = arguments
            .get(local as usize)
            .copied()
            .flatten()
            .ok_or_else(mismatch)?;
        let actual = trace.value(definition.ok_or_else(mismatch)?, out)?;
        if classes[0].get(expected).is_none() || classes[0].get(expected) != classes[0].get(actual)
        {
            return Err(mismatch());
        }
    }
    if emit {
        super::generate::entry(model, root, &trace, &path, out)?;
    }
    Ok(())
}

fn cover(path: &[usize], covered: &mut [bool], out: &mut Writer<'_, '_>) -> Result<()> {
    for &block in path {
        out.budget.charge_work(1)?;
        *covered.get_mut(block).ok_or_else(mismatch)? = true;
    }
    Ok(())
}

fn symbol(nodes: &mut Vec<NodeV30>, scalar: ScalarV30, definition: Option<usize>) -> Result<usize> {
    let expression = match definition {
        Some(definition) => {
            ExpressionV30::Argument(u32::try_from(definition).map_err(|_| Resource::Arithmetic)?)
        }
        None if scalar == ScalarV30::Unit => ExpressionV30::Constant(0),
        None => return Err(mismatch()),
    };
    if nodes.len() == nodes.capacity() {
        return Err(Resource::Accounting.into());
    }
    let index = nodes.len();
    nodes.push(NodeV30 { scalar, expression });
    Ok(index)
}

fn normalize(
    source: &SourceBlock,
    binding: &BlockBindings,
    model: &ActualInvocations<'_, '_, '_, '_, '_, '_>,
    capacity: usize,
    out: &mut Writer<'_, '_>,
) -> Result<Vec<NodeV30>> {
    if source.live.len() != binding.live.len()
        || source.program.assignments.len() != binding.assignments.len()
    {
        return Err(mismatch());
    }
    let mut definitions = vector(source.program.locals.len(), out)?;
    out.budget.charge_work(source.program.locals.len())?;
    definitions.resize(source.program.locals.len(), None);
    for (live, &(local, definition)) in source.live.iter().zip(&binding.live) {
        out.budget.charge_work(3)?;
        if live.local != local {
            return Err(mismatch());
        }
        if let Some(definition) = definition {
            let row = model
                .inventory
                .definitions()
                .get(definition)
                .ok_or_else(mismatch)?;
            if matches!(row.coordinate, Definition::Result { operation, .. } if operation.block == binding.physical)
            {
                return Err(mismatch());
            }
        }
        *definitions.get_mut(local as usize).ok_or_else(mismatch)? = Some(definition);
    }
    let mut nodes = vector(capacity, out)?;
    for &node in &source.program.nodes {
        out.budget.charge_work(4)?;
        let expression = match node.expression {
            ExpressionV30::Argument(local) => {
                let definition = definitions
                    .get(local as usize)
                    .copied()
                    .flatten()
                    .ok_or_else(mismatch)?;
                if let Some(definition) = definition {
                    let row = model
                        .inventory
                        .definitions()
                        .get(definition)
                        .ok_or_else(mismatch)?;
                    if canonical::scalar(row.ty)? != node.scalar {
                        return Err(mismatch());
                    }
                }
                symbol(&mut nodes, node.scalar, definition)?;
                continue;
            }
            other => other,
        };
        nodes.push(NodeV30 {
            scalar: node.scalar,
            expression,
        });
    }
    Ok(nodes)
}

fn call_arguments(
    source: &SourceBlock,
    transfer: &super::super::super::call_transfers::DirectTransfer,
    nodes: &mut Vec<NodeV30>,
    out: &mut Writer<'_, '_>,
) -> Result<Vec<(usize, usize)>> {
    let mut translated = vector(transfer.operands.nodes.len(), out)?;
    for &node in &transfer.operands.nodes {
        out.budget.charge_work(5)?;
        let edge = |at: usize| translated.get(at).copied().ok_or_else(mismatch);
        let expression = match node.expression {
            ExpressionV30::Argument(local) => {
                let value = source
                    .program
                    .locals
                    .get(local as usize)
                    .copied()
                    .flatten()
                    .ok_or_else(mismatch)?;
                if nodes.get(value).map(|row| row.scalar) != Some(node.scalar) {
                    return Err(mismatch());
                }
                translated.push(value);
                continue;
            }
            ExpressionV30::Constant(value) => ExpressionV30::Constant(value),
            ExpressionV30::Not(input) => ExpressionV30::Not(edge(input)?),
            ExpressionV30::Select {
                condition,
                true_value,
                false_value,
            } => {
                out.budget.charge_work(1)?;
                ExpressionV30::Select {
                    condition: edge(condition)?,
                    true_value: edge(true_value)?,
                    false_value: edge(false_value)?,
                }
            }
            ExpressionV30::Binary {
                operation,
                left,
                right,
            } => ExpressionV30::Binary {
                operation,
                left: edge(left)?,
                right: edge(right)?,
            },
        };
        if nodes.len() == nodes.capacity() {
            return Err(Resource::Accounting.into());
        }
        translated.push(nodes.len());
        nodes.push(NodeV30 {
            scalar: node.scalar,
            expression,
        });
    }
    let mut result = vector(transfer.arguments.len(), out)?;
    for &(local, expression) in &transfer.arguments {
        out.budget.charge_work(2)?;
        result.push((local, *translated.get(expression).ok_or_else(mismatch)?));
    }
    let log = usize::BITS as usize - result.len().max(1).leading_zeros() as usize;
    out.budget.charge_work(
        result
            .len()
            .checked_mul(log + 1)
            .ok_or(Resource::Arithmetic)?,
    )?;
    result.sort_unstable_by_key(|row| row.0);
    Ok(result)
}

#[allow(clippy::too_many_arguments)]
fn segment(
    model: &ActualInvocations<'_, '_, '_, '_, '_, '_>,
    index: usize,
    block: usize,
    ordinal: usize,
    covered: &mut [bool],
    emit: bool,
    out: &mut Writer<'_, '_>,
) -> Result<()> {
    let body = model.original.bodies[index].as_ref().ok_or_else(mismatch)?;
    let actual = model.bodies[index].as_ref().ok_or_else(mismatch)?;
    let root = &model.roots[body.root];
    let source = body.control.blocks[block].as_ref().ok_or_else(mismatch)?;
    let binding = actual.bindings[block].as_ref().ok_or_else(mismatch)?;
    let target = root
        .targets
        .get(binding.physical.block as usize)
        .ok_or_else(mismatch)?;
    if target.coordinate(model.inventory)? != binding.physical {
        return Err(mismatch());
    }
    let transfer = match source.branch {
        Branch::Call { transfer, .. } => Some(model.original.transfer(transfer, out)?),
        _ => None,
    };
    let extras = segment_extras(body, actual, &source.branch, model.original, out)?;
    let capacity = source
        .program
        .nodes
        .len()
        .checked_add(extras)
        .and_then(|n| n.checked_add(transfer.map_or(0, |row| row.operands.nodes.len())))
        .ok_or(Resource::Arithmetic)?;
    let mut nodes = normalize(source, binding, model, capacity, out)?;
    let mut observations = vector(
        source
            .program
            .assignments
            .len()
            .checked_add(extras)
            .ok_or(Resource::Arithmetic)?,
        out,
    )?;
    let mut trace = ConcreteTrace::boundary(model.inventory, root.physical, root.capacity, out)?;
    let appended = trace.append(target, out)?;
    *covered
        .get_mut(binding.physical.block as usize)
        .ok_or_else(mismatch)? = true;
    for (assignment, &definition) in source.program.assignments.iter().zip(&binding.assignments) {
        out.budget.charge_work(2)?;
        observations.push((
            assignment.value,
            definition
                .map(|definition| trace.value(definition, out))
                .transpose()?,
        ));
    }
    let arguments = transfer
        .map(|transfer| call_arguments(source, transfer, &mut nodes, out))
        .transpose()?;
    let (next_body, next_block, edge) = match (&source.branch, &target.branch) {
        (Branch::Goto(next), super::super::super::canonical::control::TargetBranch::Goto)
            if target.edges.len() == 1 =>
        {
            (index, next.get() as usize, 0)
        }
        (
            Branch::Switch {
                selector,
                cases,
                otherwise,
            },
            super::super::super::canonical::control::TargetBranch::Switch {
                selector: actual_selector,
                cases: actual_cases,
                otherwise: actual_otherwise,
            },
        ) => {
            out.budget.charge_work(cases.len())?;
            if cases.len() != actual_cases.len()
                || target.edges.len() != cases.len().checked_add(1).ok_or(Resource::Arithmetic)?
            {
                return Err(mismatch());
            }
            for ((value, _), (actual, edge)) in cases.iter().zip(actual_cases) {
                if value != actual || *edge >= target.edges.len() {
                    return Err(mismatch());
                }
            }
            observations.push((
                *selector,
                Some(trace.appended_value(&appended, *actual_selector, out)?),
            ));
            if ordinal < cases.len() {
                (
                    index,
                    cases[ordinal].1.get() as usize,
                    actual_cases[ordinal].1,
                )
            } else {
                (index, otherwise.get() as usize, *actual_otherwise)
            }
        }
        (Branch::Call { .. }, super::super::super::canonical::control::TargetBranch::Goto)
            if target.edges.len() == 1 =>
        {
            let call = transfer.ok_or_else(mismatch)?;
            let next = model.original.roots[body.root]
                .start
                .checked_add(call.child)
                .ok_or(Resource::Arithmetic)?;
            let callee = model
                .original
                .bodies
                .get(next)
                .and_then(Option::as_ref)
                .ok_or_else(mismatch)?;
            if call.entry
                != callee
                    .blocks
                    .start
                    .checked_add(callee.control.entry.get() as usize)
                    .ok_or(Resource::Arithmetic)?
            {
                return Err(mismatch());
            }
            (next, callee.control.entry.get() as usize, 0)
        }
        (Branch::Return, super::super::super::canonical::control::TargetBranch::Goto)
            if body.returned.is_some() && target.edges.len() == 1 =>
        {
            let call = model
                .original
                .transfer(body.returned.ok_or_else(mismatch)?, out)?;
            let plan = model.original.transfers.plan(out)?;
            let (parent, _) = plan
                .instance(body.root, body.instance, out)?
                .incoming
                .ok_or_else(mismatch)?;
            let next = model.original.roots[body.root]
                .start
                .checked_add(parent)
                .ok_or(Resource::Arithmetic)?;
            let caller = model
                .original
                .bodies
                .get(next)
                .and_then(Option::as_ref)
                .ok_or_else(mismatch)?;
            (
                next,
                call.returned
                    .continuation
                    .checked_sub(caller.blocks.start)
                    .ok_or_else(mismatch)?,
                0,
            )
        }
        (Branch::Return, super::super::super::canonical::control::TargetBranch::Return)
            if body.returned.is_none() && target.edges.is_empty() =>
        {
            match (source.program.returned, target.program.returned) {
                (Some(source), Some(target)) => {
                    observations.push((source, Some(trace.appended_value(&appended, target, out)?)))
                }
                (None, None) => (),
                _ => return Err(mismatch()),
            }
            preserve(
                &actual.suspended,
                &mut nodes,
                &mut observations,
                &trace,
                out,
            )?;
            equal(&nodes, &trace.nodes, &observations, out)?;
            if emit {
                let returned = target
                    .program
                    .returned
                    .map(|node| trace.appended_value(&appended, node, out))
                    .transpose()?;
                super::generate::segment(
                    model,
                    index,
                    block,
                    ordinal,
                    &trace,
                    &observations[..source.program.assignments.len()],
                    returned,
                    None,
                    &[],
                    out,
                )?;
            }
            return Ok(());
        }
        _ => return Err(mismatch()),
    };
    let at = trace.edge(appended, edge, out)?;
    let (end, path) = trace.connectors(at, &root.targets, &root.boundaries, out)?;
    cover(&path, covered, out)?;
    if root.cuts.get(end.block as usize) != Some(&Some((next_body, next_block))) {
        return Err(mismatch());
    }
    let next_source = model.original.bodies[next_body]
        .as_ref()
        .ok_or_else(mismatch)?;
    let next_actual = model.bodies[next_body].as_ref().ok_or_else(mismatch)?;
    let next = next_actual
        .bindings
        .get(next_block)
        .and_then(Option::as_ref)
        .ok_or_else(mismatch)?;
    if next.physical != end {
        return Err(mismatch());
    }
    preserve(
        &actual.suspended,
        &mut nodes,
        &mut observations,
        &trace,
        out,
    )?;
    if transfer.is_some() {
        let arguments = arguments.as_ref().ok_or_else(mismatch)?;
        for &(local, definition) in &next_actual.inputs {
            out.budget.charge_work(
                logarithm(arguments.len())
                    .checked_add(3)
                    .ok_or(Resource::Arithmetic)?,
            )?;
            let global = next_source
                .locals
                .start
                .checked_add(local as usize)
                .ok_or(Resource::Arithmetic)?;
            let at = arguments
                .binary_search_by_key(&global, |row| row.0)
                .map_err(|_| mismatch())?;
            observations.push((
                arguments[at].1,
                definition
                    .map(|definition| trace.value(definition, out))
                    .transpose()?,
            ));
        }
        for suspended in &next_actual.suspended {
            out.budget.charge_work(4)?;
            if body.locals.contains(&suspended.local) {
                let local = suspended
                    .local
                    .checked_sub(body.locals.start)
                    .ok_or_else(mismatch)?;
                let expected = source
                    .program
                    .locals
                    .get(local)
                    .copied()
                    .flatten()
                    .ok_or_else(mismatch)?;
                observations.push((
                    expected,
                    suspended
                        .definition
                        .map(|definition| trace.value(definition, out))
                        .transpose()?,
                ));
            }
        }
    }
    if matches!(source.branch, Branch::Return) {
        let definition = actual.returned.ok_or_else(mismatch)?;
        let expected = match source.program.returned {
            Some(value) => value,
            None if definition.is_none() => symbol(&mut nodes, ScalarV30::Unit, None)?,
            _ => return Err(mismatch()),
        };
        observations.push((
            expected,
            definition
                .map(|definition| trace.value(definition, out))
                .transpose()?,
        ));
    }
    for &(local, definition) in &next.live {
        out.budget.charge_work(5)?;
        let expected = match &source.branch {
            Branch::Call { .. } => {
                let arguments = arguments.as_ref().ok_or_else(mismatch)?;
                let global = next_source
                    .locals
                    .start
                    .checked_add(local as usize)
                    .ok_or(Resource::Arithmetic)?;
                out.budget.charge_work(logarithm(arguments.len()))?;
                let at = arguments
                    .binary_search_by_key(&global, |row| row.0)
                    .map_err(|_| mismatch())?;
                arguments[at].1
            }
            Branch::Return => {
                let call = model
                    .original
                    .transfer(body.returned.ok_or_else(mismatch)?, out)?;
                let global = next_source
                    .locals
                    .start
                    .checked_add(local as usize)
                    .ok_or(Resource::Arithmetic)?;
                if global == call.returned.destination {
                    match source.program.returned {
                        Some(value) => value,
                        None if call.returned.scalar == ScalarV30::Unit => {
                            symbol(&mut nodes, ScalarV30::Unit, None)?
                        }
                        _ => return Err(mismatch()),
                    }
                } else {
                    out.budget.charge_work(logarithm(actual.suspended.len()))?;
                    let at = actual
                        .suspended
                        .binary_search_by_key(&global, |row| row.local)
                        .map_err(|_| mismatch())?;
                    let row = actual.suspended[at];
                    symbol(&mut nodes, row.scalar, row.definition)?
                }
            }
            _ => source
                .program
                .locals
                .get(local as usize)
                .copied()
                .flatten()
                .ok_or_else(mismatch)?,
        };
        observations.push((
            expected,
            definition
                .map(|definition| trace.value(definition, out))
                .transpose()?,
        ));
    }
    equal(&nodes, &trace.nodes, &observations, out)?;
    if emit {
        let returned = if source.program.returned.is_some() {
            Some(trace.value(actual.returned.flatten().ok_or_else(mismatch)?, out)?)
        } else {
            None
        };
        super::generate::segment(
            model,
            index,
            block,
            ordinal,
            &trace,
            &observations[..source.program.assignments.len()],
            returned,
            Some((next_body, next_block)),
            &path,
            out,
        )?;
    }
    Ok(())
}

pub(super) fn segment_extras(
    body: &Body,
    actual: &ActualBody,
    branch: &Branch,
    original: &InvocationBodies<'_, '_, '_, '_>,
    out: &mut Writer<'_, '_>,
) -> Result<usize> {
    out.budget.charge_work(8)?;
    // Only the current frame, its adjacent call/return and suspended ancestors
    // can contribute observations or fresh symbols to this segment.
    let adjacent = match *branch {
        Branch::Call { transfer, .. } => original
            .transfer(transfer, out)?
            .child_locals
            .len()
            .checked_mul(2)
            .and_then(|n| n.checked_add(body.control.locals))
            .ok_or(Resource::Arithmetic)?,
        Branch::Return => match body.returned {
            Some(transfer) => original.transfer(transfer, out)?.caller_locals.len(),
            None => 0,
        },
        _ => body.control.locals,
    };
    actual
        .suspended
        .len()
        .checked_add(adjacent)
        .and_then(|n| n.checked_add(3))
        .ok_or_else(|| Resource::Arithmetic.into())
}

fn logarithm(count: usize) -> usize {
    (usize::BITS - count.max(1).leading_zeros()) as usize + 1
}

fn preserve(
    suspended: &[Suspended],
    nodes: &mut Vec<NodeV30>,
    observations: &mut Vec<(usize, Option<usize>)>,
    trace: &ConcreteTrace<'_, '_>,
    out: &mut Writer<'_, '_>,
) -> Result<()> {
    for row in suspended {
        out.budget.charge_work(3)?;
        observations.push((
            symbol(nodes, row.scalar, row.definition)?,
            row.definition
                .map(|definition| trace.value(definition, out))
                .transpose()?,
        ));
    }
    Ok(())
}

fn equal(
    source: &[NodeV30],
    target: &[NodeV30],
    observations: &[(usize, Option<usize>)],
    out: &mut Writer<'_, '_>,
) -> Result<()> {
    let classes = relation::classes([source, target], out)?;
    for &(source_node, target_node) in observations {
        out.budget.charge_work(3)?;
        match target_node {
            Some(target)
                if classes[0].get(source_node).is_some()
                    && classes[0].get(source_node) == classes[1].get(target) =>
            {
                ()
            }
            None if source.get(source_node).is_some_and(|node| {
                node.scalar == ScalarV30::Unit && node.expression == ExpressionV30::Constant(0)
            }) =>
            {
                ()
            }
            _ => return Err(mismatch()),
        }
    }
    Ok(())
}

pub(super) fn headers() -> usize {
    fn h<T>() -> usize {
        size_of::<T>() + 2 * size_of::<Result<T>>()
    }
    h::<&ActualInvocations<'_, '_, '_, '_, '_, '_>>()
        + h::<&InvocationBodies<'_, '_, '_, '_>>()
        + h::<&Body>()
        + h::<&ActualBody>()
        + h::<&ActualRoot>()
        + h::<&SourceBlock>()
        + h::<&BlockBindings>()
        + h::<&TargetBlock>()
        + h::<&super::super::super::call_transfers::DirectTransfer>()
        + h::<ConcreteTrace<'_, '_>>()
        + h::<Vec<Suspended>>()
        + h::<Vec<NodeV30>>()
        + h::<Vec<Option<Option<usize>>>>()
        + h::<Vec<Option<usize>>>()
        + h::<Vec<bool>>()
        + h::<Vec<(usize, usize)>>()
        + h::<Vec<usize>>()
        + h::<Vec<(usize, Option<usize>)>>()
        + h::<Option<Vec<(usize, usize)>>>()
        + h::<Option<&super::super::super::call_transfers::DirectTransfer>>()
        + h::<[Vec<usize>; 2]>()
        + h::<(usize, usize, usize)>()
        + h::<(Block, Vec<usize>)>()
        + h::<&mut Writer<'_, '_>>()
        + h::<NodeV30>()
        + h::<Suspended>()
        + h::<Block>()
        + 32 * size_of::<usize>()
        + 16 * size_of::<&()>()
}
