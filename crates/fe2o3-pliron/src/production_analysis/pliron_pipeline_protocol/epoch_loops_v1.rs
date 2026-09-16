fn discover_epoch_loops(
    context: &Context,
    inventory: &BoundedPlironFunctionInventoryV1,
    equivalence_resources: &mut EquivalenceResourceMeterV1,
) -> EpochLoopDiscoveryV1 {
    let block_indices = inventory
        .blocks()
        .iter()
        .enumerate()
        .map(|(index, block)| (*block, index))
        .collect::<HashMap<_, _>>();
    let mut predecessors = vec![Vec::new(); inventory.blocks().len()];
    let mut cfg_successors = vec![Vec::new(); inventory.blocks().len()];
    for (source, block) in inventory.blocks().iter().copied().enumerate() {
        let Some(terminator) = block.deref(context).get_terminator(context) else {
            continue;
        };
        for successor in terminator.deref(context).successors() {
            if let Some(target) = block_indices.get(&successor).copied() {
                predecessors[target].push(source);
                cfg_successors[source].push(target);
            }
        }
    }
    let dominators = pipeline_dominators_v1(&cfg_successors, &predecessors);
    let mut loops = Vec::new();
    'headers: for (header_index, header) in inventory.blocks().iter().copied().enumerate() {
        let header_ref = header.deref(context);
        let Some(terminator) = header_ref.get_terminator(context) else {
            continue;
        };
        let operation = Operation::get_op_dyn(terminator, context);
        let Some(branch) = operation.downcast_ref::<IndexLessThanBranchArgsOp>() else {
            continue;
        };
        let induction = branch.lhs(context);
        let Some(induction_argument) = (0..header_ref.get_num_arguments())
            .find(|argument| header_ref.get_argument(*argument) == induction)
        else {
            continue;
        };
        let successors = operation
            .get_operation()
            .deref(context)
            .successors()
            .collect::<Vec<_>>();
        let [body_block, exit_block] = successors.as_slice() else {
            continue;
        };
        let (Some(body_start), Some(exit)) = (
            block_indices.get(body_block).copied(),
            block_indices.get(exit_block).copied(),
        ) else {
            continue;
        };
        let latches = predecessors[header_index]
            .iter()
            .copied()
            .filter(|predecessor| {
                *predecessor != header_index && dominators[*predecessor].contains(&header_index)
            })
            .collect::<Vec<_>>();
        let [latch] = latches.as_slice() else {
            continue;
        };
        let mut natural_loop = HashSet::from([header_index, *latch]);
        let mut pending = vec![*latch];
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
        if !natural_loop.contains(&body_start) || natural_loop.contains(&exit) {
            continue;
        }
        if predecessors[exit].as_slice() != [header_index] {
            continue;
        }
        let external = predecessors[header_index]
            .iter()
            .copied()
            .filter(|predecessor| !natural_loop.contains(predecessor))
            .collect::<Vec<_>>();
        let [entry] = external.as_slice() else {
            continue;
        };
        let Some(entry_arguments) = edge_arguments_v1(context, inventory.blocks()[*entry], header)
        else {
            continue;
        };
        if entry_arguments
            .get(induction_argument)
            .and_then(|value| index_constant(context, *value))
            != Some(0)
        {
            continue;
        }

        let mut inductions = HashMap::from([(header_index, induction)]);
        for _ in 0..natural_loop.len() {
            let mut changed = false;
            for source in natural_loop.iter().copied().collect::<Vec<_>>() {
                let Some(source_induction) = inductions.get(&source).copied() else {
                    continue;
                };
                let Some(terminator) = inventory.blocks()[source]
                    .deref(context)
                    .get_terminator(context)
                else {
                    continue;
                };
                for successor in terminator.deref(context).successors() {
                    let Some(target) = block_indices.get(&successor).copied() else {
                        continue;
                    };
                    if target == header_index || !natural_loop.contains(&target) {
                        continue;
                    }
                    let Some(arguments) =
                        edge_arguments_v1(context, inventory.blocks()[source], successor)
                    else {
                        continue;
                    };
                    let matching = arguments
                        .iter()
                        .enumerate()
                        .filter(|(_, argument)| {
                            index_values_equivalent(
                                context,
                                **argument,
                                source_induction,
                                equivalence_resources,
                            )
                        })
                        .map(|(argument, _)| argument)
                        .collect::<Vec<_>>();
                    let [argument] = matching.as_slice() else {
                        continue;
                    };
                    let target_ref = successor.deref(context);
                    if *argument >= target_ref.get_num_arguments() {
                        continue;
                    }
                    let target_induction = target_ref.get_argument(*argument);
                    match inductions.get(&target).copied() {
                        Some(existing) if existing != target_induction => {
                            continue 'headers;
                        }
                        Some(_) => {}
                        None => {
                            inductions.insert(target, target_induction);
                            changed = true;
                        }
                    }
                }
            }
            if !changed {
                break;
            }
        }
        let Some(latch_induction) = inductions.get(latch).copied() else {
            continue;
        };
        let Some(latch_arguments) = edge_arguments_v1(context, inventory.blocks()[*latch], header)
        else {
            continue;
        };
        if latch_arguments
            .get(induction_argument)
            .and_then(|value| index_offset(context, *value, latch_induction, equivalence_resources))
            != Some(1)
        {
            continue;
        }

        let body_members = natural_loop
            .iter()
            .copied()
            .filter(|block| *block != header_index)
            .collect::<HashSet<_>>();
        let body = acyclic_loop_body_order_v1(&body_members, &cfg_successors).unwrap_or_default();
        let mut prologue = vec![*entry];
        let mut current = *entry;
        while let [predecessor] = predecessors[current].as_slice() {
            if *predecessor == header_index
                || body_members.contains(predecessor)
                || predecessors[*predecessor].len() > 1
                || prologue.contains(predecessor)
            {
                break;
            }
            prologue.push(*predecessor);
            current = *predecessor;
        }
        prologue.reverse();

        let mut drain = Vec::new();
        let mut current = exit;
        loop {
            if drain.contains(&current) {
                break;
            }
            drain.push(current);
            let [successor] = cfg_successors[current].as_slice() else {
                break;
            };
            if *successor == header_index
                || body_members.contains(successor)
                || predecessors[*successor].len() > 1
            {
                break;
            }
            current = *successor;
        }
        inductions.remove(&header_index);
        loops.push(CanonicalEpochLoopV1 {
            entry: *entry,
            body_start,
            latch: *latch,
            exit,
            prologue,
            header: header_index,
            body,
            body_members,
            drain,
            inductions,
            header_induction: induction,
            bound: branch.rhs(context),
            finite_inner_loops: Vec::new(),
        });
    }
    let loops = admit_finite_nested_epoch_loops_v1(
        context, inventory, loops, &cfg_successors, equivalence_resources,
    );
    EpochLoopDiscoveryV1 {
        loops,
        dominators,
        cfg_successors,
    }
}

fn pipeline_dominators_v1(
    successors: &[Vec<usize>],
    predecessors: &[Vec<usize>],
) -> Vec<HashSet<usize>> {
    let mut reachable = vec![false; successors.len()];
    let mut pending = (!successors.is_empty())
        .then_some(0)
        .into_iter()
        .collect::<Vec<_>>();
    while let Some(block) = pending.pop() {
        if reachable[block] {
            continue;
        }
        reachable[block] = true;
        pending.extend(successors[block].iter().copied());
    }
    let all = reachable
        .iter()
        .enumerate()
        .filter_map(|(block, reachable)| (*reachable).then_some(block))
        .collect::<HashSet<_>>();
    let mut dominators = vec![HashSet::new(); successors.len()];
    for block in all.iter().copied() {
        dominators[block] = all.clone();
    }
    if !successors.is_empty() {
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

fn edge_arguments_v1(
    context: &Context,
    source: Ptr<BasicBlock>,
    target: Ptr<BasicBlock>,
) -> Option<Vec<Value>> {
    let terminator = source.deref(context).get_terminator(context)?;
    let operation = Operation::get_op_dyn(terminator, context);
    let successor = operation
        .get_operation()
        .deref(context)
        .successors()
        .position(|successor| successor == target)?;
    if operation
        .downcast_ref::<dialect_gpu::optimization_v1::CondBranchOp>()
        .is_some()
    {
        let raw = terminator.deref(context);
        // This legacy query identifies a target, not an edge occurrence. Do
        // not choose one of two distinct Boolean payloads to the same target.
        if raw.get_num_successors() != 2 || raw.get_successor(0) == raw.get_successor(1) {
            return None;
        }
        let first = raw.get_successor(0).deref(context).get_num_arguments();
        let second = raw.get_successor(1).deref(context).get_num_arguments();
        if raw.get_num_operands() != 1 + first + second {
            return None;
        }
        let (start, count) = if successor == 0 {
            (1, first)
        } else {
            (1 + first, second)
        };
        return Some(
            (start..start + count)
                .map(|index| raw.get_operand(index))
                .collect(),
        );
    }
    if let Some(branch) = operation.downcast_ref::<BranchArgsOp>() {
        return (successor == 0).then(|| branch.arguments(context));
    };
    if let Some(branch) = operation.downcast_ref::<IndexLessThanBranchArgsOp>() {
        return match successor {
            0 => Some(branch.true_arguments(context)),
            1 => Some(branch.false_arguments(context)),
            _ => None,
        };
    }
    if let Some(branch) = operation.downcast_ref::<IndexEqualBranchArgsOp>() {
        return match successor {
            0 => Some(branch.true_arguments(context)),
            1 => Some(branch.false_arguments(context)),
            _ => None,
        };
    }
    if let Some(split) = operation.downcast_ref::<AnalysisSplitOp>() {
        return match successor {
            0 => Some(split.first_arguments(context)),
            1 => Some(split.second_arguments(context)),
            _ => None,
        };
    }
    (target.deref(context).get_num_arguments() == 0).then(Vec::new)
}

fn acyclic_loop_body_order_v1(
    members: &HashSet<usize>,
    successors: &[Vec<usize>],
) -> Option<Vec<usize>> {
    let mut incoming = members
        .iter()
        .copied()
        .map(|block| (block, 0_usize))
        .collect::<HashMap<_, _>>();
    for source in members.iter().copied() {
        for target in &successors[source] {
            if members.contains(target) {
                *incoming.get_mut(target)? += 1;
            }
        }
    }
    let mut ready = incoming
        .iter()
        .filter_map(|(block, incoming)| (*incoming == 0).then_some(*block))
        .collect::<Vec<_>>();
    ready.sort_unstable_by(|left, right| right.cmp(left));
    let mut ordered = Vec::with_capacity(members.len());
    while let Some(source) = ready.pop() {
        ordered.push(source);
        for target in successors[source].iter().copied() {
            if !members.contains(&target) {
                continue;
            }
            let count = incoming.get_mut(&target)?;
            *count = count.checked_sub(1)?;
            if *count == 0 {
                ready.push(target);
                ready.sort_unstable_by(|left, right| right.cmp(left));
            }
        }
    }
    (ordered.len() == members.len()).then_some(ordered)
}

fn slot_is_epoch_modulo(
    context: &Context,
    slot: Value,
    epoch: Value,
    buffers: u32,
    equivalence_resources: &mut EquivalenceResourceMeterV1,
) -> bool {
    if let (Some(slot), Some(epoch)) = (
        index_constant(context, slot),
        index_constant(context, epoch),
    ) {
        return slot == epoch % u64::from(buffers);
    }
    let Some(definition) = slot.defining_op() else {
        return false;
    };
    let operation = Operation::get_op_dyn(definition, context);
    let Some(remainder) = operation.downcast_ref::<IndexBinaryOp>() else {
        return false;
    };
    remainder.kind(context) == Some(IndexBinaryKindAttr::Remainder)
        && index_values_equivalent(
            context,
            remainder.lhs(context),
            epoch,
            equivalence_resources,
        )
        && index_constant(context, remainder.rhs(context)) == Some(u64::from(buffers))
}

struct PipelineBlockValueV1 {
    value: Value,
    block: usize,
}

fn slot_is_epoch_modulo_across_loop_blocks(
    context: &Context,
    slot: PipelineBlockValueV1,
    epoch: PipelineBlockValueV1,
    buffers: u32,
    summary: &CanonicalEpochLoopV1,
    equivalence_resources: &mut EquivalenceResourceMeterV1,
) -> bool {
    let PipelineBlockValueV1 {
        value: slot,
        block: slot_block,
    } = slot;
    let PipelineBlockValueV1 {
        value: epoch,
        block: epoch_block,
    } = epoch;
    if let (Some(slot), Some(epoch)) = (
        index_constant(context, slot),
        index_constant(context, epoch),
    ) {
        return slot == epoch % u64::from(buffers);
    }
    let Some(definition) = slot.defining_op() else {
        return false;
    };
    let operation = Operation::get_op_dyn(definition, context);
    let Some(remainder) = operation.downcast_ref::<IndexBinaryOp>() else {
        return false;
    };
    remainder.kind(context) == Some(IndexBinaryKindAttr::Remainder)
        && index_values_equivalent_across_loop_blocks(
            context,
            remainder.lhs(context),
            slot_block,
            epoch,
            epoch_block,
            summary,
            equivalence_resources,
        )
        && index_constant(context, remainder.rhs(context)) == Some(u64::from(buffers))
}

fn index_values_equivalent_across_loop_blocks(
    context: &Context,
    left: Value,
    left_block: usize,
    right: Value,
    right_block: usize,
    summary: &CanonicalEpochLoopV1,
    equivalence_resources: &mut EquivalenceResourceMeterV1,
) -> bool {
    if index_values_equivalent(context, left, right, equivalence_resources) {
        return true;
    }
    let Some(left_induction) = summary.inductions.get(&left_block).copied() else {
        return false;
    };
    let Some(right_induction) = summary.inductions.get(&right_block).copied() else {
        return false;
    };
    match (
        index_offset(context, left, left_induction, equivalence_resources),
        index_offset(context, right, right_induction, equivalence_resources),
    ) {
        (Some(left_offset), Some(right_offset)) => left_offset == right_offset,
        _ => false,
    }
}
