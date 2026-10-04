fn analyze_control_flow_shared_v1(
    function: &Function,
    limits: ControlFlowLimits,
    resources: &mut ControlFlowResourcesV1<'_, '_>,
) -> Result<IndexedControlFlow, MeteredControlFlowErrorV1> {
    resources.charge(1)?;
    let body = function
        .body
        .as_ref()
        .ok_or(ControlFlowError::EmptyFunction)?;
    check_limit(
        ControlFlowResource::Blocks,
        body.blocks.len(),
        limits.blocks,
    )?;
    if body.blocks.is_empty() {
        return Err(ControlFlowError::EmptyFunction.into());
    }
    let blocks = body.blocks.len();
    let mut block_ids = resources.allocate(blocks, 1)?;
    let mut block_positions = resources.allocate(blocks, 2)?;
    resources.charge(blocks)?;
    for (position, block) in body.blocks.iter().enumerate() {
        block_ids.push(block.id);
        block_positions.push((block.id, position));
    }
    resources.sort_blocks(&mut block_positions)?;

    // Sorting is stable by physical ordinal. Retain the earliest second
    // occurrence, then replay the original per-block diagnostic precedence.
    let mut first_duplicate = None;
    resources.charge(block_positions.len())?;
    for pair in block_positions.windows(2) {
        if pair[0].0 == pair[1].0 {
            let candidate = (pair[1].1, pair[1].0);
            if first_duplicate.is_none_or(|previous| candidate < previous) {
                first_duplicate = Some(candidate);
            }
        }
    }
    resources.charge(blocks)?;
    for (position, block) in body.blocks.iter().enumerate() {
        if first_duplicate.is_some_and(|duplicate| duplicate.0 == position) {
            return Err(ControlFlowError::DuplicateBlock(block.id).into());
        }
        if block.terminator.is_none() {
            return Err(ControlFlowError::MissingTerminator(block.id).into());
        }
    }

    let mut edge_count = 0;
    let mut edge_arguments = 0;
    let mut incoming_counts = resources.filled(blocks, 0_usize, 1)?;
    resources.charge(blocks)?;
    for block in &body.blocks {
        let terminator = block
            .terminator
            .as_ref()
            .ok_or(ControlFlowError::MissingTerminator(block.id))?;
        for_each_terminator_edge(
            terminator,
            |target, arguments| -> Result<(), MeteredControlFlowErrorV1> {
                resources.charge(4)?;
                edge_count = checked_add(ControlFlowResource::Edges, edge_count, 1)?;
                edge_arguments = checked_add(
                    ControlFlowResource::EdgeArguments,
                    edge_arguments,
                    arguments.len(),
                )?;
                let Some(target_position) = resources.block_position(&block_positions, target)?
                else {
                    return Err(ControlFlowError::UnknownSuccessor {
                        source: block.id,
                        target,
                    }
                    .into());
                };
                incoming_counts[target_position] = checked_add(
                    ControlFlowResource::Edges,
                    incoming_counts[target_position],
                    1,
                )?;
                Ok(())
            },
        )?;
    }
    check_limit(ControlFlowResource::Edges, edge_count, limits.edges)?;
    check_limit(
        ControlFlowResource::EdgeArguments,
        edge_arguments,
        limits.edge_arguments,
    )?;
    let mut phi_inputs = 0;
    resources.charge(blocks)?;
    for (block, incoming) in body.blocks.iter().zip(&incoming_counts) {
        let block_phi_inputs = checked_mul(
            ControlFlowResource::PhiInputs,
            block.parameters.len(),
            *incoming,
        )?;
        phi_inputs = checked_add(ControlFlowResource::PhiInputs, phi_inputs, block_phi_inputs)?;
    }
    check_limit(
        ControlFlowResource::PhiInputs,
        phi_inputs,
        limits.phi_inputs,
    )?;

    let mut edges = resources.allocate(edge_count, 4)?;
    let mut outgoing = resources.allocate(blocks, 2)?;
    resources.charge(blocks)?;
    for (source, block) in body.blocks.iter().enumerate() {
        let start = edges.len();
        let mut ordinal = 0;
        for_each_terminator_edge(
            block
                .terminator
                .as_ref()
                .ok_or(ControlFlowError::MissingTerminator(block.id))?,
            |target, arguments| -> Result<(), MeteredControlFlowErrorV1> {
                resources.charge(4)?;
                let target = resources.block_position(&block_positions, target)?.ok_or(
                    ControlFlowError::UnknownSuccessor {
                        source: block.id,
                        target,
                    },
                )?;
                edges.push(IndexedControlFlowEdge {
                    source,
                    target,
                    ordinal,
                    argument_count: arguments.len(),
                });
                ordinal += 1;
                Ok(())
            },
        )?;
        outgoing.push(start..edges.len());
    }
    let mut incoming = resources.rows(&incoming_counts)?;
    resources.charge(edges.len())?;
    for (edge_index, edge) in edges.iter().enumerate() {
        incoming[edge.target].push(edge_index);
    }
    resources.free(incoming_counts, blocks, 1)?;

    let mut successors = resources.allocate(blocks, 1)?;
    // Each successor row is also dropped once, including rejecting paths.
    resources.charge(checked_mul(ControlFlowResource::AnalysisWork, blocks, 2)?)?;
    for range in &outgoing {
        let mut targets = resources.allocate(range.len(), 1)?;
        resources.charge(range.len())?;
        for edge in &edges[range.clone()] {
            targets.push(edge.target);
        }
        resources.sort(&mut targets)?;
        resources.charge(targets.len())?;
        targets.dedup();
        successors.push(targets);
    }
    let mut predecessor_counts = resources.filled(blocks, 0_usize, 1)?;
    resources.charge(blocks)?;
    for targets in &successors {
        resources.charge(targets.len())?;
        for target in targets {
            predecessor_counts[*target] += 1;
        }
    }
    let mut predecessors = resources.rows(&predecessor_counts)?;
    resources.charge(blocks)?;
    for (source, targets) in successors.iter().enumerate() {
        resources.charge(targets.len())?;
        for target in targets {
            predecessors[*target].push(source);
        }
    }
    resources.free(predecessor_counts, blocks, 1)?;

    let mut meter = WorkMeter::new(limits.analysis_work);
    meter.charge_index(
        u64::try_from(blocks)
            .unwrap_or(u64::MAX)
            .saturating_add(
                u64::try_from(edge_count)
                    .unwrap_or(u64::MAX)
                    .saturating_mul(2),
            )
            .saturating_add(u64::try_from(phi_inputs).unwrap_or(u64::MAX)),
    )?;
    let reachable = compute_reachable(&successors, &mut meter, resources)?;
    let reverse_postorder =
        compute_reverse_postorder(&successors, &reachable, &mut meter, resources)?;
    let reverse_postorder_capacity = reverse_postorder.len();
    let immediate_dominators = compute_immediate_dominators(
        &predecessors,
        &reachable,
        &reverse_postorder,
        &mut meter,
        resources,
    )?;
    resources.free(reverse_postorder, reverse_postorder_capacity, 1)?;
    let (dominator_preorder, dominator_postorder) =
        compute_dominator_intervals(&immediate_dominators, &reachable, &mut meter, resources)?;
    resources.free(immediate_dominators, blocks, 2)?;
    let irreducible_blocks = compute_irreducible_blocks(
        &block_ids,
        &successors,
        &reachable,
        &dominator_preorder,
        &dominator_postorder,
        &mut meter,
        resources,
    )?;
    Ok(IndexedControlFlow {
        block_ids,
        block_positions,
        edges,
        outgoing,
        incoming,
        successors,
        predecessors,
        reachable,
        dominator_preorder,
        dominator_postorder,
        irreducible_blocks,
        edge_arguments,
        phi_inputs,
        work: meter.work,
    })
}
