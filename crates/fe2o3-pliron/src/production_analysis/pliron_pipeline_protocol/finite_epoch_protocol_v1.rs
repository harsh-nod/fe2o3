#[derive(Clone)]
struct FiniteInnerLoopSummaryV1 {
    body_members: HashSet<usize>,
    inductions: HashMap<usize, Value>,
    bound: u64,
    latch: usize,
    exit: usize,
}

include!("finite_phase_coverage_v1.rs");

fn finite_loop_induction_v1(summary: &CanonicalEpochLoopV1, block: usize) -> Option<Value> {
    if block == summary.header {
        Some(summary.header_induction)
    } else {
        summary.inductions.get(&block).copied()
    }
}

// Discovery is a candidate search. This rechecks *every* edge, including an
// otherwise unmatched incoming argument, before a candidate gains proof use.
struct FiniteLoopEdgeFailureV1 {
    source: usize,
    target: usize,
    detail: &'static str,
}

fn finite_loop_edges_exact_v1(
    context: &Context,
    inventory: &BoundedPlironFunctionInventoryV1,
    summary: &CanonicalEpochLoopV1,
    successors: &[Vec<usize>],
    resources: &mut EquivalenceResourceMeterV1,
) -> Result<(), FiniteLoopEdgeFailureV1> {
    let inside = |block| block == summary.header || summary.body_members.contains(&block);
    for (source, edges) in successors.iter().enumerate() {
        for (ordinal, target) in edges.iter().copied().enumerate() {
            let reject = |detail| FiniteLoopEdgeFailureV1 {
                source,
                target,
                detail,
            };
            if !inside(source) {
                if inside(target) && (source != summary.entry || target != summary.header) {
                    return Err(reject("unproved loop side entry"));
                }
                continue;
            }
            if !inside(target) {
                if source != summary.header || target != summary.exit {
                    return Err(reject("unproved loop side exit"));
                }
                continue;
            }
            if target == summary.header && source != summary.latch {
                return Err(reject("non-latch backedge"));
            }
            let (Some(source_value), Some(target_value)) = (
                finite_loop_induction_v1(summary, source),
                finite_loop_induction_v1(summary, target),
            ) else {
                return Err(reject("missing exact induction value"));
            };
            let target_ref = inventory.blocks()[target].deref(context);
            let Some(position) = target_ref
                .arguments()
                .position(|value| value == target_value)
            else {
                return Err(reject("induction is not a destination block argument"));
            };
            let Some(terminator) = inventory.blocks()[source]
                .deref(context)
                .get_terminator(context)
            else {
                return Err(reject("missing source terminator"));
            };
            let Ok(control) = ControlViewV1::observe(context, terminator) else {
                return Err(reject("unsupported exact control shape"));
            };
            let Ok(edge) = control.edge(ordinal) else {
                return Err(reject("invalid edge occurrence"));
            };
            if edge.target() != inventory.blocks()[target] {
                return Err(reject("edge occurrence changed its destination"));
            }
            let Ok((value, destination)) = edge.argument_at(position) else {
                return Err(reject("missing exact edge argument"));
            };
            if destination != target_value {
                return Err(reject("edge argument changed its destination"));
            }
            let matches = if target == summary.header {
                index_offset(context, value, source_value, resources) == Some(1)
            } else {
                index_values_equivalent(context, value, source_value, resources)
            };
            if !matches {
                return Err(reject("edge argument changed the authenticated induction"));
            }
        }
    }
    Ok(())
}

fn finite_inner_operations_supported_v1(
    context: &Context,
    inventory: &BoundedPlironFunctionInventoryV1,
    summary: &CanonicalEpochLoopV1,
) -> bool {
    std::iter::once(summary.header)
        .chain(summary.body.iter().copied())
        .all(|block| {
            inventory.block_operations(block).iter().all(|site| {
                let operation = Operation::get_op_dyn(site.pointer(), context);
                operation.downcast_ref::<IndexConstantOp>().is_some()
                    || operation.downcast_ref::<IndexBinaryOp>().is_some()
                    || operation.downcast_ref::<IndexUnsignedCastOp>().is_some()
                    || operation.downcast_ref::<DeterministicJoinOp>().is_some()
                    || operation.downcast_ref::<BranchArgsOp>().is_some()
                    || operation
                        .downcast_ref::<dialect_kernel::BranchOp>()
                        .is_some()
                    || operation
                        .downcast_ref::<IndexLessThanBranchArgsOp>()
                        .is_some()
                    || operation.downcast_ref::<IndexEqualBranchArgsOp>().is_some()
                    || operation.downcast_ref::<AnalysisSplitOp>().is_some()
                    || operation.downcast_ref::<RankedAccessOp>().is_some()
                    || operation
                        .downcast_ref::<RequireEffectRefinementOp>()
                        .is_some()
            })
        })
}

fn finite_iteration_successors_v1(
    summary: &CanonicalEpochLoopV1,
    successors: &[Vec<usize>],
) -> Vec<Vec<usize>> {
    let mut contracted = successors.to_vec();
    for inner in &summary.finite_inner_loops {
        contracted[inner.latch] = vec![inner.exit];
    }
    contracted[summary.latch].clear();
    contracted
}

fn admit_finite_nested_epoch_loops_v1(
    context: &Context,
    inventory: &BoundedPlironFunctionInventoryV1,
    candidates: Vec<CanonicalEpochLoopV1>,
    successors: &[Vec<usize>],
    resources: &mut EquivalenceResourceMeterV1,
) -> Vec<CanonicalEpochLoopV1> {
    let finite = candidates
        .iter()
        .filter(|candidate| {
            !candidate.body.is_empty()
                && index_constant(context, candidate.bound).is_some_and(|bound| bound > 0)
                && finite_inner_operations_supported_v1(context, inventory, candidate)
        })
        .filter(|candidate| {
            finite_loop_edges_exact_v1(context, inventory, candidate, successors, resources).is_ok()
        })
        .collect::<Vec<_>>();
    let mut admitted = Vec::new();
    for candidate in &candidates {
        let mut outer = candidate.clone();
        if !outer.body.is_empty() {
            admitted.push(outer);
            continue;
        }
        let mut covered = HashSet::new();
        let mut overlap = false;
        for inner in &finite {
            if !outer.body_members.contains(&inner.header)
                || !inner
                    .body_members
                    .iter()
                    .all(|block| outer.body_members.contains(block))
            {
                continue;
            }
            for block in std::iter::once(inner.header).chain(inner.body_members.iter().copied()) {
                overlap |= !covered.insert(block);
            }
            outer.finite_inner_loops.push(FiniteInnerLoopSummaryV1 {
                body_members: inner.body_members.clone(),
                inductions: inner.inductions.clone(),
                bound: index_constant(context, inner.bound).expect("filtered finite bound"),
                latch: inner.latch,
                exit: inner.exit,
            });
        }
        if overlap || outer.finite_inner_loops.is_empty() {
            continue;
        }
        let contracted = finite_iteration_successors_v1(&outer, successors);
        if let Some(order) = finite_body_order_v1(&outer.body_members, &contracted) {
            outer.body = order;
            admitted.push(outer);
        }
    }
    admitted
}

fn finite_body_order_v1(members: &HashSet<usize>, successors: &[Vec<usize>]) -> Option<Vec<usize>> {
    let mut incoming = vec![0_usize; successors.len()];
    for (source, targets) in successors.iter().enumerate() {
        if !members.contains(&source) {
            continue;
        }
        for target in targets {
            if members.contains(target) {
                *incoming.get_mut(*target)? = incoming[*target].checked_add(1)?;
            }
        }
    }
    let mut order = (0..successors.len())
        .filter(|block| members.contains(block) && incoming[*block] == 0)
        .collect::<Vec<_>>();
    let mut cursor = 0;
    while cursor < order.len() {
        let source = order[cursor];
        cursor += 1;
        for target in &successors[source] {
            if members.contains(target) {
                incoming[*target] = incoming[*target].checked_sub(1)?;
                if incoming[*target] == 0 {
                    order.push(*target);
                }
            }
        }
    }
    (order.len() == members.len()).then_some(order)
}

/// Exact non-wrapping epoch shape for the finite two-phase schedule.
fn finite_epoch_offset_v1(
    context: &Context,
    mut value: Value,
    base: Value,
    resources: &mut EquivalenceResourceMeterV1,
) -> Option<u64> {
    let mut offset = 0_u64;
    resources.begin_query().ok()?;
    let mut steps = 0;
    for _ in 0..MAX_EQUIVALENCE_WORK_V1 {
        resources
            .charge_cursor(&mut steps, MAX_EQUIVALENCE_WORK_V1)
            .ok()?;
        let definition = value.defining_op()?;
        let operation = Operation::get_op_dyn(definition, context);
        let binary = operation.downcast_ref::<IndexBinaryOp>()?;
        let (constant, expression) =
            if let Some(constant) = index_constant(context, binary.lhs(context)) {
                (constant, binary.rhs(context))
            } else {
                (
                    index_constant(context, binary.rhs(context))?,
                    binary.lhs(context),
                )
            };
        match binary.kind(context)? {
            IndexBinaryKindAttr::Add => {
                offset = offset.checked_add(constant)?;
                value = expression;
            }
            IndexBinaryKindAttr::Multiply if constant == 2 => {
                return index_values_equivalent(context, expression, base, resources)
                    .then_some(offset);
            }
            _ => return None,
        }
    }
    None
}

fn finite_uniform_control_v1(
    context: &Context,
    pointer: Ptr<Operation>,
    roots: &HashSet<Value>,
    visit_limit: usize,
) -> bool {
    let operation = Operation::get_op_dyn(pointer, context);
    let dependencies = if let Some(branch) = operation.downcast_ref::<IndexLessThanBranchArgsOp>() {
        vec![branch.lhs(context), branch.rhs(context)]
    } else if let Some(branch) = operation.downcast_ref::<IndexEqualBranchArgsOp>() {
        vec![branch.lhs(context), branch.rhs(context)]
    } else if let Some(branch) = operation.downcast_ref::<dialect_kernel::IndexLessThanBranchOp>() {
        vec![branch.lhs(context), branch.rhs(context)]
    } else if let Some(branch) = operation.downcast_ref::<dialect_kernel::IndexEqualBranchOp>() {
        vec![branch.lhs(context), branch.rhs(context)]
    } else if let Some(branch) = operation.downcast_ref::<AnalysisSplitOp>() {
        branch.control_dependencies(context)
    } else {
        return false;
    };
    !dependencies.is_empty()
        && dependencies
            .into_iter()
            .all(|value| is_uniform_value(context, value, roots, visit_limit) == Ok(true))
}

// Collapse the whole bounded epoch loop to one collective word. Distinct
// successor words may be selected only by workgroup-uniform control. Unknown
// control is harmless only when every successor has the exact same word.
fn finite_uniform_participation_v1(
    context: &Context,
    inventory: &BoundedPlironFunctionInventoryV1,
    discovery: &EpochLoopDiscoveryV1,
    summary: &CanonicalEpochLoopV1,
    roots: &HashSet<Value>,
    visit_limit: usize,
) -> bool {
    let members = (0..inventory.blocks().len())
        .filter(|block| !summary.body_members.contains(block))
        .collect::<HashSet<_>>();
    let mut successors = discovery.cfg_successors.clone();
    successors[summary.header] = vec![summary.exit];
    let Some(order) = finite_body_order_v1(&members, &successors) else {
        return false;
    };
    let mut words = vec![None; successors.len()];
    for block in order.into_iter().rev() {
        let Some(terminator) = inventory.blocks()[block]
            .deref(context)
            .get_terminator(context)
        else {
            return false;
        };
        if ControlViewV1::observe(context, terminator).is_err() {
            return false;
        }
        let mut children = Vec::new();
        for target in &successors[block] {
            let Some(word) = words.get(*target).copied().flatten() else {
                return false;
            };
            children.push(word);
        }
        words[block] = if block == summary.header {
            (children.as_slice() == [0]).then_some(1)
        } else if children.is_empty() {
            Some(0)
        } else if children.iter().all(|word| *word == children[0]) {
            Some(children[0])
        } else if finite_uniform_control_v1(context, terminator, roots, visit_limit) {
            Some(block + 2)
        } else {
            return false;
        };
        if words[block].is_none() {
            return false;
        }
    }
    words.first().copied().flatten().is_some()
}

#[allow(clippy::too_many_arguments)]
fn verify_finite_two_phase_pipeline_v1(
    context: &Context,
    inventory: &BoundedPlironFunctionInventoryV1,
    discovery: &EpochLoopDiscoveryV1,
    pipeline: PlironOperationSiteV1,
    buffers: u32,
    distance: u32,
    schedule: &[EventSiteV1],
    accesses: &[AccessSiteV1],
    summary: &CanonicalEpochLoopV1,
    uniform_roots: &HashSet<Value>,
    uniformity_visit_limit: usize,
    resources: &mut EquivalenceResourceMeterV1,
) -> Result<PlironPipelineProtocolCertificateV1, PlironPipelineProtocolFindingV1> {
    let reject = |message: &str| invalid(pipeline, None, message);
    if buffers != 2
        || distance != 1
        || !index_constant(context, summary.bound)
            .is_some_and(|bound| bound > 0 && bound <= u64::MAX / 2)
    {
        return Err(reject(
            "finite two-phase loop lacks exact non-wrapping induction transport",
        ));
    }
    if let Err(failure) = finite_loop_edges_exact_v1(
        context,
        inventory,
        summary,
        &discovery.cfg_successors,
        resources,
    ) {
        return Err(reject(&format!(
            "finite loop edge {} -> {}: {}",
            failure.source, failure.target, failure.detail,
        )));
    }
    if !finite_uniform_participation_v1(
        context,
        inventory,
        discovery,
        summary,
        uniform_roots,
        uniformity_visit_limit,
    ) {
        return Err(reject(
            "finite pipeline loop lacks complete workgroup-uniform participation",
        ));
    }
    let positions = summary
        .body
        .iter()
        .enumerate()
        .map(|(position, block)| (*block, position))
        .collect::<HashMap<_, _>>();
    let mut events = schedule.to_vec();
    events.sort_by_key(|event| {
        (
            positions
                .get(&event.site.block())
                .copied()
                .unwrap_or(usize::MAX),
            event.site.operation(),
        )
    });
    let kinds = [
        PipelineEventKindAttr::Stage,
        PipelineEventKindAttr::Commit,
        PipelineEventKindAttr::Wait,
        PipelineEventKindAttr::Consume,
        PipelineEventKindAttr::Release,
    ];
    for (ordinal, event) in events.iter().enumerate() {
        let Some(induction) = summary.inductions.get(&event.site.block()).copied() else {
            return Err(reject("finite phase has no exact induction at its event"));
        };
        if event.kind != Some(kinds[ordinal % 5])
            || finite_epoch_offset_v1(context, event.epoch, induction, resources)
                != Some((ordinal / 5) as u64)
            || !slot_is_epoch_modulo(context, event.slot, event.epoch, buffers, resources)
        {
            return Err(invalid(
                pipeline,
                Some(event.site),
                "finite phase changed its event order, epoch, or slot",
            ));
        }
    }
    let event_ordinals = events
        .iter()
        .enumerate()
        .map(|(index, event)| (event.site.pointer(), index))
        .collect::<HashMap<_, _>>();
    let access_sites = accesses
        .iter()
        .map(|access| (access.site.pointer(), access))
        .collect::<HashMap<_, _>>();
    if event_ordinals.len() != 10 || access_sites.len() != accesses.len() {
        return Err(reject(
            "finite phase has duplicate physical operation occurrences",
        ));
    }
    let successors = finite_iteration_successors_v1(summary, &discovery.cfg_successors);
    let mut incoming = vec![None; inventory.blocks().len()];
    incoming[summary.body_start] = Some(0_usize);
    let mut phase_accesses = [Vec::new(), Vec::new()];
    let mut seen_accesses = 0;
    for block in &summary.body {
        let Some(mut cursor) = incoming[*block] else {
            return Err(reject("finite epoch contains an uncovered body path"));
        };
        for site in inventory.block_operations(*block) {
            if let Some(ordinal) = event_ordinals.get(&site.pointer()).copied() {
                if cursor != ordinal {
                    return Err(reject("finite epoch paths execute different phase traces"));
                }
                cursor += 1;
            }
            if let Some(access) = access_sites.get(&site.pointer()) {
                let expected = match cursor % 5 {
                    1 => AccessKindAttr::Write,
                    4 => AccessKindAttr::Read,
                    _ => return Err(reject("finite epoch access is outside its phase window")),
                };
                if cursor >= 10 || access.kind != expected {
                    return Err(reject("finite epoch access changes its phase effect kind"));
                }
                phase_accesses[cursor / 5].push((*access).clone());
                seen_accesses += 1;
            }
        }
        if *block == summary.latch {
            if cursor != 10 {
                return Err(reject("finite epoch latch skips a required phase"));
            }
            continue;
        }
        if successors[*block].is_empty() {
            return Err(reject("finite epoch terminates before its latch"));
        }
        for target in &successors[*block] {
            if !summary.body_members.contains(target) {
                return Err(reject("finite epoch has an unproved early exit"));
            }
            match incoming[*target] {
                Some(previous) if previous != cursor => {
                    return Err(reject("finite epoch paths skip or duplicate a phase"));
                }
                _ => incoming[*target] = Some(cursor),
            }
        }
    }
    if seen_accesses != accesses.len() {
        return Err(reject(
            "finite epoch has accesses outside the authenticated loop",
        ));
    }
    let mut writes = 0;
    let mut reads = 0;
    for (phase, accesses) in phase_accesses.iter().enumerate() {
        let base = phase * 5;
        let (phase_writes, phase_reads, refined) = verify_finite_phase_accesses_v1(
            context,
            inventory,
            FinitePhaseAccessInputV1 {
                pipeline,
                buffers,
                phase: phase as u64,
                summary,
                stage: events[base],
                commit: events[base + 1],
                consume: events[base + 3],
                release: events[base + 4],
                accesses,
                dominators: &discovery.dominators,
            },
            resources,
        )?;
        if !refined {
            return Err(reject("finite phase lacks initialized-coordinate coverage"));
        }
        writes += phase_writes;
        reads += phase_reads;
    }
    Ok(PlironPipelineProtocolCertificateV1 {
        pipeline_block: pipeline.block(),
        pipeline_operation: pipeline.operation(),
        buffers,
        prefetch_distance: distance,
        dynamic_loop: Some(EpochAwareLoopSummaryV1 {
            prologue: pipeline.block(),
            prologue_blocks: vec![pipeline.block()],
            header: summary.header,
            body: summary.body.clone(),
            exit: summary.exit,
            drain_blocks: summary.drain.clone(),
            induction: bounded_pipeline_value_name_v1(summary.header_induction, context),
            bound: bounded_pipeline_value_name_v1(summary.bound, context),
            step: 1,
            prefetched_epochs: 0,
            live_epoch_window: 1,
            drained_epochs: 0,
        }),
        concrete_epochs: 0,
        staged_writes: writes,
        consuming_reads: reads,
        access_refinement_proven: true,
    })
}
