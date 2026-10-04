use crate::production_analysis::pliron_control_edges_v1::ControlViewV1;

struct PipelineControlContextV1<'a> {
    inventory: &'a BoundedPlironFunctionInventoryV1,
    discovery: &'a EpochLoopDiscoveryV1,
    concrete: Option<Result<ConcreteCfgV1, &'static str>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ConcreteExitV1 {
    Branch,
    Return,
    Trap,
}

struct ConcreteCfgV1 {
    reachable: Vec<bool>,
    order: Vec<usize>,
    exits: Vec<ConcreteExitV1>,
    offsets: Vec<usize>,
}

impl ConcreteCfgV1 {
    fn observe(
        context: &Context,
        inventory: &BoundedPlironFunctionInventoryV1,
        successors: &[Vec<usize>],
    ) -> Result<Self, &'static str> {
        let blocks = inventory.blocks();
        if blocks.is_empty() || successors.len() != blocks.len() {
            return Err("cross-block concrete pipeline has an incomplete CFG roster");
        }
        let mut exits = Vec::with_capacity(blocks.len());
        let mut offsets = Vec::with_capacity(blocks.len() + 1);
        offsets.push(0_usize);
        for (block, pointer) in blocks.iter().copied().enumerate() {
            let terminator = pointer
                .deref(context)
                .get_terminator(context)
                .ok_or("cross-block concrete pipeline has a block without a terminator")?;
            let control = ControlViewV1::observe(context, terminator).map_err(
                |_| "cross-block concrete pipeline has unsupported or malformed control",
            )?;
            if control.successor_count() != successors[block].len() {
                return Err("cross-block concrete pipeline has an incomplete successor roster");
            }
            for (ordinal, target) in successors[block].iter().copied().enumerate() {
                let edge = control
                    .edge(ordinal)
                    .map_err(|_| "cross-block concrete pipeline has a malformed edge occurrence")?;
                if blocks.get(target).copied() != Some(edge.target()) {
                    return Err(
                        "cross-block concrete pipeline has a foreign or reordered successor",
                    );
                }
            }
            exits.push(if control.successor_count() != 0 {
                ConcreteExitV1::Branch
            } else if Operation::get_op::<dialect_kernel::TrapOp>(terminator, context).is_some() {
                ConcreteExitV1::Trap
            } else if Operation::get_op::<dialect_kernel::ReturnOp>(terminator, context).is_some()
                || Operation::get_op::<dialect_gpu::optimization_v1::ReturnOp>(terminator, context)
                    .is_some()
            {
                ConcreteExitV1::Return
            } else {
                return Err("cross-block concrete pipeline has an unrecognized exit");
            });
            let next = offsets[block]
                .checked_add(inventory.block_operations(block).len())
                .ok_or("cross-block concrete pipeline operation offsets overflow")?;
            offsets.push(next);
        }
        if offsets.last().copied() != Some(inventory.operations().len()) {
            return Err("cross-block concrete pipeline operation roster is incomplete");
        }

        let mut reachable = vec![false; blocks.len()];
        let mut pending = Vec::with_capacity(blocks.len());
        reachable[0] = true;
        pending.push(0);
        while let Some(block) = pending.pop() {
            for target in successors[block].iter().copied() {
                if !reachable[target] {
                    reachable[target] = true;
                    pending.push(target);
                }
            }
        }
        let mut indegrees = vec![0_usize; blocks.len()];
        for (block, edges) in successors.iter().enumerate() {
            if reachable[block] {
                for target in edges.iter().copied() {
                    indegrees[target] = indegrees[target]
                        .checked_add(1)
                        .ok_or("cross-block concrete pipeline edge count overflows")?;
                }
            }
        }
        let mut order = Vec::with_capacity(blocks.len());
        for block in 0..blocks.len() {
            if reachable[block] && indegrees[block] == 0 {
                order.push(block);
            }
        }
        let mut next = 0;
        while next < order.len() {
            let block = order[next];
            next += 1;
            // Every edge occurrence contributes and removes one indegree unit.
            for target in successors[block].iter().copied() {
                indegrees[target] -= 1;
                if indegrees[target] == 0 {
                    order.push(target);
                }
            }
        }
        if order.len() != reachable.iter().filter(|value| **value).count() {
            return Err("cross-block concrete pipeline has a reachable cycle or creation re-entry");
        }
        Ok(Self {
            reachable,
            order,
            exits,
            offsets,
        })
    }

    fn site_index(
        &self,
        inventory: &BoundedPlironFunctionInventoryV1,
        site: PlironOperationSiteV1,
    ) -> Result<usize, &'static str> {
        if site.block() >= inventory.blocks().len()
            || inventory
                .block_operations(site.block())
                .get(site.operation())
                .copied()
                != Some(site)
        {
            return Err("cross-block concrete pipeline has a foreign operation occurrence");
        }
        Ok(self.offsets[site.block()] + site.operation())
    }
}

#[derive(Clone, Copy)]
enum ConcreteActionV1<'a> {
    Event(&'a EventSiteV1),
    Access(&'a AccessSiteV1),
}

fn verify_cross_block_concrete_trace_v1<'a>(
    context: &Context,
    control: &mut PipelineControlContextV1<'_>,
    pipeline: PlironOperationSiteV1,
    schedule: &'a [EventSiteV1],
    accesses: &'a [AccessSiteV1],
) -> Result<Vec<ConcreteActionV1<'a>>, PlironPipelineProtocolFindingV1> {
    let cfg = control.concrete.get_or_insert_with(|| {
        ConcreteCfgV1::observe(
            context,
            control.inventory,
            &control.discovery.cfg_successors,
        )
    });
    let cfg = cfg
        .as_ref()
        .map_err(|detail| invalid(pipeline, None, detail))?;
    if cfg.reachable.get(pipeline.block()) != Some(&true)
        || !pipeline_creation_dominates_schedule(
            &control.discovery.dominators,
            pipeline,
            schedule,
            accesses,
        )
    {
        return Err(invalid(
            pipeline,
            None,
            "cross-block concrete pipeline creation is unreachable or does not dominate every event and access",
        ));
    }
    let count = schedule
        .len()
        .checked_add(accesses.len())
        .and_then(|count| count.checked_add(1))
        .ok_or_else(|| {
            invalid(
                pipeline,
                None,
                "cross-block concrete pipeline site count overflows",
            )
        })?;
    let mut ordinals = vec![usize::MAX; control.inventory.operations().len()];
    for (token, site) in std::iter::once(pipeline)
        .chain(schedule.iter().map(|event| event.site))
        .chain(accesses.iter().map(|access| access.site))
        .enumerate()
    {
        let index = cfg
            .site_index(control.inventory, site)
            .map_err(|detail| invalid(pipeline, Some(site), detail))?;
        if ordinals[index] != usize::MAX {
            return Err(invalid(
                pipeline,
                Some(site),
                "cross-block concrete pipeline registers one physical occurrence twice",
            ));
        }
        ordinals[index] = token;
    }
    let mut actions = Vec::with_capacity(count - 1);
    let mut next = 0;
    for block in cfg.order.iter().copied() {
        for ordinal in &mut ordinals[cfg.offsets[block]..cfg.offsets[block + 1]] {
            let token = *ordinal;
            if token == usize::MAX {
                continue;
            }
            if token == 0 {
                if next != 0 {
                    return Err(invalid(
                        pipeline,
                        None,
                        "cross-block concrete pipeline has an action before its creation",
                    ));
                }
            } else if token <= schedule.len() {
                actions.push(ConcreteActionV1::Event(&schedule[token - 1]));
            } else {
                actions.push(ConcreteActionV1::Access(
                    &accesses[token - schedule.len() - 1],
                ));
            }
            *ordinal = next;
            next += 1;
        }
    }
    if next != count {
        return Err(invalid(
            pipeline,
            None,
            "cross-block concrete pipeline has an unreachable or uncovered lifecycle occurrence",
        ));
    }
    let mut incoming = vec![usize::MAX; cfg.reachable.len()];
    incoming[0] = 0;
    let mut normal_returns = 0;
    for block in cfg.order.iter().copied() {
        let mut cursor = incoming[block];
        if cursor == usize::MAX {
            return Err(invalid(
                pipeline,
                None,
                "cross-block concrete pipeline has no entry cursor for a reachable block",
            ));
        }
        for ordinal in &ordinals[cfg.offsets[block]..cfg.offsets[block + 1]] {
            if *ordinal != usize::MAX {
                if cursor != *ordinal {
                    return Err(invalid(
                        pipeline,
                        None,
                        "cross-block concrete pipeline paths do not share one exact lifecycle trace",
                    ));
                }
                cursor += 1;
            }
        }
        match cfg.exits[block] {
            ConcreteExitV1::Return => {
                if cursor != count {
                    return Err(invalid(
                        pipeline,
                        None,
                        "cross-block concrete pipeline normal return bypasses lifecycle occurrences",
                    ));
                }
                normal_returns += 1;
            }
            ConcreteExitV1::Trap => {
                if cursor > 1 {
                    return Err(invalid(
                        pipeline,
                        None,
                        "cross-block concrete pipeline trap follows an event or matching access",
                    ));
                }
            }
            ConcreteExitV1::Branch => {
                for target in control.discovery.cfg_successors[block].iter().copied() {
                    if incoming[target] == usize::MAX {
                        incoming[target] = cursor;
                    } else if incoming[target] != cursor {
                        return Err(invalid(
                            pipeline,
                            None,
                            "cross-block concrete pipeline incoming lifecycle cursors disagree",
                        ));
                    }
                }
            }
        }
    }
    if normal_returns == 0 {
        return Err(invalid(
            pipeline,
            None,
            "cross-block concrete pipeline has no complete normal lifecycle trace",
        ));
    }
    Ok(actions)
}
