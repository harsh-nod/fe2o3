#[cfg(test)]
pub(crate) fn run_pliron_progress_check_v1(
    context: &Context,
    function: &FuncOp,
) -> PlironProgressReportV1 {
    match catch_unwind(AssertUnwindSafe(|| {
        run_pliron_progress_check_inner_v1(context, function)
    })) {
        Ok(report) => report,
        Err(payload) => report(structural_rejection(format!(
            "bounded structural preflight panicked: {}",
            panic_detail(payload)
        ))),
    }
}

#[cfg(test)]
fn run_pliron_progress_check_inner_v1(
    context: &Context,
    function: &FuncOp,
) -> PlironProgressReportV1 {
    let inventory = match bounded_structural_inventory(context, function) {
        Ok(inventory) => inventory,
        Err(finding) => return report(finding),
    };
    let mut work = ProgressWorkBudgetV1::default();
    if let Err(finding) = work.charge(inventory.verification_work()) {
        return report(finding);
    }
    match catch_unwind(AssertUnwindSafe(|| {
        verify_operation(function.get_operation(), context)
    })) {
        Ok(Ok(())) => {}
        Ok(Err(error)) => {
            return report(structural_rejection(format!(
                "the PLIRON verifier rejected the function at {}",
                bounded_display_detail(error.disp(context))
            )));
        }
        Err(payload) => {
            return report(structural_rejection(format!(
                "the PLIRON verifier panicked: {}",
                panic_detail(payload)
            )));
        }
    }

    run_pliron_progress_after_verification_v1(context, inventory, work)
}

fn run_pliron_progress_after_verification_v1(
    context: &Context,
    inventory: StructuralInventoryV1,
    mut work: ProgressWorkBudgetV1,
) -> PlironProgressReportV1 {
    let blocks = inventory.root_blocks;
    let block_indices = blocks
        .iter()
        .enumerate()
        .map(|(index, block)| (*block, index))
        .collect::<HashMap<_, _>>();
    let graph = match build_root_graph(context, &blocks, &block_indices) {
        Ok(graph) => graph,
        Err(finding) => return report(finding),
    };
    let edge_count = graph.edges.iter().map(Vec::len).sum::<usize>();
    let graph_work = blocks
        .len()
        .checked_add(edge_count)
        .and_then(|units| units.checked_mul(8))
        .unwrap_or(usize::MAX);
    if let Err(finding) = work.charge(graph_work) {
        return report(finding);
    }

    let reachable = reachable_blocks(&graph.edges);
    let definitely_reachable = reachable_blocks(&graph.unconditional_edges);
    let dominators = progress_dominators_v1(&graph.edges, &graph.predecessors);
    let mut findings = Vec::new();
    let mut certificates = Vec::new();
    for mut component in strongly_connected_components(&graph.edges) {
        component.sort_unstable();
        let component_work = component.len().saturating_mul(4);
        if let Err(finding) = work.charge(component_work) {
            return report(finding);
        }
        if !component.iter().any(|block| reachable[*block]) || !is_cycle(&component, &graph.edges) {
            continue;
        }
        let component_members = component.iter().copied().collect::<HashSet<_>>();
        let has_exit = component.iter().any(|block| {
            graph.edges[*block]
                .iter()
                .any(|successor| !component_members.contains(successor))
        });
        if !has_exit {
            if component.iter().any(|block| definitely_reachable[*block]) {
                findings.push(PlironProgressFindingV1::NonTerminatingCycle {
                    blocks: component,
                    reason: "the strongly connected component has no exit edge",
                    counterexample: "an unconditional path from entry reaches the cycle, and every successor remains in it".to_owned(),
                });
            } else {
                findings.push(PlironProgressFindingV1::ProgressIncomplete {
                    blocks: component,
                    reason: "the exit-free cycle is only conditionally reachable and no feasible incoming witness was reconstructed",
                });
            }
            continue;
        }
        match canonical_positive_induction_loop(
            context,
            &blocks,
            &block_indices,
            &inventory.root_operation_blocks,
            &dominators,
            &graph.predecessors,
            &graph.edges,
            &graph.incoming,
            &component,
            &component_members,
        ) {
            CanonicalLoopResultV1::Proved(certificate) => certificates.push(certificate),
            CanonicalLoopResultV1::Inactive => {}
            CanonicalLoopResultV1::Rejected {
                reason,
                counterexample,
            } => {
                findings.push(PlironProgressFindingV1::NonTerminatingCycle {
                    blocks: component,
                    reason,
                    counterexample,
                });
            }
            CanonicalLoopResultV1::Incomplete(reason) => {
                match prove_nested_positive_induction_loops_v1(
                    context,
                    &blocks,
                    &block_indices,
                    &inventory.root_operation_blocks,
                    &dominators,
                    &graph.predecessors,
                    &graph.edges,
                    &component,
                    &component_members,
                ) {
                    Ok(mut nested) => certificates.append(&mut nested),
                    Err(()) => findings.push(PlironProgressFindingV1::ProgressIncomplete {
                        blocks: component,
                        reason,
                    }),
                }
            }
        }
    }
    PlironProgressReportV1 {
        findings,
        certificates,
    }
}

#[allow(clippy::too_many_arguments)]
fn prove_nested_positive_induction_loops_v1(
    context: &Context,
    blocks: &[Ptr<BasicBlock>],
    block_indices: &HashMap<Ptr<BasicBlock>, usize>,
    operation_blocks: &HashMap<Ptr<Operation>, usize>,
    dominators: &[HashSet<usize>],
    predecessors: &[Vec<usize>],
    edges: &[Vec<usize>],
    component: &[usize],
    component_members: &HashSet<usize>,
) -> Result<Vec<PlironProgressCertificateV1>, ()> {
    let mut backedges = Vec::new();
    for source in component.iter().copied() {
        for target in edges[source].iter().copied() {
            if component_members.contains(&target) && dominators[source].contains(&target) {
                backedges.push((source, target));
            }
        }
    }
    if backedges.is_empty() {
        return Err(());
    }

    let mut certificates = Vec::with_capacity(backedges.len());
    let mut proved_backedges = HashSet::new();
    for (latch, header_index) in backedges {
        let header = blocks[header_index];
        let header_ref = header.deref(context);
        let terminator = header_ref.get_terminator(context).ok_or(())?;
        let operation = Operation::get_op_dyn(terminator, context);
        let branch = operation
            .downcast_ref::<IndexLessThanBranchArgsOp>()
            .ok_or(())?;
        let induction = branch.lhs(context);
        let induction_argument = (0..header_ref.get_num_arguments())
            .find(|argument| header_ref.get_argument(*argument) == induction)
            .ok_or(())?;
        let successors = operation
            .get_operation()
            .deref(context)
            .successors()
            .collect::<Vec<_>>();
        let [body, exit] = successors.as_slice() else {
            return Err(());
        };
        let body_index = *block_indices.get(body).ok_or(())?;
        let exit_index = *block_indices.get(exit).ok_or(())?;

        let mut natural_loop = HashSet::from([header_index, latch]);
        let mut pending = vec![latch];
        while let Some(block) = pending.pop() {
            for predecessor in predecessors[block].iter().copied() {
                if predecessor != header_index
                    && dominators[predecessor].contains(&header_index)
                    && natural_loop.insert(predecessor)
                {
                    pending.push(predecessor);
                }
            }
        }
        if !natural_loop.contains(&body_index) || natural_loop.contains(&exit_index) {
            return Err(());
        }
        for block in natural_loop.iter().copied() {
            if block != header_index
                && predecessors[block]
                    .iter()
                    .any(|predecessor| !natural_loop.contains(predecessor))
            {
                return Err(());
            }
        }
        let entries = predecessors[header_index]
            .iter()
            .copied()
            .filter(|predecessor| !natural_loop.contains(predecessor))
            .collect::<Vec<_>>();
        if let [entry] = entries.as_slice() {
            let entry_arguments = progress_edge_arguments_v1(context, blocks[*entry], header)?;
            if entry_arguments
                .get(induction_argument)
                .and_then(|value| index_constant(context, *value))
                != Some(0)
            {
                return Err(());
            }
        } else {
            let initial = progress_multi_entry_initial_v1(
                context,
                blocks,
                block_indices,
                operation_blocks,
                dominators,
                &natural_loop,
                header_index,
                &entries,
                induction_argument,
            )?;
            if index_constant(context, initial) != Some(0) {
                return Err(());
            }
        }
        if branch
            .rhs(context)
            .defining_op()
            .and_then(|definition| operation_blocks.get(&definition).copied())
            .is_some_and(|block| natural_loop.contains(&block))
        {
            return Err(());
        }

        let inductions = propagate_loop_induction_v1(
            context,
            blocks,
            edges,
            &natural_loop,
            header_index,
            induction,
        )?;
        let latch_induction = *inductions.get(&latch).ok_or(())?;
        let latch_arguments = progress_edge_arguments_v1(context, blocks[latch], header)?;
        let next = *latch_arguments.get(induction_argument).ok_or(())?;
        let step = progress_index_offset_v1(context, next, latch_induction).ok_or(())?;
        if step == 0 {
            return Err(());
        }
        if step > 1 {
            let upper_bound = index_constant(context, branch.rhs(context))
                .or_else(|| unsigned_cast_upper_bound(context, branch.rhs(context)))
                .ok_or(())?;
            if upper_bound != 0 && (upper_bound - 1).checked_add(step).is_none() {
                return Err(());
            }
        }
        proved_backedges.insert((latch, header_index));
        certificates.push(PlironProgressCertificateV1 {
            header: header_index,
            body: body_index,
            exit: exit_index,
            induction: induction.id(context).into(),
            bound: branch.rhs(context).id(context).into(),
            step,
        });
    }
    if !is_acyclic_without_edges_v1(component_members, edges, &proved_backedges) {
        return Err(());
    }
    certificates.sort_by_key(|certificate| certificate.header);
    Ok(certificates)
}

fn progress_dominators_v1(
    edges: &[Vec<usize>],
    predecessors: &[Vec<usize>],
) -> Vec<HashSet<usize>> {
    let reachable = reachable_blocks(edges);
    let all = reachable
        .iter()
        .enumerate()
        .filter_map(|(block, reachable)| (*reachable).then_some(block))
        .collect::<HashSet<_>>();
    let mut dominators = vec![HashSet::new(); edges.len()];
    for block in all.iter().copied() {
        dominators[block] = all.clone();
    }
    if !edges.is_empty() {
        dominators[0] = HashSet::from([0]);
    }
    let mut changed = true;
    while changed {
        changed = false;
        for block in all.iter().copied().filter(|block| *block != 0) {
            let mut incoming = predecessors[block]
                .iter()
                .copied()
                .filter(|predecessor| reachable[*predecessor]);
            let Some(first) = incoming.next() else {
                continue;
            };
            let mut next = dominators[first].clone();
            for predecessor in incoming {
                next.retain(|dominator| dominators[predecessor].contains(dominator));
            }
            next.insert(block);
            if next != dominators[block] {
                dominators[block] = next;
                changed = true;
            }
        }
    }
    dominators
}

fn propagate_loop_induction_v1(
    context: &Context,
    blocks: &[Ptr<BasicBlock>],
    edges: &[Vec<usize>],
    members: &HashSet<usize>,
    header: usize,
    induction: pliron::value::Value,
) -> Result<HashMap<usize, pliron::value::Value>, ()> {
    let mut inductions = HashMap::from([(header, induction)]);
    for _ in 0..members.len() {
        let mut changed = false;
        for source in members.iter().copied().collect::<Vec<_>>() {
            let Some(source_induction) = inductions.get(&source).copied() else {
                continue;
            };
            for target in edges[source].iter().copied() {
                if target == header || !members.contains(&target) {
                    continue;
                }
                let arguments =
                    progress_edge_arguments_v1(context, blocks[source], blocks[target])?;
                let matching = arguments
                    .iter()
                    .enumerate()
                    .filter(|(_, argument)| **argument == source_induction)
                    .map(|(argument, _)| argument)
                    .collect::<Vec<_>>();
                let [argument] = matching.as_slice() else {
                    return Err(());
                };
                let target_induction = blocks[target].deref(context).get_argument(*argument);
                if let Some(existing) = inductions.get(&target) {
                    if *existing != target_induction {
                        return Err(());
                    }
                } else {
                    inductions.insert(target, target_induction);
                    changed = true;
                }
            }
        }
        if !changed {
            break;
        }
    }
    Ok(inductions)
}

fn progress_edge_arguments_v1(
    context: &Context,
    source: Ptr<BasicBlock>,
    target: Ptr<BasicBlock>,
) -> Result<Vec<pliron::value::Value>, ()> {
    #[cfg(test)]
    multi_entry_query_tests::record();
    let terminator = source.deref(context).get_terminator(context).ok_or(())?;
    let operation = Operation::get_op_dyn(terminator, context);
    let mut arguments = None;
    for (ordinal, successor) in operation
        .get_operation()
        .deref(context)
        .successors()
        .enumerate()
    {
        if successor != target {
            continue;
        }
        let candidate = successor_arguments(context, &operation, ordinal)
            .or_else(|| (target.deref(context).get_num_arguments() == 0).then(Vec::new))
            .ok_or(())?;
        // A block pair is a usable summary only when all parallel edges agree.
        if arguments.as_ref().is_some_and(|first| first != &candidate) {
            return Err(());
        }
        arguments = Some(candidate);
    }
    arguments.ok_or(())
}

#[cfg(test)]
mod multi_entry_query_tests {
    include!("multi_entry_query_v1_tests.rs");
}

fn progress_index_offset_v1(
    context: &Context,
    value: pliron::value::Value,
    base: pliron::value::Value,
) -> Option<u64> {
    if value == base {
        return Some(0);
    }
    let definition = value.defining_op()?;
    let operation = Operation::get_op_dyn(definition, context);
    let add = operation.downcast_ref::<IndexBinaryOp>()?;
    if add.kind(context) != Some(IndexBinaryKindAttr::Add) {
        return None;
    }
    if add.lhs(context) == base {
        index_constant(context, add.rhs(context))
    } else if add.rhs(context) == base {
        index_constant(context, add.lhs(context))
    } else {
        None
    }
}

fn is_acyclic_without_edges_v1(
    members: &HashSet<usize>,
    edges: &[Vec<usize>],
    removed: &HashSet<(usize, usize)>,
) -> bool {
    let mut incoming = members
        .iter()
        .copied()
        .map(|block| (block, 0_usize))
        .collect::<HashMap<_, _>>();
    for source in members.iter().copied() {
        for target in edges[source].iter().copied() {
            if members.contains(&target) && !removed.contains(&(source, target)) {
                let Some(count) = incoming.get_mut(&target) else {
                    return false;
                };
                *count += 1;
            }
        }
    }
    let mut ready = incoming
        .iter()
        .filter_map(|(block, count)| (*count == 0).then_some(*block))
        .collect::<Vec<_>>();
    let mut visited = 0_usize;
    while let Some(source) = ready.pop() {
        visited += 1;
        for target in edges[source].iter().copied() {
            if !members.contains(&target) || removed.contains(&(source, target)) {
                continue;
            }
            let Some(count) = incoming.get_mut(&target) else {
                return false;
            };
            let Some(next) = count.checked_sub(1) else {
                return false;
            };
            *count = next;
            if next == 0 {
                ready.push(target);
            }
        }
    }
    visited == members.len()
}
