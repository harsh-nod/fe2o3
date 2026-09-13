fn compute_reachable(
    successors: &[Vec<usize>],
    meter: &mut WorkMeter,
    resources: &mut ControlFlowResourcesV1<'_, '_>,
) -> Result<Vec<bool>, MeteredControlFlowErrorV1> {
    let blocks = successors.len();
    let mut reachable = resources.filled(blocks, false, 1)?;
    let mut pending = resources.allocate(blocks, 1)?;
    resources.charge(2)?;
    pending.push(0);
    reachable[0] = true;
    loop {
        resources.charge(1)?;
        let Some(block) = pending.pop() else { break };
        for successor in &successors[block] {
            resources.charge(4)?;
            meter.charge_reachability_edge()?;
            if !reachable[*successor] {
                reachable[*successor] = true;
                pending.push(*successor);
            }
        }
    }
    resources.free(pending, blocks, 1)?;
    Ok(reachable)
}

fn compute_reverse_postorder(
    successors: &[Vec<usize>],
    reachable: &[bool],
    meter: &mut WorkMeter,
    resources: &mut ControlFlowResourcesV1<'_, '_>,
) -> Result<Vec<usize>, MeteredControlFlowErrorV1> {
    let blocks = successors.len();
    let mut visited = resources.filled(blocks, false, 1)?;
    resources.charge(reachable.len())?;
    let reachable_count = reachable.iter().filter(|value| **value).count();
    let mut postorder = resources.allocate(reachable_count, 1)?;
    let mut stack = resources.allocate(blocks, 2)?;
    resources.charge(2)?;
    stack.push((0_usize, 0_usize));
    visited[0] = true;
    loop {
        resources.charge(3)?;
        let Some((block, next_successor)) = stack.last_mut() else {
            break;
        };
        if *next_successor == successors[*block].len() {
            postorder.push(*block);
            stack.pop();
            continue;
        }
        resources.charge(4)?;
        let successor = successors[*block][*next_successor];
        *next_successor += 1;
        meter.charge_depth_first_edge()?;
        if reachable[successor] && !visited[successor] {
            visited[successor] = true;
            stack.push((successor, 0));
        }
    }
    resources.charge(postorder.len())?;
    postorder.reverse();
    resources.free(stack, blocks, 2)?;
    resources.free(visited, blocks, 1)?;
    Ok(postorder)
}

fn compute_immediate_dominators(
    predecessors: &[Vec<usize>],
    reachable: &[bool],
    reverse_postorder: &[usize],
    meter: &mut WorkMeter,
    resources: &mut ControlFlowResourcesV1<'_, '_>,
) -> Result<Vec<Option<usize>>, MeteredControlFlowErrorV1> {
    let blocks = predecessors.len();
    let mut order = resources.filled(blocks, usize::MAX, 1)?;
    resources.charge(reverse_postorder.len())?;
    for (position, block) in reverse_postorder.iter().copied().enumerate() {
        order[block] = position;
    }
    let mut dominators = resources.filled(blocks, None, 2)?;
    resources.charge(1)?;
    dominators[0] = Some(0);
    loop {
        resources.charge(1)?;
        let mut changed = false;
        resources.charge(reverse_postorder.len())?;
        for block in reverse_postorder.iter().copied().skip(1) {
            let mut next = None;
            for predecessor in predecessors[block].iter().copied() {
                // Charge the filter even when it rejects the predecessor.
                resources.charge(3)?;
                if !reachable[predecessor] || dominators[predecessor].is_none() {
                    continue;
                }
                meter.charge_dominator_predecessor()?;
                next = Some(match next {
                    None => predecessor,
                    Some(previous) => intersect_dominators(
                        previous,
                        predecessor,
                        &dominators,
                        &order,
                        meter,
                        resources,
                    )?,
                });
                // Entry absorbs further intersections; CFG references are already validated.
                resources.charge(1)?;
                if next == Some(0) {
                    break;
                }
            }
            if let Some(next) = next {
                resources.charge(2)?;
                if dominators[block] != Some(next) {
                    dominators[block] = Some(next);
                    changed = true;
                }
            }
        }
        if !changed {
            break;
        }
    }
    resources.charge(1)?;
    dominators[0] = None;
    resources.free(order, blocks, 1)?;
    Ok(dominators)
}

fn intersect_dominators(
    mut left: usize,
    mut right: usize,
    dominators: &[Option<usize>],
    order: &[usize],
    meter: &mut WorkMeter,
    resources: &mut ControlFlowResourcesV1<'_, '_>,
) -> Result<usize, MeteredControlFlowErrorV1> {
    loop {
        resources.charge(1)?;
        if left == right {
            break;
        }
        loop {
            resources.charge(3)?;
            if order[left] <= order[right] {
                break;
            }
            meter.charge_dominator_climb()?;
            left = dominators[left].unwrap_or(0);
        }
        loop {
            resources.charge(3)?;
            if order[right] <= order[left] {
                break;
            }
            meter.charge_dominator_climb()?;
            right = dominators[right].unwrap_or(0);
        }
    }
    Ok(left)
}

fn compute_dominator_intervals(
    immediate_dominators: &[Option<usize>],
    reachable: &[bool],
    meter: &mut WorkMeter,
    resources: &mut ControlFlowResourcesV1<'_, '_>,
) -> Result<(Vec<u32>, Vec<u32>), MeteredControlFlowErrorV1> {
    let blocks = immediate_dominators.len();
    let mut child_counts = resources.filled(blocks, 0_usize, 1)?;
    let mut child_entries = 0;
    resources.charge(blocks)?;
    for (block, parent) in immediate_dominators.iter().copied().enumerate().skip(1) {
        if reachable[block] {
            let parent = parent.ok_or(CanonicalKernelIrVerificationResourceErrorV1::Accounting)?;
            child_counts[parent] += 1;
            child_entries += 1;
        }
    }
    let mut children = resources.rows(&child_counts)?;
    resources.charge(blocks)?;
    for (block, parent) in immediate_dominators.iter().copied().enumerate().skip(1) {
        if reachable[block] {
            let parent = parent.ok_or(CanonicalKernelIrVerificationResourceErrorV1::Accounting)?;
            children[parent].push(block);
        }
    }
    resources.free(child_counts, blocks, 1)?;
    let mut preorder = resources.filled(blocks, 0_u32, 1)?;
    let mut postorder = resources.filled(blocks, 0_u32, 1)?;
    let mut clock = 0_u32;
    let mut stack = resources.allocate(blocks, 2)?;
    resources.charge(1)?;
    stack.push((0_usize, false));
    loop {
        resources.charge(1)?;
        let Some((block, exiting)) = stack.pop() else {
            break;
        };
        resources.charge(3)?;
        meter.charge_interval_node()?;
        if exiting {
            postorder[block] = clock;
            clock = clock.saturating_add(1);
            continue;
        }
        preorder[block] = clock;
        clock = clock.saturating_add(1);
        resources.charge(checked_add(
            ControlFlowResource::AnalysisWork,
            children[block].len(),
            1,
        )?)?;
        stack.push((block, true));
        stack.extend(children[block].iter().rev().map(|child| (*child, false)));
    }
    resources.free(stack, blocks, 2)?;
    resources.free_rows(children, child_entries)?;
    Ok((preorder, postorder))
}

#[allow(clippy::too_many_arguments)]
fn compute_irreducible_blocks(
    block_ids: &[BlockId],
    successors: &[Vec<usize>],
    reachable: &[bool],
    preorder: &[u32],
    postorder: &[u32],
    meter: &mut WorkMeter,
    resources: &mut ControlFlowResourcesV1<'_, '_>,
) -> Result<Vec<BlockId>, MeteredControlFlowErrorV1> {
    let blocks = successors.len();
    let dominates = |definition: usize, use_block: usize| {
        if !reachable[use_block] {
            return definition == use_block;
        }
        reachable[definition]
            && preorder[definition] <= preorder[use_block]
            && postorder[use_block] <= postorder[definition]
    };
    let mut forward_counts = resources.filled(blocks, 0_usize, 1)?;
    let mut indegrees = resources.filled(blocks, 0_usize, 1)?;
    let mut forward_entries = 0;
    resources.charge(blocks)?;
    for (source, targets) in successors.iter().enumerate() {
        for target in targets {
            resources.charge(5)?;
            meter.charge_reducibility_edge()?;
            if dominates(*target, source) {
                continue;
            }
            forward_counts[source] += 1;
            forward_entries += 1;
            indegrees[*target] += 1;
        }
    }
    let mut forward = resources.rows(&forward_counts)?;
    resources.charge(blocks)?;
    for (source, targets) in successors.iter().enumerate() {
        for target in targets {
            resources.charge(5)?;
            if !dominates(*target, source) {
                forward[source].push(*target);
            }
        }
    }
    resources.free(forward_counts, blocks, 1)?;
    let mut ready = resources.allocate(blocks, 1)?;
    resources.charge(blocks)?;
    for (block, count) in indegrees.iter().enumerate() {
        if *count == 0 {
            ready.push(block);
        }
    }
    let mut visited = resources.filled(blocks, false, 1)?;
    let mut cursor = 0;
    loop {
        resources.charge(1)?;
        let Some(&block) = ready.get(cursor) else {
            break;
        };
        cursor += 1;
        resources.charge(1)?;
        meter.charge_reducibility_node()?;
        visited[block] = true;
        resources.charge(forward[block].len())?;
        for successor in &forward[block] {
            let count = &mut indegrees[*successor];
            *count -= 1;
            if *count == 0 {
                ready.push(*successor);
            }
        }
    }
    resources.charge(blocks)?;
    let irreducible_count = visited.iter().filter(|visited| !**visited).count();
    let mut irreducible = resources.allocate(irreducible_count, 1)?;
    resources.charge(blocks)?;
    for (block, visited) in block_ids.iter().copied().zip(visited.iter().copied()) {
        if !visited {
            irreducible.push(block);
        }
    }
    resources.sort(&mut irreducible)?;
    resources.free(visited, blocks, 1)?;
    resources.free(ready, blocks, 1)?;
    resources.free(indegrees, blocks, 1)?;
    resources.free_rows(forward, forward_entries)?;
    Ok(irreducible)
}
