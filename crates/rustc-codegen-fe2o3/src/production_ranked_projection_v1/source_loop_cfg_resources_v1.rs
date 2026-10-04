//! The existing source-loop CFG algorithm with a strict original-ledger mode.
//! This is source analysis only, not a ranked CFG or an assertion certificate.
use super::bf16_nominal_preparation_resources_v1::{PreparationResourcesV1, resource};
use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1 as Resource;
use std::mem::size_of;

type Result<T> = std::result::Result<T, ProductionRankedProjectionErrorV1>;

pub(super) fn projected_loop_cfg_graph_with_resources_v1(
    function: &SemanticFunctionDeclV1,
    resources: &mut PreparationResourcesV1<'_, '_>,
) -> Result<ProjectedLoopCfgV1> {
    if resources.is_metered() {
        resources.work(16)?;
        let frame = 4096usize
            .checked_add(size_of::<ProjectedLoopCfgV1>())
            .and_then(|n| n.checked_add(2 * size_of::<Result<ProjectedLoopCfgV1>>()))
            .ok_or_else(|| resource(Resource::Arithmetic))?;
        resources.reserve_storage(frame)?;
    }
    let block_count = function.blocks().len();
    if block_count == 0 || block_count > MAX_RANKED_BOUNDS_BLOCKS {
        return Err(ProductionRankedProjectionErrorV1::Unsupported(
            "semantic CFG exceeds the ranked block limit before loop analysis",
        ));
    }
    let checked_target = |target: SemanticBlockIdV1| {
        let target = target.index() as usize;
        (target < block_count).then_some(target).ok_or(
            ProductionRankedProjectionErrorV1::Unsupported(
                "a semantic CFG edge outside the function during loop analysis",
            ),
        )
    };
    let mut successors = if resources.is_metered() {
        let mut values = Vec::new();
        resources.reserve(&mut values, block_count)?;
        values
    } else {
        Vec::with_capacity(block_count)
    };
    let mut edge_count = 0_usize;
    resources.work(block_count)?;
    for block in function.blocks() {
        resources.work(8)?;
        let mut block_successors = match block.terminator().kind() {
            SemanticTerminatorKindV1::Goto(edge) => {
                one_target(checked_target(edge.target())?, resources)?
            }
            SemanticTerminatorKindV1::SwitchInt { targets, .. } => {
                let mut successors = if resources.is_metered() {
                    let mut values = Vec::new();
                    resources.reserve(
                        &mut values,
                        targets
                            .values()
                            .len()
                            .checked_add(1)
                            .ok_or_else(|| resource(Resource::Arithmetic))?,
                    )?;
                    resources.work(targets.values().len())?;
                    for target in targets.values() {
                        resources.work(3)?;
                        values.push(checked_target(target.edge().target())?);
                    }
                    values
                } else {
                    targets
                        .values()
                        .iter()
                        .map(|target| checked_target(target.edge().target()))
                        .collect::<Result<Vec<_>>>()?
                };
                resources.work(1)?;
                let otherwise = checked_target(targets.otherwise().target())?;
                if resources.is_metered() {
                    resources.work(
                        targets
                            .values()
                            .len()
                            .checked_mul(2)
                            .and_then(|n| n.checked_add(8))
                            .ok_or_else(|| resource(Resource::Arithmetic))?,
                    )?;
                }
                if !(targets.values().len() == 2
                    && targets.values().iter().any(|target| target.value() == 0)
                    && targets.values().iter().any(|target| target.value() == 1)
                    && switch_fallback_is_empty_unreachable_v1(function, otherwise))
                {
                    resources.work(1)?;
                    successors.push(otherwise);
                }
                successors
            }
            SemanticTerminatorKindV1::Call(call) => {
                if resources.is_metered() {
                    resources.work(2)?;
                    match call.destination() {
                        Some(destination) => {
                            one_target(checked_target(destination.edge().target())?, resources)?
                        }
                        None => Vec::new(),
                    }
                } else {
                    call.destination()
                        .map(|destination| checked_target(destination.edge().target()))
                        .transpose()?
                        .into_iter()
                        .collect()
                }
            }
            SemanticTerminatorKindV1::Assert { target, .. }
            | SemanticTerminatorKindV1::Drop { target, .. } => {
                one_target(checked_target(target.target())?, resources)?
            }
            SemanticTerminatorKindV1::FalseEdge { .. } => {
                return Err(ProductionRankedProjectionErrorV1::Incomplete(
                    "a false edge before uniform induction CFG normalization",
                ));
            }
            SemanticTerminatorKindV1::Return
            | SemanticTerminatorKindV1::TailCall(_)
            | SemanticTerminatorKindV1::UnwindResume
            | SemanticTerminatorKindV1::UnwindTerminate
            | SemanticTerminatorKindV1::Abort
            | SemanticTerminatorKindV1::Unreachable => Vec::new(),
        };
        resources.sort_unique_indices(&mut block_successors)?;
        resources.work(3)?;
        edge_count = edge_count.checked_add(block_successors.len()).ok_or(
            ProductionRankedProjectionErrorV1::Unsupported(
                "semantic CFG edge count overflow during loop analysis",
            ),
        )?;
        if edge_count > MAX_RANKED_BOUNDS_EDGES {
            return Err(ProductionRankedProjectionErrorV1::Unsupported(
                "semantic CFG exceeds the ranked edge limit before loop analysis",
            ));
        }
        successors.push(block_successors);
    }
    let mut predecessors = if resources.is_metered() {
        let mut values = resources.nested(block_count)?;
        let mut incoming = resources.filled(block_count, 0usize)?;
        // Pre-count before any row allocation, avoiding hidden amortized growth.
        resources.work(successors.len())?;
        for targets in &successors {
            resources.work(1)?;
            resources.work(targets.len())?;
            for &target in targets {
                resources.work(3)?;
                incoming[target] = incoming[target]
                    .checked_add(1)
                    .ok_or_else(|| resource(Resource::Arithmetic))?;
            }
        }
        resources.work(values.len())?;
        for (row, count) in values.iter_mut().zip(incoming) {
            resources.work(1)?;
            resources.reserve(row, count)?;
        }
        values
    } else {
        vec![Vec::new(); block_count]
    };
    resources.work(successors.len())?;
    for (source, targets) in successors.iter().enumerate() {
        resources.work(1)?;
        resources.work(targets.len())?;
        for &target in targets {
            resources.work(2)?;
            predecessors[target].push(source);
        }
    }
    let entry = function.entry().index() as usize;
    if entry >= block_count {
        return Err(ProductionRankedProjectionErrorV1::Unsupported(
            "semantic entry block outside the function during loop analysis",
        ));
    }
    let mut reachable = if resources.is_metered() {
        resources.filled(block_count, false)?
    } else {
        vec![false; block_count]
    };
    let mut pending = if resources.is_metered() {
        // Each reachable block expands once, pushing at most all unique edges.
        let mut values = Vec::new();
        resources.reserve(
            &mut values,
            edge_count
                .checked_add(1)
                .ok_or_else(|| resource(Resource::Arithmetic))?,
        )?;
        resources.work(1)?;
        values.push(entry);
        values
    } else {
        vec![entry]
    };
    loop {
        resources.work(1)?;
        let Some(block) = pending.pop() else {
            break;
        };
        resources.work(1)?;
        if reachable[block] {
            continue;
        }
        resources.work(1)?;
        reachable[block] = true;
        resources.work(successors[block].len())?;
        if resources.is_metered()
            && pending
                .len()
                .checked_add(successors[block].len())
                .is_none_or(|needed| needed > pending.capacity())
        {
            return Err(resource(Resource::Accounting));
        }
        pending.extend(successors[block].iter().copied());
    }
    Ok(ProjectedLoopCfgV1 {
        successors,
        predecessors,
        reachable,
        entry,
    })
}
fn one_target(target: usize, resources: &mut PreparationResourcesV1<'_, '_>) -> Result<Vec<usize>> {
    if resources.is_metered() {
        let mut values = Vec::new();
        resources.reserve(&mut values, 1)?;
        resources.work(1)?;
        values.push(target);
        Ok(values)
    } else {
        Ok(vec![target])
    }
}

#[cfg(test)]
#[path = "source_loop_cfg_resources_v1_tests.rs"]
mod tests;
