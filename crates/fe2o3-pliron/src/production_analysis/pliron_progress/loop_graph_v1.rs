#[derive(Clone, Copy)]
struct IncomingEdgeV1 {
    source: usize,
    ordinal: usize,
}

struct RootGraphV1 {
    edges: Vec<Vec<usize>>,
    unconditional_edges: Vec<Vec<usize>>,
    predecessors: Vec<Vec<usize>>,
    incoming: Vec<Vec<IncomingEdgeV1>>,
}

fn build_root_graph(
    context: &Context,
    blocks: &[Ptr<BasicBlock>],
    block_indices: &HashMap<Ptr<BasicBlock>, usize>,
) -> Result<RootGraphV1, PlironProgressFindingV1> {
    let mut edges = vec![Vec::new(); blocks.len()];
    let mut unconditional_edges = vec![Vec::new(); blocks.len()];
    let mut predecessors = vec![Vec::new(); blocks.len()];
    let mut incoming = vec![Vec::new(); blocks.len()];
    for (source, block) in blocks.iter().copied().enumerate() {
        let Some(terminator) = block.deref(context).get_terminator(context) else {
            return Err(structural_rejection(format!(
                "block {source} has no registered terminator after structural verification"
            )));
        };
        let operation = Operation::get_op_dyn(terminator, context);
        let control = ControlViewV1::observe(context, terminator).map_err(|_| {
            structural_rejection(format!(
                "block {source} has unsupported or malformed control edges"
            ))
        })?;
        let is_unconditional = progress_unconditional_v1(context, &operation);
        for ordinal in 0..control.successor_count() {
            let edge = control.edge(ordinal).map_err(|_| {
                structural_rejection(format!("block {source} has malformed successor arguments"))
            })?;
            let successor = edge.target();
            let Some(target) = block_indices.get(&successor).copied() else {
                return Err(structural_rejection(format!(
                    "block {source} has a successor outside the function after structural verification"
                )));
            };
            edges[source].push(target);
            predecessors[target].push(source);
            incoming[target].push(IncomingEdgeV1 { source, ordinal });
            if is_unconditional {
                unconditional_edges[source].push(target);
            }
        }
    }
    Ok(RootGraphV1 {
        edges,
        unconditional_edges,
        predecessors,
        incoming,
    })
}

enum CanonicalLoopResultV1 {
    Proved(PlironProgressCertificateV1),
    Inactive,
    Rejected {
        reason: &'static str,
        counterexample: String,
    },
    Incomplete(&'static str),
}

#[allow(clippy::too_many_arguments)]
fn canonical_positive_induction_loop(
    context: &Context,
    blocks: &[Ptr<BasicBlock>],
    block_indices: &HashMap<Ptr<BasicBlock>, usize>,
    operation_blocks: &HashMap<Ptr<Operation>, usize>,
    dominators: &[HashSet<usize>],
    predecessors: &[Vec<usize>],
    edges: &[Vec<usize>],
    incoming: &[Vec<IncomingEdgeV1>],
    component: &[usize],
    component_members: &HashSet<usize>,
) -> CanonicalLoopResultV1 {
    for header_index in component {
        let header = blocks[*header_index];
        let header_block = header.deref(context);
        let Some(terminator) = header_block.get_terminator(context) else {
            continue;
        };
        let terminator = Operation::get_op_dyn(terminator, context);
        let Some(guard) = progress_header_view_v1(context, &terminator, operation_blocks) else {
            continue;
        };
        if header_block.get_num_arguments() != 1
            || guard.induction != header_block.get_argument(0)
            || successor_arguments(context, &terminator, guard.body_ordinal).as_deref()
                != Some(&[guard.induction])
        {
            continue;
        }
        let successors = terminator
            .get_operation()
            .deref(context)
            .successors()
            .collect::<Vec<_>>();
        if successors.len() != 2 {
            continue;
        }
        let body = &successors[guard.body_ordinal];
        let exit = &successors[guard.exit_ordinal];
        let (Some(body_index), Some(exit_index)) = (
            block_indices.get(body).copied(),
            block_indices.get(exit).copied(),
        ) else {
            continue;
        };
        if !component_members.contains(&body_index) || component_members.contains(&exit_index) {
            continue;
        }
        let internal_header_predecessors = predecessors[*header_index]
            .iter()
            .copied()
            .filter(|predecessor| component_members.contains(predecessor))
            .collect::<Vec<_>>();
        let external_header_predecessors = predecessors[*header_index]
            .iter()
            .copied()
            .filter(|predecessor| !component_members.contains(predecessor))
            .collect::<Vec<_>>();
        if internal_header_predecessors.len() != 1 || external_header_predecessors.is_empty() {
            return CanonicalLoopResultV1::Incomplete(
                "the loop header does not have exactly one external entry and one internal recurrence",
            );
        }
        if (external_header_predecessors.len() > 1
            || guard.domain != NativeProgressDomainV1::LegacyIndex)
            && progress_multi_entry_initial_v1(
                context,
                blocks,
                block_indices,
                operation_blocks,
                dominators,
                component_members,
                *header_index,
                &external_header_predecessors,
                0,
            )
            .is_err()
        {
            return CanonicalLoopResultV1::Incomplete(
                "the loop entries do not share one outside dominating typed induction seed",
            );
        }
        if !progress_external_bound_v1(
            context,
            guard.bound,
            guard.domain,
            block_indices,
            operation_blocks,
            dominators,
            component_members,
            *header_index,
        ) || (guard.domain != NativeProgressDomainV1::LegacyIndex
            && external_header_predecessors.iter().any(|entry| {
                !progress_external_bound_v1(
                    context,
                    guard.bound,
                    guard.domain,
                    block_indices,
                    operation_blocks,
                    dominators,
                    component_members,
                    *entry,
                )
            }))
        {
            return CanonicalLoopResultV1::Incomplete(
                "the loop bound is not an outside dominating invariant value",
            );
        }
        let latch_index = internal_header_predecessors[0];
        if component.iter().copied().any(|block| {
            block != *header_index
                && predecessors[block]
                    .iter()
                    .any(|predecessor| !component_members.contains(predecessor))
        }) {
            return CanonicalLoopResultV1::Incomplete(
                "a loop body block has an external predecessor that bypasses the guarded header",
            );
        }
        if component
            .iter()
            .copied()
            .any(|block| blocks[block].deref(context).get_num_arguments() != 1)
        {
            return CanonicalLoopResultV1::Incomplete(
                "every block in the guarded recurrence must carry exactly one induction argument",
            );
        }
        if !acyclic_after_removing_backedge(
            component,
            component_members,
            predecessors,
            edges,
            latch_index,
            *header_index,
        ) {
            return CanonicalLoopResultV1::Incomplete(
                "the loop body retains a control-flow cycle that can bypass the induction update",
            );
        }

        let mut next = None;
        let mut latch_induction = None;
        for source_index in component.iter().copied() {
            let source_block = blocks[source_index].deref(context);
            let source_induction = source_block.get_argument(0);
            let Some(terminator) = source_block.get_terminator(context) else {
                return CanonicalLoopResultV1::Incomplete(
                    "a loop recurrence block has no terminator",
                );
            };
            let terminator = Operation::get_op_dyn(terminator, context);
            let successors = terminator
                .get_operation()
                .deref(context)
                .successors()
                .collect::<Vec<_>>();
            for (ordinal, successor) in successors.iter().copied().enumerate() {
                let Some(successor_index) = block_indices.get(&successor).copied() else {
                    return CanonicalLoopResultV1::Incomplete(
                        "a loop recurrence edge leaves the kernel function",
                    );
                };
                if !component_members.contains(&successor_index) {
                    continue;
                }
                let Some(arguments) = successor_arguments(context, &terminator, ordinal) else {
                    return CanonicalLoopResultV1::Incomplete(
                        "an internal loop edge does not expose exact SSA successor arguments",
                    );
                };
                let [forwarded] = arguments.as_slice() else {
                    return CanonicalLoopResultV1::Incomplete(
                        "an internal loop edge does not carry exactly one induction value",
                    );
                };
                if source_index == latch_index && successor_index == *header_index {
                    next = Some(*forwarded);
                    latch_induction = Some(source_induction);
                } else if *forwarded != source_induction {
                    return CanonicalLoopResultV1::Incomplete(
                        "an internal loop edge does not forward the induction value unchanged",
                    );
                }
            }
        }
        let (Some(latch_induction), Some(next)) = (latch_induction, next) else {
            return CanonicalLoopResultV1::Incomplete(
                "the unique loop latch does not carry an authenticated induction update",
            );
        };
        if guard.domain != NativeProgressDomainV1::LegacyIndex {
            let Some(step) =
                progress_induction_offset_v1(context, next, latch_induction, guard.domain)
            else {
                return CanonicalLoopResultV1::Incomplete(
                    "the native recurrence needs an exact nonnegative constant Add value fitting the current u64 certificate step",
                );
            };
            if step == 0 {
                return native_zero_step_result_v1(
                    context,
                    blocks,
                    block_indices,
                    component_members,
                    &incoming[*header_index],
                    *header_index,
                    latch_index,
                    0,
                    guard,
                    "the native induction step is zero",
                );
            }
            if guard.domain == NativeProgressDomainV1::IndexUnknown && step > 1 {
                return CanonicalLoopResultV1::Incomplete(
                    "a native Index non-unit step needs retained source/target width custody",
                );
            }
            if !progress_step_has_no_wrap_v1(context, guard, step) {
                return CanonicalLoopResultV1::Incomplete(
                    "the native induction update lacks an exact finite-width no-wrap bound",
                );
            }
            return CanonicalLoopResultV1::Proved(PlironProgressCertificateV1 {
                header: *header_index,
                body: body_index,
                exit: exit_index,
                induction: guard.induction.id(context).into(),
                bound: guard.bound.id(context).into(),
                step,
            });
        }
        if next == latch_induction {
            return zero_step_result(
                context,
                blocks,
                component_members,
                &incoming[*header_index],
                guard.bound,
                "the induction variable is unchanged on the backedge",
            );
        }
        let Some(increment_definition) = next.defining_op() else {
            return CanonicalLoopResultV1::Incomplete(
                "the backedge value is not a locally reconstructed induction update",
            );
        };
        let increment = Operation::get_op_dyn(increment_definition, context);
        let Some(increment) = increment.downcast_ref::<IndexBinaryOp>() else {
            return CanonicalLoopResultV1::Incomplete(
                "the induction update is not target-neutral index addition",
            );
        };
        if increment.kind(context) != Some(IndexBinaryKindAttr::Add)
            || increment.lhs(context) != latch_induction
        {
            return CanonicalLoopResultV1::Incomplete("the induction update is not `i + constant`");
        }
        let Some(step_definition) = increment.rhs(context).defining_op() else {
            return CanonicalLoopResultV1::Incomplete("the induction step is not constant");
        };
        let step = Operation::get_op_dyn(step_definition, context);
        let Some(step) = step.downcast_ref::<IndexConstantOp>() else {
            return CanonicalLoopResultV1::Incomplete("the induction step is not constant");
        };
        let step = match step.value(context) {
            Some(0) => {
                return zero_step_result(
                    context,
                    blocks,
                    component_members,
                    &incoming[*header_index],
                    guard.bound,
                    "the induction step is zero",
                );
            }
            Some(step) => step,
            None => return CanonicalLoopResultV1::Incomplete("the induction step is malformed"),
        };
        if step > 1 {
            let upper_bound = index_constant(context, guard.bound)
                .or_else(|| unsigned_cast_upper_bound(context, guard.bound));
            let Some(bound) = upper_bound else {
                return CanonicalLoopResultV1::Incomplete(
                    "a symbolic bound with a non-unit step needs a no-wrap range proof",
                );
            };
            if bound != 0 && (bound - 1).checked_add(step).is_none() {
                return CanonicalLoopResultV1::Incomplete(
                    "the largest guarded induction value plus the step can overflow u64",
                );
            }
        }
        return CanonicalLoopResultV1::Proved(PlironProgressCertificateV1 {
            header: *header_index,
            body: body_index,
            exit: exit_index,
            induction: guard.induction.id(context).into(),
            bound: guard.bound.id(context).into(),
            step,
        });
    }
    CanonicalLoopResultV1::Incomplete(
        "the cycle has no supported `i < bound; i := i + positive_constant` header and backedge",
    )
}

#[allow(clippy::too_many_arguments)]
fn progress_multi_entry_initial_v1(
    context: &Context,
    blocks: &[Ptr<BasicBlock>],
    block_indices: &HashMap<Ptr<BasicBlock>, usize>,
    operation_blocks: &HashMap<Ptr<Operation>, usize>,
    dominators: &[HashSet<usize>],
    members: &HashSet<usize>,
    header_index: usize,
    entries: &[usize],
    induction_argument: usize,
) -> Result<pliron::value::Value, ()> {
    use pliron::r#type::Typed;

    let header = blocks[header_index];
    let header_ref = header.deref(context);
    let mut initial = None;
    let mut previous = None;
    for entry in entries.iter().copied() {
        // Root predecessor rows are in source order and retain occurrences.
        // One query authenticates every parallel occurrence from this source.
        if previous == Some(entry) {
            continue;
        }
        if previous.is_some_and(|previous| previous > entry) || members.contains(&entry) {
            return Err(());
        }
        previous = Some(entry);
        let arguments = progress_edge_arguments_v1(context, blocks[entry], header)?;
        if arguments.len() != header_ref.get_num_arguments() {
            return Err(());
        }
        for (argument, value) in arguments.iter().copied().enumerate() {
            if value.get_type(context) != header_ref.get_argument(argument).get_type(context) {
                return Err(());
            }
        }
        let value = *arguments.get(induction_argument).ok_or(())?;
        let definition = if let Some(operation) = value.defining_op() {
            *operation_blocks.get(&operation).ok_or(())?
        } else {
            *block_indices
                .get(&value.defining_block().ok_or(())?)
                .ok_or(())?
        };
        if members.contains(&definition) || !dominators[entry].contains(&definition) {
            return Err(());
        }
        if initial.is_some_and(|initial| initial != value) {
            return Err(());
        }
        initial = Some(value);
    }
    initial.ok_or(())
}

fn successor_arguments(
    context: &Context,
    terminator: &OpBox,
    ordinal: usize,
) -> Option<Vec<pliron::value::Value>> {
    let control = ControlViewV1::observe(context, terminator.get_operation()).ok()?;
    let edge = control.edge(ordinal).ok()?;
    Some(
        (0..edge.argument_count())
            .map(|index| {
                edge.argument_at(index)
                    .expect("authenticated in-range edge argument")
                    .0
            })
            .collect(),
    )
}

fn acyclic_after_removing_backedge(
    component: &[usize],
    component_members: &HashSet<usize>,
    predecessors: &[Vec<usize>],
    edges: &[Vec<usize>],
    latch: usize,
    header: usize,
) -> bool {
    let mut indegree = HashMap::with_capacity(component.len());
    for block in component.iter().copied() {
        let count = predecessors[block]
            .iter()
            .copied()
            .filter(|predecessor| {
                component_members.contains(predecessor)
                    && !(*predecessor == latch && block == header)
            })
            .count();
        indegree.insert(block, count);
    }
    let mut ready = indegree
        .iter()
        .filter_map(|(block, count)| (*count == 0).then_some(*block))
        .collect::<Vec<_>>();
    let mut visited = 0_usize;
    while let Some(block) = ready.pop() {
        visited += 1;
        for successor in edges[block].iter().copied().filter(|successor| {
            component_members.contains(successor) && !(block == latch && *successor == header)
        }) {
            let Some(count) = indegree.get_mut(&successor) else {
                return false;
            };
            *count -= 1;
            if *count == 0 {
                ready.push(successor);
            }
        }
    }
    visited == component.len()
}

fn zero_step_result(
    context: &Context,
    blocks: &[Ptr<BasicBlock>],
    component_members: &HashSet<usize>,
    incoming: &[IncomingEdgeV1],
    bound: pliron::value::Value,
    reason: &'static str,
) -> CanonicalLoopResultV1 {
    let Some(bound) = index_constant(context, bound) else {
        return CanonicalLoopResultV1::Incomplete(
            "the zero-step cycle needs a feasible true-edge witness, but its bound is symbolic",
        );
    };
    let mut saw_predecessor = false;
    let mut saw_unknown = false;
    for edge in incoming {
        if component_members.contains(&edge.source) {
            continue;
        }
        saw_predecessor = true;
        let initial = blocks[edge.source]
            .deref(context)
            .get_terminator(context)
            .and_then(|pointer| {
                let operation = Operation::get_op_dyn(pointer, context);
                operation
                    .downcast_ref::<BranchArgsOp>()
                    .and_then(|branch| (edge.ordinal == 0).then(|| branch.arguments(context)))
                    .and_then(|arguments| arguments.first().copied())
                    .and_then(|value| index_constant(context, value))
            });
        let Some(initial) = initial else {
            saw_unknown = true;
            continue;
        };
        if initial < bound {
            return CanonicalLoopResultV1::Rejected {
                reason,
                counterexample: format!(
                    "the live incoming edge carries i = {initial} and bound = {bound}, so the true edge repeats forever"
                ),
            };
        }
    }
    if saw_predecessor && !saw_unknown {
        CanonicalLoopResultV1::Inactive
    } else {
        CanonicalLoopResultV1::Incomplete(
            "the zero-step cycle has no reconstructed feasible incoming value",
        )
    }
}

fn index_constant(context: &Context, value: pliron::value::Value) -> Option<u64> {
    let definition = value.defining_op()?;
    Operation::get_op_dyn(definition, context)
        .downcast_ref::<IndexConstantOp>()?
        .value(context)
}

fn unsigned_cast_upper_bound(context: &Context, value: pliron::value::Value) -> Option<u64> {
    let definition = value.defining_op()?;
    let operation = Operation::get_op_dyn(definition, context);
    let cast = operation.downcast_ref::<IndexUnsignedCastOp>()?;
    (cast.result(context) == value && cast.verify(context).is_ok())
        .then(|| cast.inclusive_upper_bound(context))
        .flatten()
}

fn reachable_blocks(edges: &[Vec<usize>]) -> Vec<bool> {
    let mut reachable = vec![false; edges.len()];
    if edges.is_empty() {
        return reachable;
    }
    let mut stack = vec![0];
    while let Some(block) = stack.pop() {
        if reachable[block] {
            continue;
        }
        reachable[block] = true;
        stack.extend(edges[block].iter().copied());
    }
    reachable
}

pub(crate) fn strongly_connected_components(edges: &[Vec<usize>]) -> Vec<Vec<usize>> {
    let mut reverse = vec![Vec::new(); edges.len()];
    for (from, successors) in edges.iter().enumerate() {
        for successor in successors {
            reverse[*successor].push(from);
        }
    }
    let mut visited = vec![false; edges.len()];
    let mut order = Vec::with_capacity(edges.len());
    for root in 0..edges.len() {
        if visited[root] {
            continue;
        }
        visited[root] = true;
        let mut stack = vec![(root, 0_usize)];
        while let Some((block, next)) = stack.pop() {
            if let Some(successor) = edges[block].get(next).copied() {
                stack.push((block, next + 1));
                if !visited[successor] {
                    visited[successor] = true;
                    stack.push((successor, 0));
                }
            } else {
                order.push(block);
            }
        }
    }
    visited.fill(false);
    let mut components = Vec::new();
    for root in order.into_iter().rev() {
        if visited[root] {
            continue;
        }
        visited[root] = true;
        let mut component = Vec::new();
        let mut stack = vec![root];
        while let Some(block) = stack.pop() {
            component.push(block);
            for predecessor in &reverse[block] {
                if !visited[*predecessor] {
                    visited[*predecessor] = true;
                    stack.push(*predecessor);
                }
            }
        }
        components.push(component);
    }
    components
}

fn is_cycle(component: &[usize], edges: &[Vec<usize>]) -> bool {
    component.len() > 1
        || component
            .first()
            .is_some_and(|block| edges[*block].contains(block))
}

fn report(finding: PlironProgressFindingV1) -> PlironProgressReportV1 {
    PlironProgressReportV1 {
        findings: vec![finding],
        certificates: Vec::new(),
    }
}
