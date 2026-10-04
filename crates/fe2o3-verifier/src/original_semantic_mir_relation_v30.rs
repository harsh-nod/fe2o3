//! Structural congruence of independently read, concretely interpreted scalar DAGs.
//! The containing source-owned producer must supply authentic per-step endpoints.

use super::super::{Error, Resource, Result, Writer};
use super::{
    ExpressionV30, NodeV30, ScalarV30, SourceProgramV30, canonical::CanonicalProgramV30,
    emit_graph_v30, vector,
};
use std::{fmt::Write as _, mem::size_of};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum EndpointV30 {
    Unit,
    Definition(usize),
}

#[derive(Clone, Copy)]
struct ClassRow {
    depth: usize,
    side: usize,
    index: usize,
    node: NodeV30,
}

fn mapped(
    expression: ExpressionV30,
    mut edge: impl FnMut(usize) -> Result<usize>,
) -> Result<ExpressionV30> {
    Ok(match expression {
        ExpressionV30::Argument(argument) => ExpressionV30::Argument(argument),
        ExpressionV30::Constant(bits) => ExpressionV30::Constant(bits),
        ExpressionV30::Not(input) => ExpressionV30::Not(edge(input)?),
        ExpressionV30::Select {
            condition,
            true_value,
            false_value,
        } => ExpressionV30::Select {
            condition: edge(condition)?,
            true_value: edge(true_value)?,
            false_value: edge(false_value)?,
        },
        ExpressionV30::Binary {
            operation,
            left,
            right,
        } => ExpressionV30::Binary {
            operation,
            left: edge(left)?,
            right: edge(right)?,
        },
    })
}

// Assign classes across both DAGs by dependency depth, then exact typed key.
// Every key is constant-sized and every dependency belongs to an earlier depth.
pub(super) fn classes(nodes: [&[NodeV30]; 2], out: &mut Writer<'_, '_>) -> Result<[Vec<usize>; 2]> {
    let count = nodes[0]
        .len()
        .checked_add(nodes[1].len())
        .ok_or(Resource::Arithmetic)?;
    out.budget.reserve_storage(
        size_of::<Vec<ClassRow>>()
            + 4 * size_of::<Vec<usize>>()
            + size_of::<ClassRow>()
            + size_of::<Result<ExpressionV30>>()
            + size_of::<Result<[Vec<usize>; 2]>>()
            + 8 * size_of::<usize>(),
    )?;
    let levels = usize::BITS as usize - count.max(1).leading_zeros() as usize;
    out.budget.charge_work(
        count
            .checked_mul(
                levels
                    .checked_mul(8)
                    .and_then(|n| n.checked_add(32))
                    .ok_or(Resource::Arithmetic)?,
            )
            .ok_or(Resource::Arithmetic)?,
    )?;
    let mut rows = vector(count, out)?;
    let mut result = [vector(nodes[0].len(), out)?, vector(nodes[1].len(), out)?];
    for side in 0..2 {
        let mut depths: Vec<usize> = vector(nodes[side].len(), out)?;
        for (index, node) in nodes[side].iter().copied().enumerate() {
            let mut depth = 0usize;
            mapped(node.expression, |input| {
                let input_depth = depths.get(input).copied().ok_or(Error::Statement(
                    "original MIR scalar relation has a forward or absent edge",
                ))?;
                depth = depth.max(input_depth.checked_add(1).ok_or(Resource::Arithmetic)?);
                Ok(input)
            })?;
            depths.push(depth);
            result[side].push(usize::MAX);
            rows.push(ClassRow {
                depth,
                side,
                index,
                node,
            });
        }
    }
    rows.sort_unstable_by_key(|row| (row.depth, row.side, row.index));
    let mut start = 0;
    let mut next_class = 0usize;
    while start < rows.len() {
        let end = start + rows[start..].partition_point(|row| row.depth == rows[start].depth);
        for row in &mut rows[start..end] {
            row.node.expression = mapped(row.node.expression, |input| {
                result[row.side]
                    .get(input)
                    .copied()
                    .filter(|value| *value != usize::MAX)
                    .ok_or(Resource::Accounting.into())
            })?;
        }
        rows[start..end].sort_unstable_by_key(|row| (row.node, row.side, row.index));
        let mut previous = None;
        let mut class = 0;
        for row in &rows[start..end] {
            if previous != Some(row.node) {
                class = next_class;
                next_class = next_class.checked_add(1).ok_or(Resource::Arithmetic)?;
                previous = Some(row.node);
            }
            result[row.side][row.index] = class;
        }
        start = end;
    }
    Ok(result)
}

fn arguments(
    nodes: &[NodeV30],
    count: usize,
    out: &mut Writer<'_, '_>,
) -> Result<Vec<Option<ScalarV30>>> {
    let mut arguments = vector(count, out)?;
    arguments.resize(count, None);
    for node in nodes.get(..count).ok_or(Resource::Accounting)? {
        out.budget.charge_work(3)?;
        let ExpressionV30::Argument(index) = node.expression else {
            return Err(Error::Statement(
                "original MIR scalar argument order differs",
            ));
        };
        let slot = arguments
            .get_mut(index as usize)
            .ok_or(Resource::Accounting)?;
        if slot.replace(node.scalar).is_some() {
            return Err(Error::Statement(
                "original MIR scalar argument identity is repeated",
            ));
        }
    }
    Ok(arguments)
}

pub(super) fn check(
    source: &SourceProgramV30,
    target: &CanonicalProgramV30,
    endpoints: &[EndpointV30],
    out: &mut Writer<'_, '_>,
) -> Result<()> {
    out.budget.reserve_storage(
        size_of::<[Vec<usize>; 2]>()
            + 2 * size_of::<Vec<Option<ScalarV30>>>()
            + size_of::<Result<Vec<Option<ScalarV30>>>>() * 2
            + size_of::<Result<()>>(),
    )?;
    out.budget.charge_work(4)?;
    if source.arguments != target.arguments || source.assignments.len() != endpoints.len() {
        return Err(Error::Statement(
            "original MIR scalar relation has incomplete endpoints",
        ));
    }
    let source_arguments = arguments(&source.nodes, source.arguments, out)?;
    let target_arguments = arguments(&target.nodes, target.arguments, out)?;
    out.budget.charge_work(source.arguments)?;
    if source_arguments != target_arguments {
        return Err(Error::Statement(
            "original MIR scalar argument representation differs",
        ));
    }
    let [original, actual] = classes([&source.nodes, &target.nodes], out)?;
    for (assignment, endpoint) in source.assignments.iter().zip(endpoints) {
        out.budget.charge_work(4)?;
        let node = source.nodes.get(assignment.value).ok_or(Error::Statement(
            "original MIR scalar relation source endpoint is absent",
        ))?;
        let same = match *endpoint {
            EndpointV30::Unit => node.scalar == ScalarV30::Unit,
            EndpointV30::Definition(definition) => {
                let value = target.value(definition)?;
                original.get(assignment.value).ok_or(Resource::Accounting)?
                    == actual.get(value).ok_or(Resource::Accounting)?
            }
        };
        if !same {
            return Err(Error::Statement(
                "original MIR scalar step result differs from actual canonical value",
            ));
        }
    }
    out.budget.charge_work(2)?;
    let same_return = match (source.returned, target.returned) {
        (None, None) => true,
        (Some(source), Some(target)) => {
            original.get(source).ok_or(Resource::Accounting)?
                == actual.get(target).ok_or(Resource::Accounting)?
        }
        _ => false,
    };
    if !same_return {
        return Err(Error::Statement(
            "original MIR scalar return differs from actual canonical value",
        ));
    }
    Ok(())
}

pub(super) fn emit_prelude(out: &mut Writer<'_, '_>) -> Result<()> {
    write!(out, "spec fn source_signed_v30(value: int, modulus: int) -> int {{\n if value < modulus / 2 {{ value }} else {{ value - modulus }}\n}}\n")
        .map_err(|_| out.error())
}

/// Both traces are independently interpreted. The exact endpoint relation is
/// checked before emitting the theorem; no caller-selected equation is assumed.
pub(super) fn emit(
    source: &SourceProgramV30,
    target: &CanonicalProgramV30,
    endpoints: &[EndpointV30],
    root: usize,
    out: &mut Writer<'_, '_>,
) -> Result<()> {
    check(source, target, endpoints, out)?;
    source.emit_trace(root, out)?;
    emit_graph_v30(
        &target.nodes,
        target.arguments,
        root,
        "kir",
        endpoints.iter().map(|endpoint| match *endpoint {
            EndpointV30::Unit => Ok(None),
            EndpointV30::Definition(definition) => target.value(definition).map(Some),
        }),
        target.returned,
        out,
    )?;
    write!(out, "proof fn original_mir_refines_canonical_{root}_v30(base: Seq<int>)\n requires base.len() == {},\n", source.arguments)
        .map_err(|_| out.error())?;
    for node in source
        .nodes
        .get(..source.arguments)
        .ok_or(Resource::Accounting)?
    {
        out.budget.charge_work(2)?;
        let ExpressionV30::Argument(argument) = node.expression else {
            return Err(Error::Statement(
                "original MIR scalar argument order differs",
            ));
        };
        let width = node.scalar.width();
        if width == 0 {
            return Err(Error::Statement(
                "original MIR zero-sized argument ABI is not modeled",
            ));
        }
        let modulus = 1u128 << width;
        write!(out, " 0int <= base[{argument}] < {modulus}int,\n").map_err(|_| out.error())?;
    }
    write!(out, " ensures original_mir_trace_{root}_v30(base) == original_kir_trace_{root}_v30(base),\n{{\n let original = original_mir_trace_{root}_v30(base);\n let actual = original_kir_trace_{root}_v30(base);\n assert_seqs_equal!(original, actual);\n}}\n")
        .map_err(|_| out.error())?;
    Ok(())
}
