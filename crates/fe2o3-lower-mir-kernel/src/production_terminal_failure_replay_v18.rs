#[derive(Clone, Copy)]
struct CheckedTerminalAssertionV18 {
    site: SemanticKirAssertSiteV1,
    original_failure: BlockId,
    actual_failure: BlockId,
    diagnostic: u32,
}

fn terminal_failure_block_v18<'a>(
    body: &'a fe2o3_kernel_ir::FunctionBody,
    index: &[(BlockId, usize)],
    block: BlockId,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<&'a BasicBlock, ProductionSemanticKirErrorV1> {
    body.blocks
        .get(terminal_failure_find_v18(index, &block, budget)?)
        .ok_or_else(terminal_failure_error_v18)
}

fn check_terminal_failures_source_v18(
    subject: InstanceAssertReplaySubjectV1<'_>,
    instances: &ExecutionInstancesV29<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let Some(relation) = subject.terminal_failures else {
        return Ok(());
    };
    budget.charge_work(3)?;
    if relation.origins.source != ExecutionCallSourceV29::from_instances(instances, budget)?
        || relation.origins.ledger != budget.work_ledger_identity_v1()
        || relation.origins.rows.len() != relation.closures.len()
    {
        return Err(terminal_failure_error_v18());
    }
    let body = subject
        .functions
        .get(subject.function_ordinal)
        .and_then(|f| f.body.as_ref())
        .ok_or_else(terminal_failure_error_v18)?;
    let floor = budget.storage();
    budget.reserve_storage(argument_sum_v1(&[
        2 * size_of::<Vec<(BlockId, usize)>>(),
        size_of::<Vec<((usize, u32), usize)>>(),
        size_of::<Vec<BlockId>>(),
        size_of::<Vec<usize>>(),
    ])?)?;
    let mut blocks = emission_vec_v1(body.blocks.len(), budget)?;
    let mut sources = emission_vec_v1(relation.origins.rows.len(), budget)?;
    let mut targets = emission_vec_v1(relation.origins.rows.len(), budget)?;
    let mut original_targets = emission_vec_v1(relation.origins.rows.len(), budget)?;
    for (index, block) in body.blocks.iter().enumerate() {
        budget.charge_work(1)?;
        blocks.push((block.id, index));
    }
    for (index, origin) in relation.origins.rows.iter().enumerate() {
        budget.charge_work(1)?;
        if index != 0
            && (
                relation.origins.rows[index - 1].instance.index(),
                relation.origins.rows[index - 1].block.index(),
            ) >= (origin.instance.index(), origin.block.index())
        {
            return Err(terminal_failure_error_v18());
        }
        sources.push(((origin.instance.index(), origin.block.index()), index));
        targets.push((relation.closures[index].block, index));
        if let TerminalFailureSiteV18::Edge { target, .. } = origin.site {
            original_targets.push(target);
        }
    }
    terminal_failure_sort_v18(&mut blocks, budget)?;
    terminal_failure_sort_v18(&mut sources, budget)?;
    terminal_failure_sort_v18(&mut targets, budget)?;
    call_splice_sort_work_v1(original_targets.len(), budget)
        .map_err(|error| pending_scope_correspondence_error_v29(error.into()))?;
    original_targets.sort_unstable();
    budget.charge_work(original_targets.len())?;
    original_targets.dedup();
    check_terminal_failure_roster_v18(
        subject, instances, relation, body, &blocks, &sources, budget,
    )?;
    for (index, (origin, closure)) in relation
        .origins
        .rows
        .iter()
        .zip(&relation.closures)
        .enumerate()
    {
        budget.charge_work(12)?;
        let instance = instances
            .instance(origin.instance)
            .ok_or_else(terminal_failure_error_v18)?;
        if instances.instance_reachable(origin.instance) != Some(true)
            || instances.block_reachable(origin.instance, origin.block) != Some(true)
            || instance.function() != origin.function
            || closure.origin != index
            || closure.first != closure.original_gap
            || closure.diagnostic
                != closure
                    .first
                    .checked_add(closure.scope_ends)
                    .ok_or(ArgumentResourceV1::Arithmetic)?
        {
            return Err(terminal_failure_error_v18());
        }
        let declaration = instance
            .declaration()
            .blocks()
            .get(origin.block.index() as usize)
            .ok_or_else(terminal_failure_error_v18)?;
        let original = declaration.terminator().kind();
        let sidecar = &subject.sidecars.rows[subject
            .active_instances
            .sidecar_ordinal(
                origin.instance.index(),
                instances.instances().len(),
                &subject.sidecars.rows,
                budget,
            )?
            .ok_or_else(terminal_failure_error_v18)?];
        let control = subject
            .coordinates
            .controls
            .rows
            .get(origin.control)
            .ok_or_else(terminal_failure_error_v18)?;
        let span = subject
            .coordinates
            .spans
            .rows
            .get(origin.source_span)
            .ok_or_else(terminal_failure_error_v18)?;
        if control.instance != origin.instance
            || control.semantic_block != Some(origin.block)
            || control.origin != InstanceControlOriginV1::Retained
            || span.instance != origin.instance
            || span.removed_call.is_some()
            || !matches!(span.source, InstanceSpanSourceV1::Terminator(row)
                if row.semantic_function == origin.function && row.semantic_block == origin.block
                    && row.correspondence_owner == subject.coordinates.root)
        {
            return Err(terminal_failure_error_v18());
        }
        match (origin.kind, original) {
            (
                TerminalFailureKindV18::Assert { assertion },
                SemanticTerminatorKindV1::Assert {
                    expected, unwind, ..
                },
            ) => {
                let capture = sidecar
                    .instance_assert_origins
                    .as_ref()
                    .ok_or_else(terminal_failure_error_v18)?;
                capture.check_identity(instances, origin.instance, budget)?;
                let record = capture
                    .records
                    .get(assertion)
                    .ok_or_else(terminal_failure_error_v18)?;
                let TerminalFailureSiteV18::Edge {
                    block,
                    successor,
                    target,
                } = origin.site
                else {
                    return Err(terminal_failure_error_v18());
                };
                if record.site
                    != SemanticKirAssertSiteV1::new(
                        subject.coordinates.root,
                        origin.function,
                        origin.block,
                    )
                    || record.expected != *expected
                    || record.block != block
                    || successor != u32::from(*expected)
                    || !matches!(record.outcome, PendingAssertOutcomeV1::Emitted { failure, .. } if failure == target)
                    || matches!(unwind, SemanticUnwindActionV1::Cleanup(_))
                {
                    return Err(terminal_failure_error_v18());
                }
            }
            (TerminalFailureKindV18::Abort, SemanticTerminatorKindV1::Abort)
            | (
                TerminalFailureKindV18::UnwindTerminate,
                SemanticTerminatorKindV1::UnwindTerminate,
            ) => {}
            (
                TerminalFailureKindV18::Trap | TerminalFailureKindV18::BoundsGuard,
                SemanticTerminatorKindV1::Call(call),
            ) => {
                let callable = instances
                    .owner()
                    .source_semantic()
                    .callables()
                    .get(call.callee().index() as usize)
                    .ok_or_else(terminal_failure_error_v18)?;
                let valid = matches!(
                    (origin.kind, callable),
                    (
                        TerminalFailureKindV18::Trap,
                        SemanticCallableDeclV1::CompilerIntrinsic {
                            operation: SemanticCompilerIntrinsicOperationV1::Trap,
                            ..
                        }
                    ) | (
                        TerminalFailureKindV18::BoundsGuard,
                        SemanticCallableDeclV1::CompilerIntrinsic {
                            operation: SemanticCompilerIntrinsicOperationV1::MemoryVolatileLoad { .. },
                            ..
                        }
                    )
                );
                if !valid
                    || matches!(call.unwind(), SemanticUnwindActionV1::Cleanup(_))
                    || (origin.kind == TerminalFailureKindV18::Trap
                        && (call.destination().is_some()
                            || !call.arguments().is_empty()
                            || instances.call_control(ProductionCallOccurrenceV1 {
                                caller: origin.instance,
                                block: origin.block,
                            }) != Some(ProductionCallControlV1::NoNormalReturn)))
                {
                    return Err(terminal_failure_error_v18());
                }
            }
            _ => return Err(terminal_failure_error_v18()),
        }
        let terminal = terminal_failure_block_v18(body, &blocks, closure.block, budget)?;
        let source_block = match origin.site {
            TerminalFailureSiteV18::Edge {
                block,
                successor,
                target,
            } => {
                if !closure.generated
                    || closure.original_gap != 0
                    || closure.block == target
                    || !terminal.parameters.is_empty()
                {
                    return Err(terminal_failure_error_v18());
                }
                let actual_source = terminal_failure_block_v18(body, &blocks, block, budget)?;
                let edge = actual_source
                    .terminator
                    .as_ref()
                    .and_then(|t| terminal_failure_edge_v18(t, successor))
                    .ok_or_else(terminal_failure_error_v18)?;
                if edge.0 != closure.block || !edge.1.is_empty() {
                    return Err(terminal_failure_error_v18());
                }
                match (origin.normal, actual_source.terminator.as_ref()) {
                    (
                        Some((expected_condition, expected_success)),
                        Some(Terminator::ConditionalBranch {
                            condition,
                            then_target,
                            else_target,
                            ..
                        }),
                    ) => {
                        let actual_success = if successor == 0 {
                            *else_target
                        } else {
                            *then_target
                        };
                        if *condition != expected_condition || actual_success != expected_success {
                            return Err(terminal_failure_error_v18());
                        }
                    }
                    (None, Some(Terminator::Branch { .. })) => {}
                    _ => return Err(terminal_failure_error_v18()),
                }
                let retained_trap = terminal_failure_block_v18(body, &blocks, target, budget)?;
                if !retained_trap.parameters.is_empty()
                    || retained_trap.operations.len() != 1
                    || !matches!(retained_trap.terminator, Some(Terminator::Unreachable))
                    || !terminal_failure_is_trap_v18(&retained_trap.operations[0], budget)?
                {
                    return Err(terminal_failure_error_v18());
                }
                block
            }
            TerminalFailureSiteV18::Operation { block, operation } => {
                if closure.generated
                    || closure.block != block
                    || closure.original_gap != operation
                    || origin.normal.is_some()
                {
                    return Err(terminal_failure_error_v18());
                }
                block
            }
        };
        if control.physical_block != source_block
            || terminal.operations.len() != argument_sum_v1(&[closure.diagnostic as usize, 1])?
            || !matches!(terminal.terminator, Some(Terminator::Unreachable))
        {
            return Err(terminal_failure_error_v18());
        }
        let cleanup = terminal
            .operations
            .get(closure.first as usize..closure.diagnostic as usize)
            .ok_or_else(terminal_failure_error_v18)?;
        budget.charge_work(cleanup.len())?;
        let mut previous = None;
        for operation in cleanup {
            let OperationKind::Execution(fe2o3_kernel_ir::ExecutionOperationV15::ScopeEnd {
                workgroup,
                discarded,
            }) = &operation.kind
            else {
                return Err(terminal_failure_error_v18());
            };
            budget.charge_work(argument_sum_v1(&[
                operation.results.len(),
                discarded.len(),
                1,
            ])?)?;
            if !operation.results.is_empty()
                || previous.is_some_and(|value| value >= *workgroup)
                || discarded.windows(2).any(|pair| pair[0] >= pair[1])
            {
                return Err(terminal_failure_error_v18());
            }
            previous = Some(*workgroup);
        }
        if !terminal_failure_is_trap_v18(&terminal.operations[closure.diagnostic as usize], budget)?
        {
            return Err(terminal_failure_error_v18());
        }
    }
    // Every added block has exactly its original failure edge, never a success,
    // unwind, arbitrary predecessor or a second source occurrence.
    let mut arrivals = emission_vec_v1(relation.closures.len(), budget)?;
    budget.charge_work(relation.closures.len())?;
    arrivals.resize(relation.closures.len(), 0usize);
    for block in &body.blocks {
        budget.charge_work(1)?;
        let mut successor = 0u32;
        block
            .terminator
            .as_ref()
            .ok_or_else(terminal_failure_error_v18)?
            .try_visit_edges_v1(|target, _| {
                budget.charge_work(call_splice_search_work_v1(original_targets.len()))?;
                if original_targets.binary_search(&target).is_ok() {
                    return Err(terminal_failure_error_v18());
                }
                budget.charge_work(call_splice_search_work_v1(targets.len()))?;
                if let Ok(index) = targets.binary_search_by_key(&target, |row| row.0) {
                    let index = targets[index].1;
                    if relation.closures[index].generated {
                        if !matches!(relation.origins.rows[index].site,
                            TerminalFailureSiteV18::Edge { block: source, successor: edge, .. }
                                if source == block.id && edge == successor)
                        {
                            return Err(terminal_failure_error_v18());
                        }
                        arrivals[index] = argument_sum_v1(&[arrivals[index], 1])?;
                    }
                }
                successor = successor
                    .checked_add(1)
                    .ok_or(ArgumentResourceV1::Arithmetic)?;
                Ok::<_, ProductionSemanticKirErrorV1>(())
            })?;
    }
    for (closure, arrivals) in relation.closures.iter().zip(&arrivals) {
        budget.charge_work(1)?;
        if closure.generated && *arrivals != 1 {
            return Err(terminal_failure_error_v18());
        }
    }
    drop((blocks, sources, targets, original_targets, arrivals));
    budget.release_storage(
        budget
            .storage()
            .checked_sub(floor)
            .ok_or(ArgumentResourceV1::Accounting)?,
    )?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn check_terminal_failure_roster_v18(
    subject: InstanceAssertReplaySubjectV1<'_>,
    instances: &ExecutionInstancesV29<'_>,
    relation: &TerminalFailureRelationV18,
    body: &fe2o3_kernel_ir::FunctionBody,
    blocks: &[(BlockId, usize)],
    sources: &[((usize, u32), usize)],
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let floor = budget.storage();
    budget.reserve_storage(2 * size_of::<Vec<((usize, u32), usize)>>())?;
    let mut controls = emission_vec_v1(subject.coordinates.controls.rows.len(), budget)?;
    for (index, row) in subject.coordinates.controls.rows.iter().enumerate() {
        budget.charge_work(1)?;
        if row.physical_block == row.original_block {
            if let Some(block) = row.semantic_block {
                controls.push(((row.instance.index(), block.index()), index));
            }
        }
    }
    terminal_failure_sort_v18(&mut controls, budget)?;
    let mut count = 0usize;
    for sidecar in &subject.sidecars.rows {
        budget.charge_work(1)?;
        count = argument_sum_v1(&[
            count,
            sidecar
                .instance_assert_origins
                .as_ref()
                .ok_or_else(terminal_failure_error_v18)?
                .records
                .len(),
        ])?;
    }
    let mut assertions = emission_vec_v1(count, budget)?;
    for sidecar in &subject.sidecars.rows {
        budget.charge_work(1)?;
        let instance = sidecar
            .source_call_instance
            .ok_or_else(terminal_failure_error_v18)?;
        let capture = sidecar
            .instance_assert_origins
            .as_ref()
            .ok_or_else(terminal_failure_error_v18)?;
        capture.check_identity(instances, instance, budget)?;
        for (index, record) in capture.records.iter().enumerate() {
            budget.charge_work(1)?;
            assertions.push((
                (instance.index(), record.site.semantic_block.index()),
                index,
            ));
        }
    }
    terminal_failure_sort_v18(&mut assertions, budget)?;
    let mut required = 0usize;
    for index in 0..instances.instances().len() {
        budget.charge_work(1)?;
        let instance = instances
            .id_at(index)
            .ok_or_else(terminal_failure_error_v18)?;
        if instances.instance_reachable(instance) != Some(true) {
            continue;
        }
        let declaration = instances
            .instance(instance)
            .ok_or_else(terminal_failure_error_v18)?
            .declaration();
        let sidecar = &subject.sidecars.rows[subject
            .active_instances
            .sidecar_ordinal(
                index,
                instances.instances().len(),
                &subject.sidecars.rows,
                budget,
            )?
            .ok_or_else(terminal_failure_error_v18)?];
        for (ordinal, row) in declaration.blocks().iter().enumerate() {
            budget.charge_work(2)?;
            let block = SemanticBlockIdV1::from_index(
                u32::try_from(ordinal).map_err(|_| ArgumentResourceV1::Arithmetic)?,
            );
            if instances.block_reachable(instance, block) != Some(true)
                || !terminal_failure_candidate_v18(
                    instances.owner().source_semantic(),
                    row.terminator().kind(),
                )
            {
                continue;
            }
            let key = (index, block.index());
            let control = &subject.coordinates.controls.rows
                [terminal_failure_find_v18(&controls, &key, budget)?];
            let physical =
                terminal_failure_block_v18(body, blocks, control.physical_block, budget)?;
            let expected = match row.terminator().kind() {
                SemanticTerminatorKindV1::Assert { .. } => {
                    let capture = sidecar
                        .instance_assert_origins
                        .as_ref()
                        .ok_or_else(terminal_failure_error_v18)?;
                    let record =
                        &capture.records[terminal_failure_find_v18(&assertions, &key, budget)?];
                    matches!(record.outcome, PendingAssertOutcomeV1::Emitted { .. })
                }
                SemanticTerminatorKindV1::Call(call) => {
                    let Some(SemanticCallableDeclV1::CompilerIntrinsic { operation, .. }) =
                        instances
                            .owner()
                            .source_semantic()
                            .callables()
                            .get(call.callee().index() as usize)
                    else {
                        return Err(terminal_failure_error_v18());
                    };
                    if matches!(operation, SemanticCompilerIntrinsicOperationV1::Trap) {
                        true
                    } else {
                        match physical.terminator.as_ref() {
                            Some(Terminator::ConditionalBranch { then_target, .. }) => {
                                let destination =
                                    call.destination().ok_or_else(terminal_failure_error_v18)?;
                                let success = terminal_failure_find_v18(
                                    &controls,
                                    &(index, destination.edge().target().index()),
                                    budget,
                                )?;
                                if subject.coordinates.controls.rows[success].physical_block
                                    != *then_target
                                {
                                    return Err(terminal_failure_error_v18());
                                }
                                true
                            }
                            Some(Terminator::Branch { .. }) => false,
                            _ => return Err(terminal_failure_error_v18()),
                        }
                    }
                }
                SemanticTerminatorKindV1::Abort | SemanticTerminatorKindV1::UnwindTerminate => true,
                _ => return Err(terminal_failure_error_v18()),
            };
            budget.charge_work(call_splice_search_work_v1(sources.len()))?;
            let actual = sources.binary_search_by_key(&key, |row| row.0).is_ok();
            if actual != expected {
                return Err(terminal_failure_error_v18());
            }
            required = argument_sum_v1(&[required, usize::from(expected)])?;
        }
    }
    if required != relation.origins.rows.len() {
        return Err(terminal_failure_error_v18());
    }
    drop((controls, assertions));
    budget.release_storage(
        budget
            .storage()
            .checked_sub(floor)
            .ok_or(ArgumentResourceV1::Accounting)?,
    )?;
    Ok(())
}

fn checked_terminal_assertion_v18(
    relation: Option<&TerminalFailureRelationV18>,
    instance: ProductionCallInstanceIdV1,
    pending: &PendingAssertOriginV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<Option<CheckedTerminalAssertionV18>, ProductionSemanticKirErrorV1> {
    let Some(relation) = relation else {
        return Ok(None);
    };
    budget.charge_work(call_splice_search_work_v1(relation.origins.rows.len()))?;
    let key = (instance.index(), pending.site.semantic_block.index());
    let index = relation
        .origins
        .rows
        .binary_search_by_key(&key, |row| (row.instance.index(), row.block.index()))
        .map_err(|_| terminal_failure_error_v18())?;
    let origin = &relation.origins.rows[index];
    let closure = relation
        .closures
        .get(index)
        .ok_or_else(terminal_failure_error_v18)?;
    let TerminalFailureSiteV18::Edge { target, .. } = origin.site else {
        return Err(terminal_failure_error_v18());
    };
    if !matches!(origin.kind, TerminalFailureKindV18::Assert { .. })
        || !closure.generated
        || origin.function != pending.site.semantic_function
        || closure.origin != index
    {
        return Err(terminal_failure_error_v18());
    }
    Ok(Some(CheckedTerminalAssertionV18 {
        site: pending.site,
        original_failure: target,
        actual_failure: closure.block,
        diagnostic: closure.diagnostic,
    }))
}

fn terminal_failure_ordinal_v18(
    relation: Option<&TerminalFailureRelationV18>,
    block: BlockId,
    original: u32,
    gap: bool,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<u32, ProductionSemanticKirErrorV1> {
    let mut ordinal = original;
    if let Some(relation) = relation {
        budget.charge_work(relation.closures.len())?;
        for row in &relation.closures {
            if !row.generated
                && row.block == block
                && (row.original_gap < original || (!gap && row.original_gap == original))
            {
                ordinal = ordinal
                    .checked_add(row.scope_ends)
                    .ok_or(ArgumentResourceV1::Arithmetic)?;
            }
        }
    }
    Ok(ordinal)
}
