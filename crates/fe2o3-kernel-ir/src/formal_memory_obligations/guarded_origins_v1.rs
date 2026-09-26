use super::*;
use std::ops::Range;

#[derive(Clone)]
pub(super) struct Input {
    pub(super) value: ValueId,
    pub(super) incoming: Range<usize>,
}

#[derive(Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
struct Dependency {
    node: usize,
    dependency: usize,
}

fn fill<T: Clone, M: GuardMeter>(
    meter: &mut M,
    count: usize,
    value: T,
) -> Result<Vec<T>, ResourceError> {
    let mut rows = Vec::new();
    meter.reserve(&mut rows, count)?;
    meter.charge(count)?;
    rows.resize(count, value);
    Ok(rows)
}

fn ranges<M: GuardMeter>(
    meter: &mut M,
    count: usize,
    edges: &[Dependency],
) -> Result<Vec<Range<usize>>, ResourceError> {
    let mut rows = fill(meter, count, 0..0)?;
    for (ordinal, edge) in edges.iter().enumerate() {
        meter.charge(4)?;
        let row = rows.get_mut(edge.node).ok_or(ResourceError::Accounting)?;
        if Range::is_empty(row) {
            row.start = ordinal;
        }
        row.end = ordinal.checked_add(1).ok_or(ResourceError::Arithmetic)?;
    }
    Ok(rows)
}

// The same two-pass SCC and condensation propagation as the original engine.
// Flat sorted edges replace nested maps/sets so every live backing is prepaid.
pub(super) fn resolve<M: GuardMeter>(
    meter: &mut M,
    inputs: &[Input],
    incoming: &[ValueId],
) -> Result<Vec<Option<ValueId>>, ResourceError> {
    // Twenty fixed Vec headers coexist along the SCC/condensation path. Legacy
    // metering remains a no-op here; the live path prepays its new frames.
    meter.storage(
        20_usize
            .checked_mul(size_of::<Vec<()>>())
            .ok_or(ResourceError::Arithmetic)?,
    )?;
    let count = inputs.len();
    meter.charge(count)?;
    if inputs.windows(2).any(|pair| pair[0].value >= pair[1].value) {
        return Err(ResourceError::Accounting);
    }
    let mut edges = Vec::new();
    meter.reserve(&mut edges, incoming.len())?;
    let mut local = fill(meter, count, OriginSummary::Empty)?;
    let mut invalid = fill(meter, count, false)?;
    for (node, input) in inputs.iter().enumerate() {
        meter.charge(4)?;
        let sources = incoming
            .get(input.incoming.clone())
            .ok_or(ResourceError::Accounting)?;
        invalid[node] = sources.is_empty();
        for source in sources {
            meter.charge(2)?;
            if let Some(dependency) = meter.find(inputs, |row| row.value.cmp(source))? {
                edges.push(Dependency { node, dependency });
            } else {
                local[node].include(*source);
            }
        }
    }
    meter.sort(&mut edges, 2, |a, b| a.cmp(b))?;
    meter.charge(edges.len())?;
    edges.dedup();
    let forward = ranges(meter, count, &edges)?;
    let mut reverse_edges = Vec::new();
    meter.reserve(&mut reverse_edges, edges.len())?;
    for edge in &edges {
        meter.charge(2)?;
        reverse_edges.push(Dependency {
            node: edge.dependency,
            dependency: edge.node,
        });
    }
    meter.sort(&mut reverse_edges, 2, |a, b| a.cmp(b))?;
    let reverse = ranges(meter, count, &reverse_edges)?;
    let mut visited = fill(meter, count, false)?;
    let mut postorder = Vec::new();
    let mut stack = Vec::new();
    meter.reserve(&mut postorder, count)?;
    meter.reserve(
        &mut stack,
        count.checked_add(1).ok_or(ResourceError::Arithmetic)?,
    )?;
    for start in 0..count {
        meter.charge(2)?;
        if visited[start] {
            continue;
        }
        visited[start] = true;
        stack.push((start, forward[start].start));
        while let Some((node, next)) = stack.pop() {
            meter.charge(5)?;
            if next < forward[node].end {
                let dependency = edges[next].dependency;
                stack.push((node, next.checked_add(1).ok_or(ResourceError::Arithmetic)?));
                if !visited[dependency] {
                    visited[dependency] = true;
                    stack.push((dependency, forward[dependency].start));
                }
            } else {
                postorder.push(node);
            }
        }
    }
    let mut component_of = fill(meter, count, usize::MAX)?;
    let mut component_count = 0_usize;
    let mut pending = Vec::new();
    meter.reserve(&mut pending, count)?;
    for &start in postorder.iter().rev() {
        meter.charge(2)?;
        if component_of[start] != usize::MAX {
            continue;
        }
        component_of[start] = component_count;
        pending.push(start);
        while let Some(node) = pending.pop() {
            meter.charge(3)?;
            for edge in &reverse_edges[reverse[node].clone()] {
                meter.charge(4)?;
                if component_of[edge.dependency] == usize::MAX {
                    component_of[edge.dependency] = component_count;
                    pending.push(edge.dependency);
                }
            }
        }
        component_count = component_count
            .checked_add(1)
            .ok_or(ResourceError::Arithmetic)?;
    }
    let mut component_origins = fill(meter, component_count, OriginSummary::Empty)?;
    let mut component_invalid = fill(meter, component_count, false)?;
    let mut dependencies = Vec::new();
    meter.reserve(&mut dependencies, edges.len())?;
    for node in 0..count {
        meter.charge(6)?;
        let component = component_of[node];
        component_invalid[component] |= invalid[node];
        match local[node] {
            OriginSummary::One(origin) => component_origins[component].include(origin),
            OriginSummary::Ambiguous => component_invalid[component] = true,
            OriginSummary::Empty => {}
        }
        for edge in &edges[forward[node].clone()] {
            meter.charge(3)?;
            let dependency = component_of[edge.dependency];
            if component != dependency {
                dependencies.push(Dependency {
                    node: component,
                    dependency,
                });
            }
        }
    }
    meter.sort(&mut dependencies, 2, |a, b| a.cmp(b))?;
    meter.charge(dependencies.len())?;
    dependencies.dedup();
    let dependency_ranges = ranges(meter, component_count, &dependencies)?;
    let mut dependents = Vec::new();
    meter.reserve(&mut dependents, dependencies.len())?;
    for edge in &dependencies {
        meter.charge(2)?;
        dependents.push(Dependency {
            node: edge.dependency,
            dependency: edge.node,
        });
    }
    meter.sort(&mut dependents, 2, |a, b| a.cmp(b))?;
    let dependent_ranges = ranges(meter, component_count, &dependents)?;
    let mut remaining = fill(meter, component_count, 0_usize)?;
    let mut results = fill(meter, component_count, None)?;
    for component in 0..component_count {
        meter.charge(3)?;
        remaining[component] = dependency_ranges[component].len();
        if remaining[component] == 0 {
            pending.push(component);
        }
    }
    while let Some(component) = pending.pop() {
        meter.charge(4)?;
        let mut summary = component_origins[component];
        let mut failed = component_invalid[component];
        for edge in &dependencies[dependency_ranges[component].clone()] {
            meter.charge(3)?;
            match results[edge.dependency] {
                Some(origin) => summary.include(origin),
                None => failed = true,
            }
        }
        results[component] = match (failed, summary) {
            (false, OriginSummary::One(origin)) => Some(origin),
            _ => None,
        };
        for edge in &dependents[dependent_ranges[component].clone()] {
            meter.charge(4)?;
            remaining[edge.dependency] = remaining[edge.dependency]
                .checked_sub(1)
                .ok_or(ResourceError::Accounting)?;
            if remaining[edge.dependency] == 0 {
                pending.push(edge.dependency);
            }
        }
    }
    let mut output = Vec::new();
    meter.reserve(&mut output, count)?;
    for component in component_of {
        meter.charge(2)?;
        output.push(results[component]);
    }
    Ok(output)
}

// Compatibility adapter: the old entry did not impose the guarded ledger on
// origin construction. Keep that admission boundary while sharing its algorithm.
struct LegacyMeter;
impl GuardMeter for LegacyMeter {
    fn charge(&mut self, _: usize) -> Result<(), ResourceError> {
        Ok(())
    }
    fn storage(&mut self, _: usize) -> Result<(), ResourceError> {
        Ok(())
    }
    fn reserve<T>(&mut self, rows: &mut Vec<T>, count: usize) -> Result<(), ResourceError> {
        if rows.capacity() < count {
            rows.try_reserve_exact(
                count
                    .checked_sub(rows.len())
                    .ok_or(ResourceError::Accounting)?,
            )
            .map_err(|_| ResourceError::Allocation)?;
        }
        Ok(())
    }
    fn sort<T>(
        &mut self,
        rows: &mut [T],
        _: usize,
        compare: impl FnMut(&T, &T) -> std::cmp::Ordering,
    ) -> Result<(), ResourceError> {
        rows.sort_unstable_by(compare);
        Ok(())
    }
    fn find_width<T>(
        &mut self,
        rows: &[T],
        _: usize,
        compare: impl FnMut(&T) -> std::cmp::Ordering,
    ) -> Result<Option<usize>, ResourceError> {
        Ok(rows.binary_search_by(compare).ok())
    }
}

pub(in super::super) fn legacy(
    inputs: &BTreeMap<ValueId, Vec<ValueId>>,
) -> BTreeMap<ValueId, Option<ValueId>> {
    let mut rows = Vec::with_capacity(inputs.len());
    let mut incoming = Vec::new();
    for (&value, values) in inputs {
        let start = incoming.len();
        incoming.extend_from_slice(values);
        rows.push(Input {
            value,
            incoming: start..incoming.len(),
        });
    }
    let output = resolve(&mut LegacyMeter, &rows, &incoming)
        .expect("verified unique-origin graph is bounded and consistent");
    rows.into_iter()
        .zip(output)
        .map(|(row, origin)| (row.value, origin))
        .collect()
}

// Structural observations only. The enclosing immutable CFG scope owns every
// temporary and returned credit; no source or SSA-validity authority is added.
pub(crate) fn structural_origins_v1(
    function: &Function,
    flow: &IndexedControlFlow,
    budget: &mut Budget<'_>,
) -> Result<Vec<(ValueId, Option<ValueId>)>, VerificationResourceError> {
    use super::meter::LiveGuardMeter;
    let mut meter = LiveGuardMeter::new(budget, usize::MAX, usize::MAX, usize::MAX);
    meter.storage(
        size_of::<LiveGuardMeter<'_, '_>>()
            .checked_add(4 * size_of::<Vec<()>>())
            .and_then(|n| n.checked_add(size_of::<Result<Vec<(ValueId, Option<ValueId>)>, VerificationResourceError>>()))
            .and_then(|n| n.checked_add(size_of::<Result<Vec<Option<ValueId>>, ResourceError>>()))
            .ok_or(ResourceError::Arithmetic)?,
    )?;
    let body = function.body.as_ref().ok_or(ResourceError::Accounting)?;
    let mut count = 0usize;
    for block in &body.blocks {
        meter.charge(1)?;
        count = count.checked_add(block.parameters.len()).ok_or(ResourceError::Arithmetic)?;
    }
    let mut inputs = Vec::new();
    let mut incoming = Vec::new();
    meter.reserve(&mut inputs, count)?;
    meter.reserve(&mut incoming, flow.phi_input_count())?;
    for block in &body.blocks {
        meter.charge(flow.block_count().checked_ilog2().unwrap_or(0) as usize + 3)?;
        let edges = flow.incoming_edges(block.id).ok_or(ResourceError::Accounting)?;
        for (ordinal, parameter) in block.parameters.iter().enumerate() {
            meter.charge(2)?;
            let start = incoming.len();
            for &edge in edges {
                meter.charge(4)?;
                let arguments = flow.edge_arguments(function, edge);
                if arguments.len() != block.parameters.len() {
                    return Err(VerificationResourceError::Accounting);
                }
                // Include every edge occurrence, even disconnected ones. This
                // is conservative and does not coalesce duplicate successors.
                incoming.push(arguments[ordinal]);
            }
            inputs.push(Input { value: parameter.id, incoming: start..incoming.len() });
        }
        // Zero-parameter destinations must not hide extra edge arguments.
        if block.parameters.is_empty() {
            for &edge in edges {
                meter.charge(2)?;
                if !flow.edge_arguments(function, edge).is_empty() {
                    return Err(VerificationResourceError::Accounting);
                }
            }
        }
    }
    meter.sort(&mut inputs, 1, |a, b| a.value.cmp(&b.value))?;
    let origins = resolve(&mut meter, &inputs, &incoming)?;
    let mut output = Vec::new();
    meter.reserve(&mut output, inputs.len())?;
    for (input, origin) in inputs.into_iter().zip(origins) {
        meter.charge(1)?;
        output.push((input.value, origin));
    }
    Ok(output)
}
