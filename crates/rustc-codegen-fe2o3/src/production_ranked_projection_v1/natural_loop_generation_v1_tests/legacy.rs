// Frozen pre-conversion natural topology and trap checks; names changed only.
#[allow(clippy::too_many_arguments)]
fn legacy_project_natural_loop_topology_v1(
    callables: &[SemanticCallableDeclV1],
    function: &SemanticFunctionDeclV1,
    graph: &ProjectedLoopCfgV1,
    header: usize,
    body_entry: usize,
    exit: usize,
    work: &mut usize,
) -> Result<Option<ProjectedNaturalLoopTopologyV1>, ProductionRankedProjectionErrorV1> {
    if !graph.reachable.get(header).copied().unwrap_or(false) {
        return Err(ProductionRankedProjectionErrorV1::Incomplete(
            "a uniform induction header is unreachable",
        ));
    }
    let mut reachable_without_header = vec![false; graph.successors.len()];
    let mut pending = Vec::new();
    if graph.entry != header {
        pending.push(graph.entry);
    }
    while let Some(block) = pending.pop() {
        project_loop_graph_charge_v1(work, 1)?;
        if block == header || reachable_without_header[block] {
            continue;
        }
        reachable_without_header[block] = true;
        project_loop_graph_charge_v1(work, graph.successors[block].len())?;
        pending.extend(graph.successors[block].iter().copied());
    }
    let header_predecessors = graph.predecessors[header]
        .iter()
        .copied()
        .filter(|predecessor| graph.reachable[*predecessor])
        .collect::<Vec<_>>();
    let backedges = header_predecessors
        .iter()
        .copied()
        .filter(|predecessor| !reachable_without_header[*predecessor])
        .collect::<Vec<_>>();
    let preheaders = header_predecessors
        .iter()
        .copied()
        .filter(|predecessor| reachable_without_header[*predecessor])
        .collect::<Vec<_>>();
    if backedges.is_empty() {
        return Ok(None);
    }
    if backedges.len() != 1 {
        return Err(ProductionRankedProjectionErrorV1::Incomplete(
            "a uniform induction without one unique dominated backedge",
        ));
    }
    if preheaders.len() != 1 {
        return Err(ProductionRankedProjectionErrorV1::Incomplete(
            "a uniform induction without one unique preheader",
        ));
    }
    let latch = backedges[0];
    let preheader = preheaders[0];
    let latch_is_exact = matches!(
        function.blocks()[latch].terminator().kind(),
        SemanticTerminatorKindV1::Goto(edge)
            if edge.role() == SemanticEdgeRoleV1::Goto
                && edge.target().index() as usize == header
    );
    if !latch_is_exact || graph.successors[latch].as_slice() != [header] {
        return Err(ProductionRankedProjectionErrorV1::Incomplete(
            "a uniform induction preheader or latch has non-canonical control",
        ));
    }
    let preheader_control = match function.blocks()[preheader].terminator().kind() {
        SemanticTerminatorKindV1::Goto(edge)
            if edge.role() == SemanticEdgeRoleV1::Goto
                && edge.target().index() as usize == header
                && graph.successors[preheader].as_slice() == [header] =>
        {
            ProjectedInductionPreheaderControlV1::Direct
        }
        SemanticTerminatorKindV1::SwitchInt {
            discriminant,
            targets,
        } if targets.values().len() == 1 => {
            let explicit = targets.values()[0];
            let explicit_target = explicit.edge().target().index() as usize;
            let otherwise = targets.otherwise().target().index() as usize;
            if explicit.edge().role() != SemanticEdgeRoleV1::SwitchValue
                || targets.otherwise().role() != SemanticEdgeRoleV1::SwitchOtherwise
                || explicit_target == otherwise
                || !((explicit_target == header && otherwise == exit)
                    || (explicit_target == exit && otherwise == header))
                || graph.successors[preheader].as_slice() != [header.min(exit), header.max(exit)]
            {
                return Err(ProductionRankedProjectionErrorV1::Incomplete(
                    "an optional uniform induction preheader is not one exact header-or-exit switch",
                ));
            }
            ProjectedInductionPreheaderControlV1::Optional {
                discriminant: discriminant.clone(),
                explicit_value: explicit.value(),
                explicit_target,
                otherwise,
            }
        }
        _ => {
            return Err(ProductionRankedProjectionErrorV1::Incomplete(
                "an optional uniform induction preheader is not one exact header-or-exit switch",
            ));
        }
    };
    let mut in_loop = vec![false; graph.successors.len()];
    in_loop[header] = true;
    in_loop[latch] = true;
    let mut pending = vec![latch];
    while let Some(block) = pending.pop() {
        project_loop_graph_charge_v1(work, 1)?;
        if block == header {
            continue;
        }
        project_loop_graph_charge_v1(work, graph.predecessors[block].len())?;
        for &predecessor in &graph.predecessors[block] {
            if !graph.reachable[predecessor] || in_loop[predecessor] {
                continue;
            }
            in_loop[predecessor] = true;
            pending.push(predecessor);
        }
    }
    if !in_loop.get(body_entry).copied().unwrap_or(false)
        || in_loop.get(exit).copied().unwrap_or(false)
    {
        return Err(ProductionRankedProjectionErrorV1::Incomplete(
            "a uniform induction body and exit do not form a natural loop",
        ));
    }
    for (block, &inside) in in_loop.iter().enumerate() {
        if !inside {
            continue;
        }
        if reachable_without_header[block] {
            return Err(ProductionRankedProjectionErrorV1::Incomplete(
                "an irreducible entry enters a uniform induction body",
            ));
        }
        for &predecessor in &graph.predecessors[block] {
            if graph.reachable[predecessor]
                && !in_loop[predecessor]
                && !(block == header && predecessor == preheader)
            {
                return Err(ProductionRankedProjectionErrorV1::Incomplete(
                    "a uniform induction region has more than one entry",
                ));
            }
        }
    }
    let mut exits = Vec::new();
    for (source, &inside) in in_loop.iter().enumerate() {
        if !inside {
            continue;
        }
        project_loop_graph_charge_v1(work, graph.successors[source].len())?;
        for &target in &graph.successors[source] {
            if !in_loop[target] {
                exits.push((source, target));
            }
        }
    }
    let mut header_exit_count = 0_usize;
    for (source, target) in exits {
        if (source, target) == (header, exit) {
            header_exit_count += 1;
            continue;
        }
        if !legacy_transparent_path_terminates_in_reviewed_trap_v1(
            callables, function, &in_loop, target, work,
        )? {
            return Err(ProductionRankedProjectionErrorV1::Incomplete(
                "a uniform induction region does not have one unique header exit",
            ));
        }
    }
    if header_exit_count != 1 {
        return Err(ProductionRankedProjectionErrorV1::Incomplete(
            "a uniform induction region does not have one unique header exit",
        ));
    }
    let loop_blocks = in_loop
        .iter()
        .enumerate()
        .filter_map(|(block, inside)| inside.then_some(block))
        .collect();
    Ok(Some(ProjectedNaturalLoopTopologyV1 {
        preheader,
        preheader_control,
        latch,
        loop_blocks,
    }))
}

fn legacy_transparent_path_terminates_in_reviewed_trap_v1(
    callables: &[SemanticCallableDeclV1],
    function: &SemanticFunctionDeclV1,
    in_loop: &[bool],
    start: usize,
    work: &mut usize,
) -> Result<bool, ProductionRankedProjectionErrorV1> {
    let mut visited = vec![false; function.blocks().len()];
    let mut current = start;
    loop {
        project_loop_graph_charge_v1(work, 1)?;
        let Some(block) = function.blocks().get(current) else {
            return Err(ProductionRankedProjectionErrorV1::Unsupported(
                "a uniform induction terminal side exit is outside the semantic CFG",
            ));
        };
        if in_loop.get(current).copied().unwrap_or(false) || visited[current] {
            return Ok(false);
        }
        visited[current] = true;
        project_loop_graph_charge_v1(work, block.statements().len())?;
        if block.statements().iter().any(|statement| {
            !matches!(
                statement.kind(),
                SemanticStatementKindV1::StorageLive(_)
                    | SemanticStatementKindV1::StorageDead(_)
                    | SemanticStatementKindV1::Nop
            )
        }) {
            return Ok(false);
        }
        match block.terminator().kind() {
            SemanticTerminatorKindV1::Goto(edge) => {
                current = edge.target().index() as usize;
            }
            SemanticTerminatorKindV1::Call(call)
                if call.destination().is_none()
                    && call.arguments().is_empty()
                    && !matches!(call.unwind(), SemanticUnwindActionV1::Cleanup(_))
                    && matches!(
                        callables.get(call.callee().index() as usize),
                        Some(SemanticCallableDeclV1::CompilerIntrinsic {
                            operation: SemanticCompilerIntrinsicOperationV1::Trap,
                            ..
                        })
                    ) =>
            {
                return Ok(true);
            }
            _ => return Ok(false),
        }
    }
}
