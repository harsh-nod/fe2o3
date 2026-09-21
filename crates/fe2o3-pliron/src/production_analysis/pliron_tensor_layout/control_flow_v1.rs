fn charge_convergence_work(
    work: &mut usize,
    amount: usize,
) -> Result<(), PlironTensorLayoutFindingV1> {
    *work = work
        .checked_add(amount)
        .ok_or(PlironTensorLayoutFindingV1::ResourceLimitExceeded)?;
    if *work > MAX_PLIRON_TENSOR_UNIFORMITY_WORK_UNITS_V1 {
        return Err(PlironTensorLayoutFindingV1::ResourceLimitExceeded);
    }
    Ok(())
}

fn bounded_predecessors(
    successors: &[Vec<usize>],
    work: &mut usize,
) -> Result<Vec<Vec<usize>>, PlironTensorLayoutFindingV1> {
    let edge_count = successors
        .iter()
        .try_fold(0_usize, |count, targets| count.checked_add(targets.len()))
        .ok_or(PlironTensorLayoutFindingV1::ResourceLimitExceeded)?;
    charge_convergence_work(work, successors.len())?;
    charge_convergence_work(work, edge_count)?;
    let mut predecessors = vec![Vec::new(); successors.len()];
    for (block, targets) in successors.iter().enumerate() {
        for target in targets {
            if *target >= successors.len() {
                return Err(PlironTensorLayoutFindingV1::ConvergenceAnalysisIncomplete {
                    detail: "convergence edge is outside the kernel CFG".to_owned(),
                });
            }
            predecessors[*target].push(block);
        }
    }
    Ok(predecessors)
}

struct TensorReachabilityV1 {
    component_of: Vec<usize>,
    tensors_by_component: Vec<Vec<u64>>,
}

impl TensorReachabilityV1 {
    fn block_reaches(
        &self,
        block: usize,
        tensor: usize,
    ) -> Result<bool, PlironTensorLayoutFindingV1> {
        let component = self.component_of.get(block).copied().ok_or_else(|| {
            PlironTensorLayoutFindingV1::ConvergenceAnalysisIncomplete {
                detail: "tensor reachability queried a block outside the kernel CFG".to_owned(),
            }
        })?;
        let words = self.tensors_by_component.get(component).ok_or_else(|| {
            PlironTensorLayoutFindingV1::ConvergenceAnalysisIncomplete {
                detail: "tensor reachability lost a block SCC".to_owned(),
            }
        })?;
        let word = words.get(tensor / u64::BITS as usize).ok_or_else(|| {
            PlironTensorLayoutFindingV1::ConvergenceAnalysisIncomplete {
                detail: "tensor reachability queried an unknown tensor site".to_owned(),
            }
        })?;
        Ok(word & (1_u64 << (tensor % u64::BITS as usize)) != 0)
    }
}

fn bounded_tensor_reachability(
    successors: &[Vec<usize>],
    predecessors: &[Vec<usize>],
    tensor_blocks: &[usize],
    work: &mut usize,
) -> Result<TensorReachabilityV1, PlironTensorLayoutFindingV1> {
    let block_count = successors.len();
    if predecessors.len() != block_count {
        return Err(PlironTensorLayoutFindingV1::ConvergenceAnalysisIncomplete {
            detail: "tensor reachability inputs do not match the kernel CFG".to_owned(),
        });
    }
    if tensor_blocks.len() > MAX_PLIRON_TENSOR_LAYOUT_FINDINGS_V1 {
        return Err(PlironTensorLayoutFindingV1::ResourceLimitExceeded);
    }
    for targets in successors {
        for target in targets.iter().copied() {
            if target >= block_count {
                return Err(PlironTensorLayoutFindingV1::ConvergenceAnalysisIncomplete {
                    detail: "tensor reachability edge is outside the kernel CFG".to_owned(),
                });
            }
        }
    }
    if tensor_blocks.iter().any(|block| *block >= block_count) {
        return Err(PlironTensorLayoutFindingV1::ConvergenceAnalysisIncomplete {
            detail: "tensor block is outside the kernel CFG".to_owned(),
        });
    }

    charge_convergence_work(work, block_count.saturating_mul(3))?;
    let mut visited = vec![false; block_count];
    let mut finish_order = Vec::new();
    finish_order
        .try_reserve_exact(block_count)
        .map_err(|_| PlironTensorLayoutFindingV1::ResourceLimitExceeded)?;
    let mut dfs = Vec::new();
    dfs.try_reserve_exact(block_count)
        .map_err(|_| PlironTensorLayoutFindingV1::ResourceLimitExceeded)?;
    for root in 0..block_count {
        if visited[root] {
            continue;
        }
        visited[root] = true;
        dfs.push((root, 0_usize));
        while let Some((block, successor_index)) = dfs.last_mut() {
            charge_convergence_work(work, 1)?;
            if let Some(successor) = successors[*block].get(*successor_index).copied() {
                *successor_index += 1;
                if !visited[successor] {
                    visited[successor] = true;
                    dfs.push((successor, 0));
                }
            } else {
                finish_order.push(*block);
                dfs.pop();
            }
        }
    }

    let mut component_of = vec![usize::MAX; block_count];
    let mut component_count = 0_usize;
    let mut component_worklist = Vec::new();
    component_worklist
        .try_reserve_exact(block_count)
        .map_err(|_| PlironTensorLayoutFindingV1::ResourceLimitExceeded)?;
    for root in finish_order.into_iter().rev() {
        if component_of[root] != usize::MAX {
            continue;
        }
        let component = component_count;
        component_count = component_count
            .checked_add(1)
            .ok_or(PlironTensorLayoutFindingV1::ResourceLimitExceeded)?;
        component_of[root] = component;
        component_worklist.push(root);
        while let Some(block) = component_worklist.pop() {
            charge_convergence_work(work, 1)?;
            for predecessor in predecessors[block].iter().copied() {
                charge_convergence_work(work, 1)?;
                if predecessor >= block_count {
                    return Err(PlironTensorLayoutFindingV1::ConvergenceAnalysisIncomplete {
                        detail: "tensor reachability predecessor is outside the kernel CFG"
                            .to_owned(),
                    });
                }
                if component_of[predecessor] == usize::MAX {
                    component_of[predecessor] = component;
                    component_worklist.push(predecessor);
                }
            }
        }
    }
    if component_of.contains(&usize::MAX) {
        return Err(PlironTensorLayoutFindingV1::ConvergenceAnalysisIncomplete {
            detail: "tensor reachability did not assign every block to an SCC".to_owned(),
        });
    }

    charge_convergence_work(work, component_count)?;
    let edge_count = successors
        .iter()
        .try_fold(0_usize, |count, targets| count.checked_add(targets.len()));
    let Some(edge_count) = edge_count else {
        return Err(PlironTensorLayoutFindingV1::ResourceLimitExceeded);
    };
    let mut component_edges = HashSet::new();
    component_edges
        .try_reserve(edge_count)
        .map_err(|_| PlironTensorLayoutFindingV1::ResourceLimitExceeded)?;
    for (block, targets) in successors.iter().enumerate() {
        let source_component = component_of[block];
        for target in targets.iter().copied() {
            let target_component = component_of[target];
            if source_component != target_component
                && component_edges.insert((source_component, target_component))
            {
                charge_convergence_work(work, 1)?;
            }
        }
    }

    let mut component_outdegree = vec![0_usize; component_count];
    for (source, _) in component_edges.iter().copied() {
        component_outdegree[source] = component_outdegree[source]
            .checked_add(1)
            .ok_or(PlironTensorLayoutFindingV1::ResourceLimitExceeded)?;
    }
    let mut component_successors = Vec::new();
    component_successors
        .try_reserve_exact(component_count)
        .map_err(|_| PlironTensorLayoutFindingV1::ResourceLimitExceeded)?;
    for degree in component_outdegree {
        let mut targets = Vec::new();
        targets
            .try_reserve_exact(degree)
            .map_err(|_| PlironTensorLayoutFindingV1::ResourceLimitExceeded)?;
        component_successors.push(targets);
    }
    for (source, target) in component_edges {
        component_successors[source].push(target);
    }

    charge_convergence_work(work, component_count.saturating_mul(2))?;
    let mut indegree = vec![0_usize; component_count];
    for targets in &component_successors {
        for target in targets.iter().copied() {
            indegree[target] = indegree[target]
                .checked_add(1)
                .ok_or(PlironTensorLayoutFindingV1::ResourceLimitExceeded)?;
        }
    }
    let mut ready = VecDeque::new();
    ready
        .try_reserve_exact(component_count)
        .map_err(|_| PlironTensorLayoutFindingV1::ResourceLimitExceeded)?;
    ready.extend(
        indegree
            .iter()
            .enumerate()
            .filter_map(|(component, degree)| (*degree == 0).then_some(component)),
    );
    let mut topological = Vec::new();
    topological
        .try_reserve_exact(component_count)
        .map_err(|_| PlironTensorLayoutFindingV1::ResourceLimitExceeded)?;
    while let Some(component) = ready.pop_front() {
        charge_convergence_work(work, 1)?;
        topological.push(component);
        for successor in component_successors[component].iter().copied() {
            let degree = indegree.get_mut(successor).ok_or_else(|| {
                PlironTensorLayoutFindingV1::ConvergenceAnalysisIncomplete {
                    detail: "tensor reachability SCC edge is outside the condensation graph"
                        .to_owned(),
                }
            })?;
            *degree = degree.checked_sub(1).ok_or_else(|| {
                PlironTensorLayoutFindingV1::ConvergenceAnalysisIncomplete {
                    detail: "tensor reachability SCC indegree underflowed".to_owned(),
                }
            })?;
            if *degree == 0 {
                ready.push_back(successor);
            }
        }
    }
    if topological.len() != component_count {
        return Err(PlironTensorLayoutFindingV1::ConvergenceAnalysisIncomplete {
            detail: "tensor reachability condensation graph is cyclic".to_owned(),
        });
    }

    let word_count = tensor_blocks.len().div_ceil(u64::BITS as usize);
    let storage = component_count
        .checked_mul(word_count)
        .ok_or(PlironTensorLayoutFindingV1::ResourceLimitExceeded)?;
    charge_convergence_work(work, storage)?;
    let mut tensors_by_component = vec![vec![0_u64; word_count]; component_count];
    for (tensor, block) in tensor_blocks.iter().copied().enumerate() {
        let component = component_of[block];
        tensors_by_component[component][tensor / u64::BITS as usize] |=
            1_u64 << (tensor % u64::BITS as usize);
    }
    for component in topological.into_iter().rev() {
        for successor in component_successors[component].iter().copied() {
            charge_convergence_work(work, word_count)?;
            if component == successor {
                continue;
            }
            let (component_words, successor_words) = if component < successor {
                let (before_successor, from_successor) =
                    tensors_by_component.split_at_mut(successor);
                (&mut before_successor[component], &from_successor[0])
            } else {
                let (before_component, from_component) =
                    tensors_by_component.split_at_mut(component);
                (&mut from_component[0], &before_component[successor])
            };
            for (component_word, successor_word) in component_words.iter_mut().zip(successor_words)
            {
                *component_word |= successor_word;
            }
        }
    }

    Ok(TensorReachabilityV1 {
        component_of,
        tensors_by_component,
    })
}

fn bounded_postdominators(
    successors: &[Vec<usize>],
    reachable: &[bool],
    predecessors: &[Vec<usize>],
    work: &mut usize,
) -> Result<Vec<Option<Vec<u64>>>, PlironTensorLayoutFindingV1> {
    let block_count = successors.len();
    let word_count = block_count.div_ceil(u64::BITS as usize);
    if predecessors.len() != block_count || reachable.len() != block_count {
        return Err(PlironTensorLayoutFindingV1::ConvergenceAnalysisIncomplete {
            detail: "postdominator inputs do not match the kernel CFG".to_owned(),
        });
    }
    charge_convergence_work(work, block_count)?;
    charge_convergence_work(work, word_count)?;
    for (block, targets) in successors.iter().enumerate() {
        if !reachable.get(block).copied().unwrap_or(false) {
            continue;
        }
        for target in targets.iter().copied() {
            if target >= block_count {
                return Err(PlironTensorLayoutFindingV1::ConvergenceAnalysisIncomplete {
                    detail: "postdominator edge is outside the kernel CFG".to_owned(),
                });
            }
            let _ = reachable[target];
        }
    }

    charge_convergence_work(work, block_count)?;
    let mut can_reach_exit = vec![false; block_count];
    let mut worklist = VecDeque::new();
    for block in 0..block_count {
        if reachable[block] && successors[block].is_empty() {
            can_reach_exit[block] = true;
            worklist.push_back(block);
        }
    }
    while let Some(block) = worklist.pop_front() {
        for predecessor in predecessors[block].iter().copied() {
            charge_convergence_work(work, 1)?;
            let Some(predecessor_reaches_exit) = can_reach_exit.get_mut(predecessor) else {
                return Err(PlironTensorLayoutFindingV1::ConvergenceAnalysisIncomplete {
                    detail: "postdominator predecessor is outside the kernel CFG".to_owned(),
                });
            };
            if !*predecessor_reaches_exit {
                *predecessor_reaches_exit = true;
                worklist.push_back(predecessor);
            }
        }
    }

    charge_convergence_work(work, word_count)?;
    let mut universe = vec![0_u64; word_count];
    for block in 0..block_count {
        if can_reach_exit[block] {
            universe[block / u64::BITS as usize] |= 1_u64 << (block % u64::BITS as usize);
        }
    }
    let initial_words = can_reach_exit
        .iter()
        .filter(|available| **available)
        .count()
        .saturating_mul(word_count);
    charge_convergence_work(work, initial_words)?;
    let mut facts = can_reach_exit
        .iter()
        .map(|available| available.then(|| universe.clone()))
        .collect::<Vec<_>>();
    loop {
        let mut changed = false;
        for block in (0..block_count).rev() {
            if !can_reach_exit[block] {
                continue;
            }
            let mut next = if successors[block].is_empty()
                || successors[block]
                    .iter()
                    .any(|successor| !can_reach_exit[*successor])
            {
                charge_convergence_work(work, word_count)?;
                vec![0_u64; word_count]
            } else {
                let mut targets = successors[block].iter();
                let Some(first) = targets.next().copied() else {
                    return Err(PlironTensorLayoutFindingV1::ConvergenceAnalysisIncomplete {
                        detail: "a non-terminal postdominator block has no successor".to_owned(),
                    });
                };
                charge_convergence_work(work, word_count)?;
                let Some(first_fact) = facts[first].as_ref() else {
                    return Err(PlironTensorLayoutFindingV1::ConvergenceAnalysisIncomplete {
                        detail: "an exit-reachable successor has no postdominator fact".to_owned(),
                    });
                };
                let mut intersection = first_fact.clone();
                for successor in targets {
                    let Some(successor) = facts[*successor].as_ref() else {
                        return Err(PlironTensorLayoutFindingV1::ConvergenceAnalysisIncomplete {
                            detail: "an exit-reachable successor has no postdominator fact"
                                .to_owned(),
                        });
                    };
                    for (word, successor_word) in intersection.iter_mut().zip(successor) {
                        charge_convergence_work(work, 1)?;
                        *word &= successor_word;
                    }
                }
                intersection
            };
            next[block / u64::BITS as usize] |= 1_u64 << (block % u64::BITS as usize);
            let Some(slot) = facts[block].as_mut() else {
                return Err(PlironTensorLayoutFindingV1::ConvergenceAnalysisIncomplete {
                    detail: "an exit-reachable block has no postdominator fact".to_owned(),
                });
            };
            charge_convergence_work(work, word_count)?;
            if *slot != next {
                *slot = next;
                changed = true;
            }
        }
        if !changed {
            return Ok(facts);
        }
        charge_convergence_work(work, block_count)?;
    }
}

fn immediate_postdominator(
    controller: usize,
    facts: &[Option<Vec<u64>>],
    depths: &[u32],
    work: &mut usize,
) -> Result<Option<usize>, PlironTensorLayoutFindingV1> {
    if depths.len() != facts.len() {
        return Err(PlironTensorLayoutFindingV1::ConvergenceAnalysisIncomplete {
            detail: "postdominator depths do not match the kernel CFG".to_owned(),
        });
    }
    let Some(fact) = facts.get(controller).and_then(Option::as_ref) else {
        return Ok(None);
    };
    let mut best = None;
    let mut best_depth = 0_u32;
    let mut tied = false;
    for (word_index, word) in fact.iter().copied().enumerate() {
        let mut candidates = word;
        while candidates != 0 {
            charge_convergence_work(work, 1)?;
            let bit = candidates.trailing_zeros() as usize;
            candidates &= candidates - 1;
            let candidate = word_index * u64::BITS as usize + bit;
            if candidate == controller || candidate >= facts.len() {
                continue;
            }
            let depth = depths[candidate];
            if depth > best_depth {
                best = Some(candidate);
                best_depth = depth;
                tied = false;
            } else if depth == best_depth {
                tied = true;
            }
        }
    }
    Ok((!tied).then_some(best).flatten())
}

fn bounded_control_regions(
    successors: &[Vec<usize>],
    reachable: &[bool],
    branch_uniformity: &[SubgroupBranchUniformityV1],
    postdominators: &[Option<Vec<u64>>],
    work: &mut usize,
) -> Result<Vec<Option<TensorControlRegionV1>>, PlironTensorLayoutFindingV1> {
    let block_count = successors.len();
    let word_count = block_count.div_ceil(u64::BITS as usize);
    let controller_count = branch_uniformity
        .iter()
        .enumerate()
        .filter(|(block, uniformity)| {
            reachable[*block]
                && **uniformity != SubgroupBranchUniformityV1::Uniform
                && successors[*block].len() > 1
        })
        .count();
    let allocation_work = controller_count
        .checked_mul(word_count)
        .and_then(|work| work.checked_add(block_count))
        .ok_or(PlironTensorLayoutFindingV1::ResourceLimitExceeded)?;
    charge_convergence_work(work, allocation_work)?;
    let depth_work = block_count
        .checked_mul(word_count)
        .ok_or(PlironTensorLayoutFindingV1::ResourceLimitExceeded)?;
    charge_convergence_work(work, depth_work)?;
    let postdominator_depths = postdominators
        .iter()
        .map(|fact| {
            fact.as_ref()
                .map(|words| words.iter().map(|word| word.count_ones()).sum())
                .unwrap_or(0)
        })
        .collect::<Vec<_>>();
    let mut regions = (0..block_count).map(|_| None).collect::<Vec<_>>();
    for controller in 0..block_count {
        if !reachable[controller]
            || branch_uniformity[controller] == SubgroupBranchUniformityV1::Uniform
            || successors[controller].len() < 2
        {
            continue;
        }
        let reconvergence =
            immediate_postdominator(controller, postdominators, &postdominator_depths, work)?;
        let mut blocks = vec![0_u64; word_count];
        let mut region_blocks = Vec::new();
        let mut worklist = successors[controller]
            .iter()
            .copied()
            .collect::<VecDeque<_>>();
        while let Some(block) = worklist.pop_front() {
            charge_convergence_work(work, 1)?;
            let word = &mut blocks[block / u64::BITS as usize];
            let mask = 1_u64 << (block % u64::BITS as usize);
            if Some(block) == reconvergence || !reachable[block] || *word & mask != 0 {
                continue;
            }
            *word |= mask;
            charge_convergence_work(work, 1)?;
            region_blocks.push(block);
            worklist.extend(successors[block].iter().copied());
        }
        let contains = |block: usize| {
            blocks[block / u64::BITS as usize] & (1_u64 << (block % u64::BITS as usize)) != 0
        };
        charge_convergence_work(work, region_blocks.len())?;
        let mut indegree = region_blocks
            .iter()
            .copied()
            .map(|block| (block, 0_usize))
            .collect::<HashMap<_, _>>();
        for block in region_blocks.iter().copied() {
            for successor in successors[block].iter().copied() {
                if contains(successor) {
                    charge_convergence_work(work, 1)?;
                    let degree = indegree.get_mut(&successor).ok_or_else(|| {
                        PlironTensorLayoutFindingV1::ConvergenceAnalysisIncomplete {
                            detail: "control region membership is internally inconsistent"
                                .to_owned(),
                        }
                    })?;
                    *degree = degree.saturating_add(1);
                }
            }
        }
        let mut acyclic = indegree
            .iter()
            .filter_map(|(block, indegree)| (*indegree == 0).then_some(*block))
            .collect::<VecDeque<_>>();
        let mut consumed = 0_usize;
        while let Some(block) = acyclic.pop_front() {
            consumed += 1;
            for successor in successors[block].iter().copied() {
                if !contains(successor) {
                    continue;
                }
                charge_convergence_work(work, 1)?;
                let degree = indegree.get_mut(&successor).ok_or_else(|| {
                    PlironTensorLayoutFindingV1::ConvergenceAnalysisIncomplete {
                        detail: "control region membership is internally inconsistent".to_owned(),
                    }
                })?;
                *degree -= 1;
                if *degree == 0 {
                    acyclic.push_back(successor);
                }
            }
        }
        regions[controller] = Some(TensorControlRegionV1 {
            blocks,
            has_cycle: consumed != region_blocks.len(),
        });
    }
    Ok(regions)
}

fn tensor_trace(trace: &PlironInvocationTraceV1) -> Vec<PlironTraceLocationV1> {
    trace
        .events
        .iter()
        .filter_map(|event| match event {
            PlironTraceEventV1::TensorInstruction { location, .. } => Some(*location),
            PlironTraceEventV1::Barrier { .. }
            | PlironTraceEventV1::Fence { .. }
            | PlironTraceEventV1::Trap { .. }
            | PlironTraceEventV1::Memory { .. }
            | PlironTraceEventV1::CollectiveAllocation { .. } => None,
        })
        .collect()
}

fn report(findings: Vec<PlironTensorLayoutFindingV1>) -> PlironTensorLayoutReportV1 {
    PlironTensorLayoutReportV1 { findings }
}
