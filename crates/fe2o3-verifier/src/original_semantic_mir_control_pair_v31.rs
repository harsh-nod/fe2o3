//! Independent, complete block-step comparisons over exact source-owned locators.

use super::super::Inventory;
use super::{
    Error, ExpressionV30, NodeV30, Resource, Result, ScalarV30, Writer,
    canonical::control::{TargetBlock, TargetBranch},
    control::{Branch, SourceBlock, SourceControl},
    relation, vector,
};
use fe2o3_kernel_ir::CanonicalKirBlockCoordinateV1;
use std::mem::size_of;

/// None denotes the unique semantic Unit value, not an omitted assignment.
pub(super) struct BlockBindings {
    pub physical: CanonicalKirBlockCoordinateV1,
    pub live: Vec<(u32, Option<usize>)>,
    pub assignments: Vec<Option<usize>>,
}

pub(super) struct BlockPair {
    pub source_nodes: Vec<NodeV30>,
    pub source_observations: Vec<usize>,
    pub target_observations: Vec<Option<usize>>,
}

fn mismatch() -> Error {
    Error::Statement("original MIR concrete CFG step differs from actual canonical operations")
}

impl BlockPair {
    pub(super) fn check(
        source: &SourceControl,
        block: usize,
        bindings: &[Option<BlockBindings>],
        inventory: &Inventory<'_>,
        target: &mut TargetBlock,
        out: &mut Writer<'_, '_>,
    ) -> Result<Self> {
        out.budget.reserve_storage(headers())?;
        let original = source
            .blocks
            .get(block)
            .and_then(Option::as_ref)
            .ok_or_else(mismatch)?;
        let binding = bindings
            .get(block)
            .and_then(Option::as_ref)
            .ok_or_else(mismatch)?;
        if binding.assignments.len() != original.program.assignments.len()
            || binding.live.len() != original.live.len()
        {
            return Err(mismatch());
        }
        let mut local_definitions = vector(source.locals, out)?;
        out.budget.charge_work(source.locals)?;
        local_definitions.resize(source.locals, None);
        for (expected, &(local, definition)) in original.live.iter().zip(&binding.live) {
            out.budget.charge_work(3)?;
            if local != expected.local || local as usize >= source.locals {
                return Err(mismatch());
            }
            if let Some(definition) = definition {
                let row = inventory
                    .definitions()
                    .get(definition)
                    .ok_or_else(mismatch)?;
                if matches!(row.coordinate,
                    fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1::Result { operation, .. }
                        if operation.block == binding.physical)
                {
                    return Err(mismatch());
                }
            }
            local_definitions[local as usize] = Some(definition);
        }
        let mut nodes = vector(original.program.nodes.len(), out)?;
        for &node in &original.program.nodes {
            out.budget.charge_work(3)?;
            let expression = match node.expression {
                ExpressionV30::Argument(local) => {
                    match local_definitions
                        .get(local as usize)
                        .copied()
                        .flatten()
                        .ok_or_else(mismatch)?
                    {
                        Some(definition) => {
                            definition
                                .checked_sub(target.program.definition_start)
                                .filter(|slot| *slot < target.program.definitions.len())
                                .ok_or_else(mismatch)?;
                            if inventory
                                .definitions()
                                .get(definition)
                                .map(|row| super::canonical::scalar(row.ty))
                                .transpose()?
                                != Some(node.scalar)
                            {
                                return Err(mismatch());
                            }
                            ExpressionV30::Argument(
                                u32::try_from(definition).map_err(|_| Resource::Arithmetic)?,
                            )
                        }
                        None if node.scalar == ScalarV30::Unit => ExpressionV30::Constant(0),
                        None => return Err(mismatch()),
                    }
                }
                other => other,
            };
            nodes.push(NodeV30 {
                scalar: node.scalar,
                expression,
            });
        }
        let mut target_observations = vector(binding.assignments.len(), out)?;
        let mut source_observations = vector(binding.assignments.len(), out)?;
        for (assignment, &definition) in original
            .program
            .assignments
            .iter()
            .zip(&binding.assignments)
        {
            out.budget.charge_work(2)?;
            source_observations.push(assignment.value);
            target_observations.push(match definition {
                Some(definition) => Some(target.observe(inventory, definition, out)?),
                None => None,
            });
        }
        // Populate untouched definitions required by successor invariants before
        // fixing the target expression classes. Parallel edges remain separate.
        match &original.branch {
            Branch::Goto(next) => prepare_edge(
                source,
                bindings,
                next.get() as usize,
                inventory,
                target,
                out,
            )?,
            Branch::Switch {
                cases, otherwise, ..
            } => {
                for &(_, next) in cases {
                    prepare_edge(
                        source,
                        bindings,
                        next.get() as usize,
                        inventory,
                        target,
                        out,
                    )?;
                }
                prepare_edge(
                    source,
                    bindings,
                    otherwise.get() as usize,
                    inventory,
                    target,
                    out,
                )?;
            }
            Branch::Return => {}
            Branch::Call { .. } => {
                return Err(Error::Statement(
                    "original MIR actual call splice is not modeled",
                ));
            }
        }
        let classes = relation::classes([&nodes, &target.program.nodes], out)?;
        for (&source_node, &target_node) in source_observations.iter().zip(&target_observations) {
            equal(&nodes, &classes, source_node, target_node)?;
        }
        match (&original.branch, &target.branch) {
            (Branch::Return, TargetBranch::Return) => {
                match (original.program.returned, target.program.returned) {
                    (Some(source), Some(target)) => equal(&nodes, &classes, source, Some(target))?,
                    (None, None) => {}
                    _ => return Err(mismatch()),
                }
            }
            (Branch::Goto(next), TargetBranch::Goto) if target.edges.len() == 1 => {
                edge(
                    source,
                    original,
                    bindings,
                    next.get() as usize,
                    target,
                    0,
                    &nodes,
                    &classes,
                    out,
                )?;
            }
            (
                Branch::Switch {
                    selector,
                    cases,
                    otherwise,
                },
                TargetBranch::Switch {
                    selector: actual,
                    cases: target_cases,
                    otherwise: target_otherwise,
                },
            ) => {
                equal(&nodes, &classes, *selector, Some(*actual))?;
                let scalar = nodes[*selector].scalar;
                if scalar == ScalarV30::Unit {
                    return Err(mismatch());
                }
                let mut ordered = vector(target_cases.len(), out)?;
                ordered.extend_from_slice(target_cases);
                out.budget.charge_work(
                    target_cases
                        .len()
                        .checked_mul(logarithm(target_cases.len()) + 1)
                        .ok_or(Resource::Arithmetic)?,
                )?;
                ordered.sort_unstable_by_key(|row| row.0);
                if ordered.windows(2).any(|pair| pair[0].0 >= pair[1].0) {
                    return Err(mismatch());
                }
                let mut left = 0;
                let mut right = 0;
                let mut covered = 0u128;
                while left < cases.len() || right < ordered.len() {
                    out.budget.charge_work(5)?;
                    let value = match (cases.get(left), ordered.get(right)) {
                        (Some(a), Some(b)) => a.0.min(b.0),
                        (Some(a), None) => a.0,
                        (None, Some(b)) => b.0,
                        (None, None) => unreachable!(),
                    };
                    if value >= (1u128 << scalar.width()) {
                        return Err(mismatch());
                    }
                    let next = if cases.get(left).is_some_and(|row| row.0 == value) {
                        let next = cases[left].1;
                        left += 1;
                        next
                    } else {
                        *otherwise
                    };
                    let target_edge = if ordered.get(right).is_some_and(|row| row.0 == value) {
                        let next = ordered[right].1;
                        right += 1;
                        next
                    } else {
                        *target_otherwise
                    };
                    edge(
                        source,
                        original,
                        bindings,
                        next.get() as usize,
                        target,
                        target_edge,
                        &nodes,
                        &classes,
                        out,
                    )?;
                    covered += 1;
                }
                if covered < (1u128 << scalar.width()) {
                    edge(
                        source,
                        original,
                        bindings,
                        otherwise.get() as usize,
                        target,
                        *target_otherwise,
                        &nodes,
                        &classes,
                        out,
                    )?;
                }
            }
            _ => return Err(mismatch()),
        }
        Ok(Self {
            source_nodes: nodes,
            source_observations,
            target_observations,
        })
    }
}

fn prepare_edge(
    source: &SourceControl,
    bindings: &[Option<BlockBindings>],
    next: usize,
    inventory: &Inventory<'_>,
    target: &mut TargetBlock,
    out: &mut Writer<'_, '_>,
) -> Result<()> {
    let block = source
        .blocks
        .get(next)
        .and_then(Option::as_ref)
        .ok_or_else(mismatch)?;
    let binding = bindings
        .get(next)
        .and_then(Option::as_ref)
        .ok_or_else(mismatch)?;
    if block.live.len() != binding.live.len() {
        return Err(mismatch());
    }
    for &(local, definition) in &binding.live {
        out.budget.charge_work(2)?;
        if local as usize >= source.locals {
            return Err(mismatch());
        }
        if let Some(definition) = definition {
            // Target phi parameters are assigned simultaneously by the edge.
            let is_parameter = matches!(inventory.definitions().get(definition).map(|row| row.coordinate),
                Some(fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1::BlockArgument { block, .. }) if block == binding.physical);
            if !is_parameter {
                target.observe(inventory, definition, out)?;
            }
        }
    }
    Ok(())
}

fn equal(
    nodes: &[NodeV30],
    classes: &[Vec<usize>; 2],
    source: usize,
    target: Option<usize>,
) -> Result<()> {
    match target {
        Some(target)
            if classes[0].get(source) == classes[1].get(target)
                && classes[0].get(source).is_some() =>
        {
            Ok(())
        }
        None if nodes.get(source).is_some_and(|node| {
            node.scalar == ScalarV30::Unit && node.expression == ExpressionV30::Constant(0)
        }) =>
        {
            Ok(())
        }
        _ => Err(mismatch()),
    }
}

#[allow(clippy::too_many_arguments)]
fn edge(
    source: &SourceControl,
    original: &SourceBlock,
    bindings: &[Option<BlockBindings>],
    next: usize,
    target: &TargetBlock,
    ordinal: usize,
    nodes: &[NodeV30],
    classes: &[Vec<usize>; 2],
    out: &mut Writer<'_, '_>,
) -> Result<()> {
    out.budget.charge_work(3)?;
    let binding = bindings
        .get(next)
        .and_then(Option::as_ref)
        .ok_or_else(mismatch)?;
    let actual = target.edges.get(ordinal).ok_or_else(mismatch)?;
    if actual.target != binding.physical {
        return Err(mismatch());
    }
    let expected = source
        .blocks
        .get(next)
        .and_then(Option::as_ref)
        .ok_or_else(mismatch)?;
    if expected.live.len() != binding.live.len() {
        return Err(mismatch());
    }
    for (expected, &(local, definition)) in expected.live.iter().zip(&binding.live) {
        out.budget.charge_work(3)?;
        if expected.local != local {
            return Err(mismatch());
        }
        let source_node = original
            .program
            .locals
            .get(local as usize)
            .copied()
            .flatten()
            .ok_or_else(mismatch)?;
        let target_node = match definition {
            Some(definition) => {
                out.budget.charge_work(logarithm(actual.arguments.len()))?;
                Some(
                    match actual
                        .arguments
                        .binary_search_by_key(&definition, |row| row.0)
                    {
                        Ok(at) => actual.arguments[at].1,
                        Err(_) => target.program.value(definition)?,
                    },
                )
            }
            None => None,
        };
        equal(nodes, classes, source_node, target_node)?;
    }
    Ok(())
}

fn logarithm(count: usize) -> usize {
    (usize::BITS - count.max(1).leading_zeros()) as usize + 1
}

fn headers() -> usize {
    size_of::<BlockPair>()
        + size_of::<Result<BlockPair>>()
        + size_of::<Vec<Option<Option<usize>>>>()
        + size_of::<Vec<NodeV30>>()
        + size_of::<Vec<usize>>()
        + size_of::<Vec<Option<usize>>>()
        + size_of::<Vec<(u128, usize)>>()
        + size_of::<[Vec<usize>; 2]>()
        + size_of::<&SourceControl>()
        + size_of::<&SourceBlock>()
        + size_of::<&[Option<BlockBindings>]>()
        + size_of::<&Inventory<'_>>()
        + size_of::<&mut TargetBlock>()
        + size_of::<&mut Writer<'_, '_>>()
        + size_of::<Result<()>>()
        + size_of::<Option<usize>>()
        + size_of::<(usize, usize, u128)>()
}

#[cfg(test)]
#[path = "original_semantic_mir_control_pair_v31_tests.rs"]
mod tests;
