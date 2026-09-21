use crate::production_analysis::pliron_pipeline::invocation_receipt_v1::InvocationObserverV1;
use fe2o3_kernel_analysis::PresburgerFailureV1;

type RankedBoundsObserverV1<'o, 'p, 'r> = Option<&'o InvocationObserverV1<'p, 'r>>;

fn observe_bounds_presburger_failure_v1(
    observer: RankedBoundsObserverV1<'_, '_, '_>,
    failure: &PresburgerFailureV1,
) {
    if let (Some(observer), PresburgerFailureV1::ResourceLimit { .. }) = (observer, failure) {
        observer.deny(ranked_bounds_resource_error_v1(
            "Presburger query work limit",
        ));
    }
}

fn observe_bounds_sparse_failure_v1(
    observer: RankedBoundsObserverV1<'_, '_, '_>,
    failure: &SparseIndexFailureV1,
) {
    if let (Some(observer), SparseIndexFailureV1::ResourceLimit { resource, .. }) =
        (observer, failure)
    {
        observer.deny(ProductionAnalysisResourceLimitV1 {
            phase: ProductionAnalysisResourcePhaseV1::SparseIndex,
            resource,
        });
    }
}

/// Runs the target-neutral ranked-memory bounds stage for one Pliron function.
///
/// The function must already contain `kernel.ranked_view`, `kernel.dim`,
/// `kernel.access`, and the closed kernel CFG terminators. No GEMM operation,
/// schedule, tile, target, or device profile participates in this analysis.
#[cfg(test)]
pub(crate) fn run_pliron_ranked_bounds_check_v1(
    context: &Context,
    function: &FuncOp,
) -> RankedBoundsReportV1 {
    let mut analyses = PlironAnalysisManagerV1::new(function);
    run_pliron_ranked_bounds_check_with_analyses_v1(context, function, &mut analyses)
}

#[cfg(test)]
pub(crate) fn run_pliron_ranked_bounds_check_with_analyses_v1(
    context: &Context,
    function: &FuncOp,
    analyses: &mut PlironAnalysisManagerV1,
) -> RankedBoundsReportV1 {
    run_pliron_ranked_bounds_check_with_observation_v1(context, function, analyses, None)
}

pub(crate) fn run_pliron_ranked_bounds_check_with_observation_v1(
    context: &Context,
    function: &FuncOp,
    analyses: &mut PlironAnalysisManagerV1,
    observer: RankedBoundsObserverV1<'_, '_, '_>,
) -> RankedBoundsReportV1 {
    match observer {
        None => run_pliron_ranked_bounds_inner_v1(context, function, analyses, None),
        Some(observer) => observer.with_projection(&Ok, |nested| {
            run_pliron_ranked_bounds_inner_v1(context, function, analyses, Some(nested))
        }),
    }
}

fn run_pliron_ranked_bounds_inner_v1(
    context: &Context,
    function: &FuncOp,
    analyses: &mut PlironAnalysisManagerV1,
    observer: RankedBoundsObserverV1<'_, '_, '_>,
) -> RankedBoundsReportV1 {
    let mut budget = RankedBoundsBudget::default();
    analyses.prepare_function_inventory(context, function);
    let inventory = match analyses.function_inventory_handle() {
        Ok(inventory) => inventory,
        Err(failure) => {
            return resource_failure(
                failure.resource(),
                failure.limit(),
                failure.actual(),
                observer,
            );
        }
    };
    let blocks = inventory.blocks();
    for _block in blocks {
        if let Err(finding) = budget.reserve(RankedBoundsResource::Blocks, 1) {
            return finding_failure(finding, observer);
        }
        if let Err(finding) = budget.storage(1) {
            return finding_failure(finding, observer);
        }
    }
    if blocks.is_empty() {
        return structural_failure();
    }

    // Close the accepted language before recursive Pliron verification. Every
    // admitted body operation is regionless, so an unknown operation cannot
    // hide an unmetered nested graph from this analysis.
    for (block_index, block) in blocks.iter().enumerate() {
        let terminator = block.deref(context).get_terminator(context);
        for site in inventory.block_operations(block_index) {
            let operation_index = site.operation();
            let operation_pointer = site.pointer();
            if let Err(finding) = budget.reserve(RankedBoundsResource::Operations, 1) {
                return finding_failure(finding, observer);
            }
            let operation = Operation::get_op_dyn(operation_pointer, context);
            let Some(kind) = ranked_operation_kind(operation.as_ref()) else {
                let finding = if terminator == Some(operation_pointer) {
                    RankedBoundsFindingV1::UnsupportedTerminator {
                        block: block_index,
                        operation: operation.get_opid().to_string(),
                    }
                } else {
                    RankedBoundsFindingV1::UnsupportedOperation {
                        block: block_index,
                        operation: operation_index,
                        kind: operation.get_opid().to_string(),
                    }
                };
                return finding_failure(finding, observer);
            };
            if kind.is_terminator() != (terminator == Some(operation_pointer)) {
                return structural_failure();
            }

            let raw = operation_pointer.deref(context);
            if raw.num_regions() != 0 {
                return structural_failure();
            }
            let Some(operation_items) = raw
                .get_num_operands()
                .checked_add(raw.get_num_results())
                .and_then(|total| total.checked_add(raw.get_num_successors()))
                .and_then(|total| total.checked_add(raw.attributes.0.len()))
            else {
                return resource_failure(
                    RankedBoundsResource::OperationItems.description(),
                    RankedBoundsResource::OperationItems.limit(),
                    usize::MAX,
                    observer,
                );
            };
            if let Err(finding) =
                budget.reserve(RankedBoundsResource::OperationItems, operation_items)
            {
                return finding_failure(finding, observer);
            }
            if let Err(finding) =
                budget.reserve(RankedBoundsResource::Edges, raw.get_num_successors())
            {
                return finding_failure(finding, observer);
            }
            if let Err(finding) = budget.work(operation_items.saturating_add(1)) {
                return finding_failure(finding, observer);
            }
        }
    }

    if verify_operation(function.get_operation(), context).is_err() {
        return structural_failure();
    }

    analyses.prepare_sparse_indices(context, function);
    analyses.prepare_presburger(context, function);
    let sparse_indices = match analyses.sparse_indices() {
        Ok(analysis) => analysis,
        Err(failure) => {
            observe_bounds_sparse_failure_v1(observer, &failure);
            return finding_failure(sparse_index_failure(failure), observer);
        }
    };
    let presburger = match analyses.presburger() {
        Ok(analysis) => analysis,
        Err(failure) => {
            observe_bounds_sparse_failure_v1(observer, &failure);
            return finding_failure(sparse_index_failure(failure), observer);
        }
    };

    if let Err(finding) = budget.storage(blocks.len().saturating_mul(3)) {
        return finding_failure(finding, observer);
    }
    if let Err(finding) = budget.work(blocks.len()) {
        return finding_failure(finding, observer);
    }
    let indices = blocks
        .iter()
        .enumerate()
        .map(|(index, block)| (*block, index))
        .collect::<HashMap<_, _>>();
    let mut successors = vec![Vec::new(); blocks.len()];
    let mut predecessors = vec![Vec::new(); blocks.len()];
    let mut findings = Vec::new();
    let mut fact_indices = HashMap::new();
    let mut has_forwarded_arguments = false;

    for (block_index, block) in blocks.iter().enumerate() {
        if let Err(finding) = budget.work(1) {
            return finding_failure(finding, observer);
        }
        has_forwarded_arguments |=
            block_index != 0 && block.deref(context).get_num_arguments() != 0;
        let Some(terminator) = block.deref(context).get_terminator(context) else {
            return structural_failure();
        };
        let operation = Operation::get_op_dyn(terminator, context);
        let operands = operation
            .downcast_ref::<IndexLessThanBranchOp>()
            .map(|branch| (branch.lhs(context), branch.rhs(context)))
            .or_else(|| {
                operation
                    .downcast_ref::<IndexLessThanBranchArgsOp>()
                    .map(|branch| (branch.lhs(context), branch.rhs(context)))
            });
        let guard_fact = if let Some((lhs, rhs)) = operands {
            let fact = LessThanFact {
                lhs: canonical_index_expr(lhs, context),
                rhs: canonical_index_expr(rhs, context),
            };
            if let Some(index) = fact_indices.get(&fact).copied() {
                Some(index)
            } else {
                if let Err(finding) = budget.reserve(RankedBoundsResource::Facts, 1) {
                    return finding_failure(finding, observer);
                }
                if let Err(finding) = budget.storage(1) {
                    return finding_failure(finding, observer);
                }
                let next = fact_indices.len();
                fact_indices.insert(fact, next);
                Some(next)
            }
        } else {
            None
        };

        let raw = terminator.deref(context);
        for (successor_index, successor) in raw.successors().enumerate() {
            if let Err(finding) = budget.work(2) {
                return finding_failure(finding, observer);
            }
            let Some(target) = indices.get(&successor).copied() else {
                if let Err(finding) = push_finding(&mut findings, &mut budget, || {
                    RankedBoundsFindingV1::UnsupportedTerminator {
                        block: block_index,
                        operation: "successor outside function region".to_owned(),
                    }
                }) {
                    return finding_failure(finding, observer);
                }
                continue;
            };
            successors[block_index].push(target);
            if let Err(finding) = budget.storage(1) {
                return finding_failure(finding, observer);
            }
            predecessors[target].push(PredecessorEdge {
                block: block_index,
                successor: successor_index,
                guard_fact: (successor_index == 0).then_some(guard_fact).flatten(),
            });
        }
    }

    if let Err(finding) = budget.storage(budget.edges.saturating_mul(2)) {
        return finding_failure(finding, observer);
    }
    if let Err(finding) = budget.storage(blocks.len().saturating_mul(2)) {
        return finding_failure(finding, observer);
    }
    let reachable = match reachable_blocks(&successors, &mut budget) {
        Ok(reachable) => reachable,
        Err(finding) => return finding_failure(finding, observer),
    };
    for (block, is_reachable) in reachable.iter().copied().enumerate() {
        if !is_reachable
            && let Err(finding) = push_finding(&mut findings, &mut budget, || {
                RankedBoundsFindingV1::UnreachableBlock { block }
            })
        {
            return finding_failure(finding, observer);
        }
    }

    let fact_count = fact_indices.len();
    let fact_words = fact_count.div_ceil(u64::BITS as usize);
    let Some(input_words) = blocks.len().checked_mul(fact_words) else {
        return resource_failure(
            RankedBoundsResource::StorageItems.description(),
            RankedBoundsResource::StorageItems.limit(),
            usize::MAX,
            observer,
        );
    };
    let Some(dataflow_storage) = blocks
        .len()
        .checked_mul(3)
        .and_then(|outer| outer.checked_add(input_words))
    else {
        return resource_failure(
            RankedBoundsResource::StorageItems.description(),
            RankedBoundsResource::StorageItems.limit(),
            usize::MAX,
            observer,
        );
    };
    if let Err(finding) = budget.storage(dataflow_storage) {
        return finding_failure(finding, observer);
    }
    let mut inputs = (0..blocks.len())
        .map(|block| {
            if block == 0 {
                FactSet::empty(fact_count)
            } else {
                FactSet::full(fact_count)
            }
        })
        .collect::<Vec<_>>();
    let mut pending = (0..blocks.len())
        .filter(|block| !has_forwarded_arguments && reachable[*block])
        .collect::<VecDeque<_>>();
    let mut queued = reachable.clone();
    while let Some(block) = pending.pop_front() {
        queued[block] = false;
        let next = if block == 0 {
            FactSet::empty(fact_count)
        } else {
            match intersect_predecessor_facts(
                block,
                &predecessors,
                &inputs,
                fact_count,
                &mut budget,
            ) {
                Ok(next) => next,
                Err(finding) => return finding_failure(finding, observer),
            }
        };
        if let Err(finding) = budget.work(1) {
            return finding_failure(finding, observer);
        }
        if next != inputs[block] {
            inputs[block] = next;
            for successor in &successors[block] {
                if let Err(finding) = budget.work(1) {
                    return finding_failure(finding, observer);
                }
                if reachable[*successor] && !queued[*successor] {
                    queued[*successor] = true;
                    pending.push_back(*successor);
                }
            }
        }
    }

    let graph = BoundsEdgeTransportV1 {
        context,
        blocks,
        predecessors: &predecessors,
    };
    let transport = has_forwarded_arguments.then_some(&graph);
    for block_index in 0..blocks.len() {
        if !reachable[block_index] {
            continue;
        }
        for site in inventory.block_operations(block_index) {
            let operation_index = site.operation();
            if let Err(finding) = budget.work(1) {
                return finding_failure(finding, observer);
            }
            let operation = Operation::get_op_dyn(site.pointer(), context);
            if let Some(access) = operation.downcast_ref::<RankedAccessOp>()
                && let Err(finding) = verify_access(
                    access,
                    block_index,
                    operation_index,
                    &mut AccessCheck {
                        facts: &inputs[block_index],
                        fact_indices: &fact_indices,
                        transport,
                        graph: &graph,
                        sparse_indices,
                        presburger,
                        findings: &mut findings,
                        budget: &mut budget,
                    },
                    observer,
                )
            {
                return finding_failure(finding, observer);
            }
        }
    }

    RankedBoundsReportV1 { findings }
}

/// Enforces the ranked-memory stage as a compile-time pre-lowering gate.
///
/// A clean result is still descriptive and grants no authority. Any malformed,
/// static-out-of-bounds, or unproved access returns a terminal error; callers
/// must not fall back to unchecked lowering.
#[cfg(test)]
pub(crate) fn require_pliron_ranked_bounds_before_lowering_v1(
    context: &Context,
    function: &FuncOp,
) -> Result<RankedBoundsReportV1, RankedBoundsCheckErrorV1> {
    let report = run_pliron_ranked_bounds_check_v1(context, function);
    if report.is_clean() {
        Ok(report)
    } else {
        Err(RankedBoundsCheckErrorV1 { report })
    }
}

pub(crate) fn require_pliron_ranked_bounds_with_observation_v1(
    context: &Context,
    function: &FuncOp,
    analyses: &mut PlironAnalysisManagerV1,
    observer: RankedBoundsObserverV1<'_, '_, '_>,
) -> Result<RankedBoundsReportV1, RankedBoundsCheckErrorV1> {
    let report =
        run_pliron_ranked_bounds_check_with_observation_v1(context, function, analyses, observer);
    if report.is_clean() {
        Ok(report)
    } else {
        Err(RankedBoundsCheckErrorV1 { report })
    }
}

fn resource_failure(
    resource: &'static str,
    limit: usize,
    actual: usize,
    observer: RankedBoundsObserverV1<'_, '_, '_>,
) -> RankedBoundsReportV1 {
    finding_failure(
        RankedBoundsFindingV1::ResourceLimitExceeded {
            resource,
            limit,
            actual,
        },
        observer,
    )
}

fn structural_failure() -> RankedBoundsReportV1 {
    finding_failure(RankedBoundsFindingV1::StructuralVerificationFailed, None)
}

fn finding_failure(
    finding: RankedBoundsFindingV1,
    observer: RankedBoundsObserverV1<'_, '_, '_>,
) -> RankedBoundsReportV1 {
    if let (Some(observer), RankedBoundsFindingV1::ResourceLimitExceeded { resource, .. }) =
        (observer, &finding)
    {
        observer.deny(ranked_bounds_resource_error_v1(resource));
    }
    RankedBoundsReportV1 {
        findings: vec![finding],
    }
}

fn push_finding(
    findings: &mut Vec<RankedBoundsFindingV1>,
    budget: &mut RankedBoundsBudget,
    make_finding: impl FnOnce() -> RankedBoundsFindingV1,
) -> Result<(), RankedBoundsFindingV1> {
    budget.reserve(RankedBoundsResource::Findings, 1)?;
    budget.storage(1)?;
    findings.push(make_finding());
    Ok(())
}

fn reachable_blocks(
    successors: &[Vec<usize>],
    budget: &mut RankedBoundsBudget,
) -> Result<Vec<bool>, RankedBoundsFindingV1> {
    let mut reachable = vec![false; successors.len()];
    let mut pending = vec![0];
    reachable[0] = true;
    while let Some(block) = pending.pop() {
        budget.work(1)?;
        for successor in &successors[block] {
            budget.work(1)?;
            if !reachable[*successor] {
                reachable[*successor] = true;
                pending.push(*successor);
            }
        }
    }
    Ok(reachable)
}

fn intersect_predecessor_facts(
    block: usize,
    predecessors: &[Vec<PredecessorEdge>],
    inputs: &[FactSet],
    fact_count: usize,
    budget: &mut RankedBoundsBudget,
) -> Result<FactSet, RankedBoundsFindingV1> {
    let mut edges = predecessors[block].iter();
    let Some(first) = edges.next() else {
        budget.work(1)?;
        return Ok(FactSet::empty(fact_count));
    };
    budget.work(inputs[first.block].words.len().saturating_add(1))?;
    let mut result = inputs[first.block].clone();
    if let Some(fact) = first.guard_fact {
        result.insert(fact);
    }
    for edge in edges {
        budget.work(result.words.len().saturating_add(1))?;
        result.intersect_edge(&inputs[edge.block], edge.guard_fact);
    }
    Ok(result)
}
