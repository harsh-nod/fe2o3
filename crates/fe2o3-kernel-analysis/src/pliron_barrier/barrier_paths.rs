use std::collections::HashMap;

use dialect_gpu::BarrierOp;
use dialect_kernel::{
    AnalysisSplitOp, BranchArgsOp, BranchOp, IndexEqualBranchArgsOp, IndexEqualBranchOp,
    IndexLessThanBranchArgsOp, IndexLessThanBranchOp, PipelineEventOp, ReturnOp, TensorLayoutOp,
    TrapOp,
};
use pliron::{builtin::ops::FuncOp, context::Context, operation::Operation};

use super::{
    BarrierPathBlockSummaryV1, BarrierPathFailureV1, BarrierPathSummaryV1,
    MAX_FALLBACK_BARRIER_CFG_BLOCKS_V1, MAX_FALLBACK_BARRIER_PATH_EVENTS_V1,
    merge_barrier_path_summary_v1, prepend_barrier_path_v1,
};
use crate::{
    pliron_function_inventory::BoundedPlironFunctionInventoryV1,
    pliron_progress::{run_pliron_progress_check_v1, strongly_connected_components},
};

#[derive(Clone, Copy)]
enum BarrierPathEndV1 {
    Branch,
    Return,
    Trap,
}

struct BarrierPathNodeV1 {
    local: Vec<(usize, usize)>,
    successors: Vec<usize>,
    end: BarrierPathEndV1,
    has_collective: bool,
}

pub(super) fn summarize_all_barrier_paths(
    context: &Context,
    function: &FuncOp,
    inventory: &BoundedPlironFunctionInventoryV1,
) -> BarrierPathSummaryV1 {
    match summarize_barrier_cfg_v1(context, function, inventory) {
        Ok(_) => BarrierPathSummaryV1::Unique,
        Err(BarrierPathFailureV1::Divergent {
            first_trace,
            second_trace,
        }) => BarrierPathSummaryV1::Divergent {
            first_trace,
            second_trace,
        },
        Err(BarrierPathFailureV1::Incomplete(detail)) => BarrierPathSummaryV1::Incomplete(detail),
    }
}

fn build_barrier_cfg_v1(
    context: &Context,
    inventory: &BoundedPlironFunctionInventoryV1,
) -> Result<Vec<BarrierPathNodeV1>, BarrierPathFailureV1> {
    let blocks = inventory.blocks();
    if blocks.is_empty() {
        return Err(BarrierPathFailureV1::Incomplete(
            "the kernel CFG is empty".to_owned(),
        ));
    }
    if blocks.len() > MAX_FALLBACK_BARRIER_CFG_BLOCKS_V1 {
        return Err(BarrierPathFailureV1::Incomplete(format!(
            "the fallback barrier CFG has {} blocks, exceeding the bounded limit of {MAX_FALLBACK_BARRIER_CFG_BLOCKS_V1}",
            blocks.len(),
        )));
    }
    let block_indices = blocks
        .iter()
        .enumerate()
        .map(|(index, block)| (*block, index))
        .collect::<HashMap<_, _>>();
    let mut nodes = Vec::with_capacity(blocks.len());
    for (block_index, block) in blocks.iter().copied().enumerate() {
        let terminator = block
            .deref(context)
            .get_terminator(context)
            .ok_or_else(|| {
                BarrierPathFailureV1::Incomplete(format!("block {block_index} has no terminator"))
            })?;
        let mut local = Vec::new();
        let mut has_collective = false;
        for site in inventory.block_operations(block_index) {
            if site.pointer() == terminator {
                continue;
            }
            let operation = Operation::get_op_dyn(site.pointer(), context);
            let is_barrier = operation.downcast_ref::<BarrierOp>().is_some();
            has_collective |= is_barrier
                || operation.downcast_ref::<TensorLayoutOp>().is_some()
                || operation.downcast_ref::<PipelineEventOp>().is_some();
            if is_barrier {
                if local.len() == MAX_FALLBACK_BARRIER_PATH_EVENTS_V1 {
                    return Err(BarrierPathFailureV1::Incomplete(format!(
                        "block {block_index} has more than {MAX_FALLBACK_BARRIER_PATH_EVENTS_V1} barriers",
                    )));
                }
                local.push((block_index, site.operation()));
            }
        }
        let terminator = Operation::get_op_dyn(terminator, context);
        let (end, expected_successors) = if terminator.downcast_ref::<ReturnOp>().is_some() {
            (BarrierPathEndV1::Return, 0)
        } else if terminator.downcast_ref::<TrapOp>().is_some() {
            (BarrierPathEndV1::Trap, 0)
        } else if terminator.downcast_ref::<BranchOp>().is_some()
            || terminator.downcast_ref::<BranchArgsOp>().is_some()
        {
            (BarrierPathEndV1::Branch, 1)
        } else if terminator.downcast_ref::<IndexLessThanBranchOp>().is_some()
            || terminator
                .downcast_ref::<IndexLessThanBranchArgsOp>()
                .is_some()
            || terminator.downcast_ref::<IndexEqualBranchOp>().is_some()
            || terminator
                .downcast_ref::<IndexEqualBranchArgsOp>()
                .is_some()
            || terminator.downcast_ref::<AnalysisSplitOp>().is_some()
        {
            (BarrierPathEndV1::Branch, 2)
        } else {
            return Err(BarrierPathFailureV1::Incomplete(format!(
                "block {block_index} has an unsupported terminator",
            )));
        };
        let raw = terminator.get_operation().deref(context);
        if raw.get_num_successors() != expected_successors {
            return Err(BarrierPathFailureV1::Incomplete(format!(
                "block {block_index} has an invalid terminator successor count",
            )));
        }
        let successors = raw
            .successors()
            .map(|successor| {
                block_indices.get(&successor).copied().ok_or_else(|| {
                    BarrierPathFailureV1::Incomplete(format!(
                        "block {block_index} targets a block outside the kernel",
                    ))
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        nodes.push(BarrierPathNodeV1 {
            local,
            successors,
            end,
            has_collective,
        });
    }
    Ok(nodes)
}

fn summarize_barrier_cfg_v1(
    context: &Context,
    function: &FuncOp,
    inventory: &BoundedPlironFunctionInventoryV1,
) -> Result<BarrierPathBlockSummaryV1, BarrierPathFailureV1> {
    let nodes = build_barrier_cfg_v1(context, inventory)?;
    let edges = nodes
        .iter()
        .map(|node| node.successors.clone())
        .collect::<Vec<_>>();
    let mut reachable = vec![false; nodes.len()];
    let mut pending = vec![0];
    while let Some(block) = pending.pop() {
        if !reachable[block] {
            reachable[block] = true;
            pending.extend(edges[block].iter().copied());
        }
    }
    let components = strongly_connected_components(&edges);
    let mut component_of = vec![0; nodes.len()];
    for (component, members) in components.iter().enumerate() {
        for member in members {
            component_of[*member] = component;
        }
    }

    let mut condensed = Vec::with_capacity(components.len());
    let mut needs_progress = false;
    for (component, members) in components.iter().enumerate() {
        let first = members[0];
        let cyclic = members.len() > 1 || edges[first].contains(&first);
        let live_cycle = cyclic && reachable[first];
        if live_cycle {
            if let Some(block) = members
                .iter()
                .copied()
                .find(|block| nodes[*block].has_collective)
            {
                return Err(BarrierPathFailureV1::Incomplete(format!(
                    "cyclic block {block} contains a barrier, tensor collective, or pipeline event",
                )));
            }
            needs_progress = true;
        }
        // Every outgoing edge remains a possible suffix; no branch is assumed uniform.
        let successors = members
            .iter()
            .flat_map(|block| edges[*block].iter())
            .filter_map(|target| {
                let target_component = component_of[*target];
                (target_component != component).then_some(target_component)
            })
            .collect::<Vec<_>>();
        if live_cycle && successors.is_empty() {
            return Err(BarrierPathFailureV1::Incomplete(format!(
                "cyclic block {first} has no exit from its control-flow component",
            )));
        }
        condensed.push(BarrierPathNodeV1 {
            local: if cyclic {
                Vec::new()
            } else {
                nodes[first].local.clone()
            },
            successors,
            end: if cyclic {
                BarrierPathEndV1::Branch
            } else {
                nodes[first].end
            },
            has_collective: false,
        });
    }
    if needs_progress {
        // Reconstruct termination from this exact immutable function, not a detached certificate.
        let progress = run_pliron_progress_check_v1(context, function);
        if !progress.is_clean() {
            let detail = progress
                .findings()
                .first()
                .map(ToString::to_string)
                .unwrap_or_else(|| "progress analysis returned no proof".to_owned());
            return Err(BarrierPathFailureV1::Incomplete(format!(
                "barrier-free loop termination was not proved: {detail}",
            )));
        }
    }
    summarize_condensed_paths_v1(&condensed, component_of[0])
}

fn summarize_condensed_paths_v1(
    nodes: &[BarrierPathNodeV1],
    entry: usize,
) -> Result<BarrierPathBlockSummaryV1, BarrierPathFailureV1> {
    let mut states = vec![0_u8; nodes.len()];
    let mut summaries: Vec<Option<BarrierPathBlockSummaryV1>> = vec![None; nodes.len()];
    let mut pending = vec![(entry, false)];
    while let Some((block, expanded)) = pending.pop() {
        if states[block] == 2 {
            continue;
        }
        if !expanded {
            if states[block] == 1 {
                return Err(BarrierPathFailureV1::Incomplete(
                    "the condensed barrier CFG unexpectedly contains a cycle".to_owned(),
                ));
            }
            states[block] = 1;
            pending.push((block, true));
            pending.extend(
                nodes[block]
                    .successors
                    .iter()
                    .rev()
                    .map(|successor| (*successor, false)),
            );
            continue;
        }
        let node = &nodes[block];
        let mut complete = BarrierPathBlockSummaryV1::default();
        for successor in &node.successors {
            let suffix = summaries[*successor]
                .as_ref()
                .ok_or_else(|| {
                    BarrierPathFailureV1::Incomplete(
                        "a condensed barrier successor has no completed summary".to_owned(),
                    )
                })?
                .clone();
            merge_barrier_path_summary_v1(
                &mut complete,
                prepend_barrier_path_v1(&node.local, suffix)?,
            )?;
        }
        match node.end {
            BarrierPathEndV1::Return => complete.normal = Some(node.local.clone()),
            BarrierPathEndV1::Trap => complete.trapped_prefix = Some(node.local.clone()),
            BarrierPathEndV1::Branch => {}
        }
        summaries[block] = Some(complete);
        states[block] = 2;
    }
    summaries[entry].take().ok_or_else(|| {
        BarrierPathFailureV1::Incomplete("the barrier entry has no completed summary".to_owned())
    })
}
