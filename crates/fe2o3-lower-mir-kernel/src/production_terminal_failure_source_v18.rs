// Source-derived requests only. Ownership closure and physical replay are separate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TerminalFailureKindV18 {
    Assert { assertion: usize },
    Abort,
    UnwindTerminate,
    Trap,
    BoundsGuard,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TerminalFailureSiteV18 {
    Edge {
        block: BlockId,
        successor: u32,
        target: BlockId,
    },
    Operation {
        block: BlockId,
        operation: u32,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct TerminalFailureOriginV18 {
    instance: ProductionCallInstanceIdV1,
    function: SemanticFunctionIdV1,
    block: SemanticBlockIdV1,
    kind: TerminalFailureKindV18,
    control: usize,
    source_span: usize,
    site: TerminalFailureSiteV18,
    normal: Option<(ValueId, BlockId)>,
}

struct TerminalFailureOriginsV18 {
    source: ExecutionCallSourceV29,
    ledger: fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
    rows: Vec<TerminalFailureOriginV18>,
}

fn terminal_failure_error_v18() -> ProductionSemanticKirErrorV1 {
    unsupported(
        0,
        None,
        None,
        "terminal failure differs from its original source occurrence",
    )
}

fn terminal_failure_candidate_v18(
    source: &AdmittedInertSemanticMirV1,
    terminator: &SemanticTerminatorKindV1,
) -> bool {
    match terminator {
        SemanticTerminatorKindV1::Assert { .. }
        | SemanticTerminatorKindV1::Abort
        | SemanticTerminatorKindV1::UnwindTerminate => true,
        SemanticTerminatorKindV1::Call(call) => matches!(
            source.callables().get(call.callee().index() as usize),
            Some(SemanticCallableDeclV1::CompilerIntrinsic {
                operation: SemanticCompilerIntrinsicOperationV1::Trap
                    | SemanticCompilerIntrinsicOperationV1::MemoryVolatileLoad { .. },
                ..
            })
        ),
        _ => false,
    }
}

fn terminal_failure_is_trap_v18(
    operation: &Operation,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<bool, ProductionSemanticKirErrorV1> {
    budget.charge_work(argument_sum_v1(&[operation.results.len(), 1])?)?;
    let OperationKind::Call { callee, arguments } = &operation.kind else {
        return Ok(false);
    };
    budget.charge_work(argument_sum_v1(&[callee.as_str().len(), arguments.len()])?)?;
    Ok(operation.results.is_empty()
        && AmdGpuDiagnosticOperation::from_intrinsic_call(callee, arguments)
            == Some(AmdGpuDiagnosticOperation::Trap))
}

fn terminal_failure_find_v18<K: Ord>(
    rows: &[(K, usize)],
    key: &K,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<usize, ProductionSemanticKirErrorV1> {
    budget.charge_work(call_splice_search_work_v1(rows.len()))?;
    rows.binary_search_by(|row| row.0.cmp(key))
        .map(|i| rows[i].1)
        .map_err(|_| terminal_failure_error_v18())
}

fn terminal_failure_sort_v18<K: Ord>(
    rows: &mut [(K, usize)],
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    call_splice_sort_work_v1(rows.len(), budget)
        .map_err(|e| pending_scope_correspondence_error_v29(e.into()))?;
    rows.sort_unstable_by(|a, b| a.0.cmp(&b.0));
    budget.charge_work(rows.len())?;
    if rows.windows(2).any(|p| p[0].0 == p[1].0) {
        return Err(terminal_failure_error_v18());
    }
    Ok(())
}

fn derive_terminal_failure_origins_v18(
    instances: &ExecutionInstancesV29<'_>,
    pending: &PendingScopedRootEmissionV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<TerminalFailureOriginsV18, ProductionSemanticKirErrorV1> {
    pending
        .coordinates
        .check_source_plan(instances, budget)
        .map_err(pending_scope_correspondence_error_v29)?;
    let source = ExecutionCallSourceV29::from_instances(instances, budget)?;
    let semantic = instances.owner().source_semantic();
    let body = pending
        .function
        .body
        .as_ref()
        .ok_or_else(terminal_failure_error_v18)?;
    let mut candidates = 0usize;
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
        for (block, row) in declaration.blocks().iter().enumerate() {
            budget.charge_work(2)?;
            if instances.block_reachable(
                instance,
                SemanticBlockIdV1::from_index(
                    u32::try_from(block).map_err(|_| ArgumentResourceV1::Arithmetic)?,
                ),
            ) == Some(true)
                && terminal_failure_candidate_v18(semantic, row.terminator().kind())
            {
                candidates = argument_sum_v1(&[candidates, 1])?;
            }
        }
    }
    let mut rows = emission_vec_v1(candidates, budget)?;
    if candidates == 0 {
        return Ok(TerminalFailureOriginsV18 {
            source,
            ledger: budget.work_ledger_identity_v1(),
            rows,
        });
    }
    let scratch_floor = budget.storage();
    budget.reserve_storage(argument_sum_v1(&[
        size_of::<Vec<(BlockId, usize)>>(),
        3 * size_of::<Vec<((usize, u32), usize)>>(),
        size_of::<Vec<(usize, usize)>>(),
    ])?)?;
    let mut blocks = emission_vec_v1(body.blocks.len(), budget)?;
    let mut controls = emission_vec_v1(pending.coordinates.controls.rows.len(), budget)?;
    let mut spans = emission_vec_v1(pending.coordinates.spans.rows.len(), budget)?;
    let mut assertion_count = 0usize;
    for sidecar in &pending.sidecars.rows {
        budget.charge_work(1)?;
        assertion_count = argument_sum_v1(&[
            assertion_count,
            sidecar
                .instance_assert_origins
                .as_ref()
                .ok_or_else(terminal_failure_error_v18)?
                .records
                .len(),
        ])?;
    }
    let mut assertions = emission_vec_v1(assertion_count, budget)?;
    let mut traps = emission_vec_v1(pending.sidecars.rows.len(), budget)?;
    for sidecar in &pending.sidecars.rows {
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
        let mut trap = None;
        for span in &sidecar.synthetic_operation_spans {
            budget.charge_work(1)?;
            if span.rule != SemanticKirSyntheticOperationRuleV1::RuntimeAssertFailureTrap {
                continue;
            }
            if span.semantic_function != capture.function
                || span.correspondence_owner != pending.coordinates.root
                || span.first_operation_ordinal != 0
                || span.operation_count != 1
                || trap.replace(span.kernel_ir_block).is_some()
            {
                return Err(terminal_failure_error_v18());
            }
        }
        if let Some(trap) = trap {
            traps.push((instance.index(), trap.0 as usize));
        }
    }
    for (index, block) in body.blocks.iter().enumerate() {
        budget.charge_work(1)?;
        blocks.push((block.id, index));
    }
    for (index, row) in pending.coordinates.controls.rows.iter().enumerate() {
        budget.charge_work(1)?;
        // The original block remains the entry to a split call. Its retained
        // continuation has the same semantic block but is not an edge target.
        if row.physical_block == row.original_block {
            if let Some(block) = row.semantic_block {
                controls.push(((row.instance.index(), block.index()), index));
            }
        }
    }
    for (index, row) in pending.coordinates.spans.rows.iter().enumerate() {
        budget.charge_work(1)?;
        if let InstanceSpanSourceV1::Terminator(span) = row.source {
            if row.removed_call.is_none() {
                spans.push(((row.instance.index(), span.semantic_block.index()), index));
            }
        }
    }
    terminal_failure_sort_v18(&mut blocks, budget)?;
    terminal_failure_sort_v18(&mut controls, budget)?;
    terminal_failure_sort_v18(&mut spans, budget)?;
    terminal_failure_sort_v18(&mut assertions, budget)?;
    terminal_failure_sort_v18(&mut traps, budget)?;
    for index in 0..instances.instances().len() {
        budget.charge_work(1)?;
        let instance = instances
            .id_at(index)
            .ok_or_else(terminal_failure_error_v18)?;
        if instances.instance_reachable(instance) != Some(true) {
            continue;
        }
        let original = instances
            .instance(instance)
            .ok_or_else(terminal_failure_error_v18)?;
        let sidecar = &pending.sidecars.rows[pending
            .active_instances
            .sidecar_ordinal(
                instance.index(),
                instances.instances().len(),
                &pending.sidecars.rows,
                budget,
            )?
            .ok_or_else(terminal_failure_error_v18)?];
        for (block_index, row) in original.declaration().blocks().iter().enumerate() {
            budget.charge_work(2)?;
            let block = SemanticBlockIdV1::from_index(
                u32::try_from(block_index).map_err(|_| ArgumentResourceV1::Arithmetic)?,
            );
            let terminator = row.terminator().kind();
            if instances.block_reachable(instance, block) != Some(true)
                || !terminal_failure_candidate_v18(semantic, terminator)
            {
                continue;
            }
            let key = (instance.index(), block.index());
            let control = terminal_failure_find_v18(&controls, &key, budget)?;
            if pending.coordinates.controls.rows[control].origin
                != InstanceControlOriginV1::Retained
            {
                return Err(terminal_failure_error_v18());
            }
            let source_span = terminal_failure_find_v18(&spans, &key, budget)?;
            let physical_id = pending.coordinates.controls.rows[control].physical_block;
            let physical = &body.blocks[terminal_failure_find_v18(&blocks, &physical_id, budget)?];
            let actual = physical
                .terminator
                .as_ref()
                .ok_or_else(terminal_failure_error_v18)?;
            let mut normal = None;
            let (kind, site) = match terminator {
                SemanticTerminatorKindV1::Assert {
                    expected, unwind, ..
                } => {
                    if matches!(unwind, SemanticUnwindActionV1::Cleanup(_)) {
                        return Err(terminal_failure_error_v18());
                    }
                    let capture = sidecar
                        .instance_assert_origins
                        .as_ref()
                        .ok_or_else(terminal_failure_error_v18)?;
                    let assertion = terminal_failure_find_v18(&assertions, &key, budget)?;
                    let record = capture
                        .records
                        .get(assertion)
                        .ok_or_else(terminal_failure_error_v18)?;
                    if record.site.semantic_function != original.function()
                        || record.expected != *expected
                        || record.block != physical_id
                    {
                        return Err(terminal_failure_error_v18());
                    }
                    let PendingAssertOutcomeV1::Emitted { condition, failure } = record.outcome
                    else {
                        continue;
                    };
                    let Terminator::ConditionalBranch {
                        condition: actual_condition,
                        then_target,
                        then_arguments,
                        else_target,
                        else_arguments,
                    } = actual
                    else {
                        return Err(terminal_failure_error_v18());
                    };
                    let (target, arguments) = if *expected {
                        (else_target, else_arguments)
                    } else {
                        (then_target, then_arguments)
                    };
                    if condition != *actual_condition || failure != *target || !arguments.is_empty()
                    {
                        return Err(terminal_failure_error_v18());
                    }
                    let success = if *expected {
                        *then_target
                    } else {
                        *else_target
                    };
                    if success != record.physical_success {
                        return Err(terminal_failure_error_v18());
                    }
                    normal = Some((condition, success));
                    (
                        TerminalFailureKindV18::Assert { assertion },
                        TerminalFailureSiteV18::Edge {
                            block: physical_id,
                            successor: u32::from(*expected),
                            target: failure,
                        },
                    )
                }
                SemanticTerminatorKindV1::Abort | SemanticTerminatorKindV1::UnwindTerminate => {
                    let Terminator::Branch { target, arguments } = actual else {
                        return Err(terminal_failure_error_v18());
                    };
                    if !arguments.is_empty() {
                        return Err(terminal_failure_error_v18());
                    }
                    (
                        if matches!(terminator, SemanticTerminatorKindV1::Abort) {
                            TerminalFailureKindV18::Abort
                        } else {
                            TerminalFailureKindV18::UnwindTerminate
                        },
                        TerminalFailureSiteV18::Edge {
                            block: physical_id,
                            successor: 0,
                            target: *target,
                        },
                    )
                }
                SemanticTerminatorKindV1::Call(call) => {
                    if matches!(call.unwind(), SemanticUnwindActionV1::Cleanup(_)) {
                        return Err(terminal_failure_error_v18());
                    }
                    let Some(SemanticCallableDeclV1::CompilerIntrinsic { operation, .. }) =
                        semantic.callables().get(call.callee().index() as usize)
                    else {
                        return Err(terminal_failure_error_v18());
                    };
                    if matches!(operation, SemanticCompilerIntrinsicOperationV1::Trap) {
                        if call.destination().is_some()
                            || !call.arguments().is_empty()
                            || !matches!(actual, Terminator::Unreachable)
                            || instances.call_control(ProductionCallOccurrenceV1 {
                                caller: instance,
                                block,
                            }) != Some(ProductionCallControlV1::NoNormalReturn)
                        {
                            return Err(terminal_failure_error_v18());
                        }
                        let ordinal = physical
                            .operations
                            .len()
                            .checked_sub(1)
                            .ok_or_else(terminal_failure_error_v18)?;
                        if !terminal_failure_is_trap_v18(&physical.operations[ordinal], budget)? {
                            return Err(terminal_failure_error_v18());
                        }
                        (
                            TerminalFailureKindV18::Trap,
                            TerminalFailureSiteV18::Operation {
                                block: physical_id,
                                operation: u32::try_from(ordinal)
                                    .map_err(|_| ArgumentResourceV1::Arithmetic)?,
                            },
                        )
                    } else {
                        let Terminator::ConditionalBranch {
                            condition,
                            then_target,
                            else_target,
                            else_arguments,
                            ..
                        } = actual
                        else {
                            continue;
                        };
                        if !else_arguments.is_empty() {
                            return Err(terminal_failure_error_v18());
                        }
                        let destination =
                            call.destination().ok_or_else(terminal_failure_error_v18)?;
                        let successor = terminal_failure_find_v18(
                            &controls,
                            &(instance.index(), destination.edge().target().index()),
                            budget,
                        )?;
                        if pending.coordinates.controls.rows[successor].physical_block
                            != *then_target
                        {
                            return Err(terminal_failure_error_v18());
                        }
                        normal = Some((*condition, *then_target));
                        (
                            TerminalFailureKindV18::BoundsGuard,
                            TerminalFailureSiteV18::Edge {
                                block: physical_id,
                                successor: 1,
                                target: *else_target,
                            },
                        )
                    }
                }
                _ => return Err(terminal_failure_error_v18()),
            };
            if let TerminalFailureSiteV18::Edge { target, .. } = site {
                let failure = &body.blocks[terminal_failure_find_v18(&blocks, &target, budget)?];
                budget.charge_work(argument_sum_v1(&[
                    failure.parameters.len(),
                    failure.operations.len(),
                    3,
                ])?)?;
                if !failure.parameters.is_empty()
                    || failure.operations.len() != 1
                    || !matches!(failure.terminator, Some(Terminator::Unreachable))
                {
                    return Err(terminal_failure_error_v18());
                }
                if !terminal_failure_is_trap_v18(&failure.operations[0], budget)? {
                    return Err(terminal_failure_error_v18());
                }
                if terminal_failure_find_v18(&traps, &instance.index(), budget)?
                    != target.0 as usize
                {
                    return Err(terminal_failure_error_v18());
                }
            }
            rows.push(TerminalFailureOriginV18 {
                instance,
                function: original.function(),
                block,
                kind,
                control,
                source_span,
                site,
                normal,
            });
        }
    }
    drop((blocks, controls, spans, assertions, traps));
    let scratch = budget
        .storage()
        .checked_sub(scratch_floor)
        .ok_or(ArgumentResourceV1::Accounting)?;
    budget.release_storage(scratch)?;
    Ok(TerminalFailureOriginsV18 {
        source,
        ledger: budget.work_ledger_identity_v1(),
        rows,
    })
}
