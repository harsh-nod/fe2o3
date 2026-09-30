//! Correlate the checked actual graph with all ordered original source choices.
use super::*;
use index::ProjectionIndexV30;

pub(super) fn push<T>(
    rows: &mut Vec<T>,
    value: T,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<()> {
    scoped_object_reserve_append_v29(rows, budget).map_err(source_emission_error_v18)?;
    rows.push(value);
    Ok(())
}

fn node(
    nodes: &[ActualNode],
    index: usize,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<ActualNode> {
    budget.charge_work(1)?;
    nodes
        .get(index)
        .copied()
        .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
            "selected final actual pointer node",
        ))
}

struct WalkV30 {
    marks: Vec<usize>,
    queue: Vec<usize>,
    generation: usize,
}

impl WalkV30 {
    fn contains(&self, at: usize, budget: &mut ArgumentBudgetV1<'_>) -> SourceOwnedResultV18<bool> {
        budget.charge_work(1)?;
        Ok(self
            .marks
            .get(at)
            .copied()
            .ok_or(ArgumentResourceV1::Accounting)?
            == self.generation)
    }

    fn new(count: usize, budget: &mut ArgumentBudgetV1<'_>) -> SourceOwnedResultV18<Self> {
        let mut marks = resources::vector(count, budget)?;
        budget.charge_work(count)?;
        marks.resize(count, 0);
        Ok(Self {
            marks,
            queue: resources::vector(count, budget)?,
            generation: 0,
        })
    }

    fn add(&mut self, at: usize, budget: &mut ArgumentBudgetV1<'_>) -> SourceOwnedResultV18<()> {
        budget.charge_work(2)?;
        let mark = self
            .marks
            .get_mut(at)
            .ok_or(ArgumentResourceV1::Accounting)?;
        if *mark != self.generation {
            *mark = self.generation;
            if self.queue.len() == self.queue.capacity() {
                return Err(ArgumentResourceV1::Accounting.into());
            }
            self.queue.push(at);
        }
        Ok(())
    }

    fn edges(
        &mut self,
        access: usize,
        root_node: usize,
        nodes: &[ActualNode],
        incoming: &[ActualIncoming],
        index: &ProjectionIndexV30<'_>,
        forwarding: &forwarding::ForwardingV30,
        output: &mut Vec<SelectedFinalEdgeJoinV30>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<()> {
        budget.charge_work(self.queue.len())?;
        self.queue.clear();
        self.generation = argument_sum_v1(&[self.generation, 1])?;
        self.add(root_node, budget)?;
        let mut next = 0;
        while let Some(&at) = self.queue.get(next) {
            next = argument_sum_v1(&[next, 1])?;
            let actual = node(nodes, at, budget)?;
            match actual.step {
                ActualStep::Formation { .. } => {}
                ActualStep::Cast { input } => self.add(input, budget)?,
                ActualStep::Unsupported => {
                    return resources::binding(
                        "selected final unsupported actual pointer equation",
                    );
                }
                ActualStep::Parameter { first, count } => {
                    let end = argument_sum_v1(&[first, count])?;
                    let edges = incoming
                        .get(first..end)
                        .ok_or(ArgumentResourceV1::Accounting)?;
                    for (relative, edge) in edges.iter().enumerate() {
                        budget.charge_work(5)?;
                        let input = node(nodes, edge.argument, budget)?;
                        let projections =
                            index.edge_projections(access, edge.occurrence, budget)?;
                        if projections.is_empty() {
                            let anchor = forwarding
                                .edge(
                                    argument_sum_v1(&[first, relative])?,
                                    at,
                                    nodes,
                                    incoming,
                                    budget,
                                )?
                                .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                                    "selected final actual edge has no exact source occurrence",
                                ))?;
                            push(
                                output,
                                SelectedFinalEdgeJoinV30 {
                                    actual: argument_sum_v1(&[first, relative])?,
                                    original: SelectedFinalEdgeOriginV30::Forwarding { anchor },
                                },
                                budget,
                            )?;
                        }
                        for projection in projections {
                            budget.charge_work(4)?;
                            if projection.relation.output != Some(edge.occurrence)
                                || projection.relation.output_incoming != Some(input.definition)
                                || projection.relation.output_target != Some(actual.definition)
                            {
                                return resources::binding(
                                    "selected final actual edge changed its source endpoints",
                                );
                            }
                            push(
                                output,
                                SelectedFinalEdgeJoinV30 {
                                    actual: argument_sum_v1(&[first, relative])?,
                                    original: SelectedFinalEdgeOriginV30::Source(projection.source),
                                },
                                budget,
                            )?;
                        }
                        // Unreachable rows remain in the exact edge census but
                        // do not prove any reachable leaf or initialization.
                        if edge.reachable {
                            self.add(edge.argument, budget)?;
                        }
                    }
                }
            }
        }
        Ok(())
    }
}

fn role_matches(
    index: &ProjectionIndexV30<'_>,
    access: usize,
    leaf: usize,
    component: usize,
    output: Definition,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<bool> {
    index.matches(
        access,
        SelectedDefinitionRoleV30::Leaf { leaf, component },
        output,
        budget,
    )
}

fn leaf_matches(
    optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
    index: &ProjectionIndexV30<'_>,
    access: usize,
    leaf: usize,
    choice: &ActualChoice,
    pointer: ActualNode,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<bool> {
    let original = index.leaf(access, leaf, budget)?;
    let ActualStep::Formation { operation } = pointer.step else {
        return resources::binding("selected final choice is not an actual address formation");
    };
    let actual = source_operation_row_v18(optimized.checked.output(), operation, budget)?.operation;
    let OperationKind::GetElementPointer { base, offset } = actual.kind else {
        return resources::binding("selected final choice changed address formation");
    };
    let function = operation.block.function;
    let data = definition(optimized.checked.output(), function, base, budget)?;
    let address_index = definition(optimized.checked.output(), function, offset, budget)?;
    let Definition::Result {
        operation: data_operation,
        result: 0,
    } = data
    else {
        return resources::binding("selected final data is not an exact result");
    };
    let data_actual =
        source_operation_row_v18(optimized.checked.output(), data_operation, budget)?.operation;
    let OperationKind::SliceData { slice } = data_actual.kind else {
        return resources::binding("selected final data changed slice producer");
    };
    let receiver = definition(optimized.checked.output(), function, slice, budget)?;
    budget.charge_work(10)?;
    if !matches!(actual.results.as_slice(), [result] if result.id == pointer.value)
        || choice.domain.pointer() != pointer.value
        || choice.domain.index() != offset
    {
        return resources::binding("selected final domain changed exact address operands");
    }
    let matches = match original.origin {
        PendingSourceSelectedLeafOriginV30::Issued(source) => {
            let root = definition(
                optimized.checked.output(),
                function,
                choice.domain.slice(),
                budget,
            )?;
            let length = definition(
                optimized.checked.output(),
                function,
                choice.length_origin,
                budget,
            )?;
            role_matches(index, access, leaf, 0, root, budget)?
                && role_matches(index, access, leaf, 1, receiver, budget)?
                && role_matches(index, access, leaf, 2, address_index, budget)?
                && role_matches(index, access, leaf, 3, length, budget)?
                && role_matches(index, access, leaf, 5, data, budget)?
                && role_matches(index, access, leaf, 6, pointer.definition, budget)?
                && pointer.scalar == Some(source.element)
        }
        PendingSourceSelectedLeafOriginV30::Descriptor(source) => {
            role_matches(index, access, leaf, 0, receiver, budget)?
                && role_matches(index, access, leaf, 2, data, budget)?
                && role_matches(index, access, leaf, 3, address_index, budget)?
                && role_matches(index, access, leaf, 4, pointer.definition, budget)?
                && pointer.scalar == Some(source.element)
        }
    };
    Ok(matches)
}

fn injection_matches(
    optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
    access: usize,
    obligation: PendingSourceSelectedObligationV30,
    choice: &ActualChoice,
    incoming: &[ActualIncoming],
    nodes: &[ActualNode],
    index: &ProjectionIndexV30<'_>,
    walk: &WalkV30,
    forwarding: &mut forwarding::ForwardingV30,
    paths: &mut Vec<SelectedFinalForwardingStepV30>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<bool> {
    match choice.injection {
        // A checked rewrite may collapse a parameter. Retain all of the leaf's
        // original obligations; final M5 still interprets their original paths.
        ActualInjection::Access => Ok(true),
        ActualInjection::Incoming(at) => {
            budget.charge_work(2)?;
            let edge = incoming.get(at).ok_or(ArgumentResourceV1::Accounting)?;
            let projections = index.edge_projections(access, edge.occurrence, budget)?;
            let source = match obligation.usage {
                PendingSourceSelectedUseV30::Access => None,
                PendingSourceSelectedUseV30::Incoming(at) => Some(SourceEdgeV30::Incoming(at)),
                PendingSourceSelectedUseV30::Invocation(at) => Some(SourceEdgeV30::Invocation(at)),
            };
            if !projections.is_empty() {
                let Some(source) = source else {
                    return Ok(false);
                };
                charge_execution_cfg_lookup_v29(projections.len(), budget)
                    .map_err(source_emission_error_v18)?;
                return Ok(projections
                    .binary_search_by_key(&source, |row| row.source)
                    .is_ok());
            }
            if forwarding.edge(at, edge.parameter, nodes, incoming, budget)? != Some(choice.leaf) {
                return Ok(false);
            }
            let destination = match source {
                Some(source) => {
                    let Some(projection) = index.source_edge_projection(access, source, budget)?
                    else {
                        return Ok(false);
                    };
                    let Some(target) = projection.relation.output_target else {
                        return Ok(false);
                    };
                    let target = forwarding::definition_node(optimized, nodes, target, budget)?;
                    if !projection.relation.control.executable || !walk.contains(target, budget)? {
                        return Ok(false);
                    }
                    projection.relation.output_incoming
                }
                None => {
                    index
                        .accesses
                        .get(access)
                        .ok_or(ArgumentResourceV1::Accounting)?
                        .pointer
                }
            };
            let Some(destination) = destination else {
                return Ok(false);
            };
            let destination = forwarding::definition_node(optimized, nodes, destination, budget)?;
            if !walk.contains(destination, budget)? {
                return Ok(false);
            }
            forwarding.path(
                edge.parameter,
                destination,
                choice.leaf,
                SelectedFinalForwardingStepV30::Incoming { actual: at },
                nodes,
                incoming,
                paths,
                budget,
            )
        }
    }
}

fn choices(
    optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
    index: &ProjectionIndexV30<'_>,
    access: usize,
    actual: &[ActualChoice],
    nodes: &[ActualNode],
    incoming: &[ActualIncoming],
    walk: &WalkV30,
    forwarding: &mut forwarding::ForwardingV30,
    output: &mut SelectedFinalRowsV30,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<()> {
    for (actual_ordinal, choice) in actual.iter().enumerate() {
        let pointer = node(nodes, choice.leaf, budget)?;
        let candidates = index.leaf_candidates(access, pointer.definition, budget)?;
        let fe2o3_kernel_ir::FormalGuardedPathV1::TrueEdge {
            source,
            ordinal,
            target,
        } = choice.domain.path()
        else {
            return resources::binding("selected final domain has no checked guard edge");
        };
        let function = match pointer.definition {
            Definition::FunctionArgument { function, .. } => function,
            Definition::BlockArgument { block, .. } => block.function,
            Definition::Result { operation, .. } => operation.block.function,
        };
        let guard_edge = Edge {
            source: block(optimized.checked.output(), function, source, budget)?,
            successor: u32::try_from(ordinal).map_err(|_| ArgumentResourceV1::Arithmetic)?,
        };
        if edge(optimized.checked.output(), guard_edge, budget)?.target_id != target {
            return resources::binding("selected final checked guard edge changed target");
        }
        let predicate = definition(
            optimized.checked.output(),
            function,
            choice.domain.predicate(),
            budget,
        )?;
        let first_choice = output.choices.len();
        for candidate in candidates {
            let (leaf, component) = index.candidate_leaf(candidate, budget)?;
            let original = index.leaf(access, leaf, budget)?;
            let pointer_component = match original.origin {
                PendingSourceSelectedLeafOriginV30::Issued(_) => 6,
                PendingSourceSelectedLeafOriginV30::Descriptor(_) => 4,
            };
            if component != pointer_component
                || !leaf_matches(optimized, index, access, leaf, choice, pointer, budget)?
            {
                continue;
            }
            let first = output.obligations.len();
            let first_path = output.forwarding.len();
            for &(_, _, ordinal) in index.leaf_obligations(access, leaf, budget)? {
                let (obligation, _) = index.obligation(access, ordinal, budget)?;
                let path = output.forwarding.len();
                if injection_matches(
                    optimized,
                    access,
                    obligation,
                    choice,
                    incoming,
                    nodes,
                    index,
                    walk,
                    forwarding,
                    &mut output.forwarding,
                    budget,
                )? {
                    let count = output
                        .forwarding
                        .len()
                        .checked_sub(path)
                        .ok_or(ArgumentResourceV1::Accounting)?;
                    push(
                        &mut output.obligations,
                        SelectedFinalObligationJoinV30 {
                            original: ordinal,
                            forwarding: RangeLocatorV30 { first: path, count },
                        },
                        budget,
                    )?;
                }
            }
            let count = output
                .obligations
                .len()
                .checked_sub(first)
                .ok_or(ArgumentResourceV1::Accounting)?;
            if count == 0 {
                continue;
            }
            let before_guards = output.choices.len();
            for guard in index.guard_projections(access, guard_edge, leaf, budget)? {
                let source = index.guard(access, guard.ordinal, budget)?;
                budget.charge_work(3)?;
                if source.leaf != leaf {
                    return resources::binding(
                        "selected final guard projection changed source leaf",
                    );
                }
                if !index.matches(
                    access,
                    SelectedDefinitionRoleV30::Guard(guard.ordinal),
                    predicate,
                    budget,
                )? {
                    continue;
                }
                if guard.selector.coordinate
                    != (UseCoordinate::TerminatorOperand {
                        block: guard_edge.source,
                        operand: 0,
                    })
                {
                    return resources::binding(
                        "selected final guard selector changed exact occurrence",
                    );
                }
                if !index.matches(
                    access,
                    SelectedDefinitionRoleV30::GuardSelector(guard.ordinal),
                    guard.selector.definition,
                    budget,
                )? {
                    return resources::binding(
                        "selected final guard selector lost source provenance",
                    );
                }
                push(
                    &mut output.choices,
                    SelectedFinalChoiceJoinV30 {
                        actual: actual_ordinal,
                        original_leaf: leaf,
                        original_guard: guard.ordinal,
                        obligations: RangeLocatorV30 { first, count },
                    },
                    budget,
                )?;
            }
            if before_guards == output.choices.len() {
                // This candidate did not match an original guard. Do not leave
                // orphan obligations in the canonical retained join.
                budget.charge_work(count)?;
                output.obligations.truncate(first);
                budget.charge_work(output.forwarding.len() - first_path)?;
                output.forwarding.truncate(first_path);
            }
        }
        if output.choices.len() == first_choice {
            return resources::binding(
                "selected final conditional choice lost its exact source leaf or guard",
            );
        }
    }
    Ok(())
}

pub(super) fn build(
    optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
    domains: &Domains<'_, '_>,
    function: FunctionCoordinate,
    index: &ProjectionIndexV30<'_>,
    sources: &[PendingSourceSelectedAccessV30],
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<SelectedFinalRowsV30> {
    let nodes = formal(optimized.original, domains.pointer_nodes(function, budget))?;
    let incoming = formal(optimized.original, domains.incoming_edges(function, budget))?;
    let mut walk = WalkV30::new(nodes.len(), budget)?;
    let mut forwarding =
        forwarding::ForwardingV30::build(optimized, index, nodes, incoming, budget)?;
    let mut rows = SelectedFinalRowsV30 {
        accesses: resources::vector(sources.len(), budget)?,
        choices: Vec::new(),
        edges: Vec::new(),
        obligations: Vec::new(),
        forwarding: Vec::new(),
    };
    for (ordinal, source) in sources.iter().enumerate() {
        budget.charge_work(6)?;
        let projection = index
            .accesses
            .get(ordinal)
            .ok_or(ArgumentResourceV1::Accounting)?;
        let first_choice = rows.choices.len();
        let first_edge = rows.edges.len();
        match projection.disposition {
            ProductionOptimizedSourceOperationV18::Retained { output, .. } => {
                if output.block.function != function {
                    return resources::binding("selected final access changed exact root");
                }
                let actual = formal(optimized.original, domains.access_at(output, budget))?.ok_or(
                    ProductionSourceOwnedViewErrorV18::Binding(
                        "selected final source access is outside the checked memory census",
                    ),
                )?;
                let actual_choices =
                    formal(optimized.original, domains.choices_at(output, budget))?.ok_or(
                        ProductionSourceOwnedViewErrorV18::Binding(
                            "selected final source access has no checked choices",
                        ),
                    )?;
                let pointer = definition(
                    optimized.checked.output(),
                    function,
                    actual.pointer(),
                    budget,
                )?;
                let value =
                    definition(optimized.checked.output(), function, actual.value(), budget)?;
                budget.charge_work(7)?;
                if projection.pointer != Some(pointer)
                    || projection.value != Some(value)
                    || actual.memory() != source.memory
                    || actual.writing() != source.writing
                    || actual_choices.is_empty()
                {
                    return resources::binding(
                        "selected final source and actual memory endpoints differ",
                    );
                }
                walk.edges(
                    ordinal,
                    actual.root_node(),
                    nodes,
                    incoming,
                    index,
                    &forwarding,
                    &mut rows.edges,
                    budget,
                )?;
                choices(
                    optimized,
                    index,
                    ordinal,
                    actual_choices,
                    nodes,
                    incoming,
                    &walk,
                    &mut forwarding,
                    &mut rows,
                    budget,
                )?;
            }
            ProductionOptimizedSourceOperationV18::Rewritten { .. }
            | ProductionOptimizedSourceOperationV18::RemovedUnreachable { .. } => {
                if projection.pointer.is_some() || projection.value.is_some() {
                    return resources::binding(
                        "selected final removed source effect kept an actual endpoint",
                    );
                }
            }
        }
        rows.accesses.push(SelectedFinalAccessV30 {
            original: ordinal,
            disposition: projection.disposition,
            choices: RangeLocatorV30 {
                first: first_choice,
                count: rows
                    .choices
                    .len()
                    .checked_sub(first_choice)
                    .ok_or(ArgumentResourceV1::Accounting)?,
            },
            edges: RangeLocatorV30 {
                first: first_edge,
                count: rows
                    .edges
                    .len()
                    .checked_sub(first_edge)
                    .ok_or(ArgumentResourceV1::Accounting)?,
            },
        });
    }
    Ok(rows)
}

pub(super) fn headers() -> Result<usize, ArgumentResourceV1> {
    fn h<T>() -> Result<usize, ArgumentResourceV1> {
        argument_sum_v1(&[
            size_of::<T>(),
            argument_product_v1(2, size_of::<SourceOwnedResultV18<T>>())?,
        ])
    }
    argument_sum_v1(&[
        h::<WalkV30>()?,
        argument_product_v1(2, h::<Vec<usize>>()?)?,
        h::<SelectedFinalRowsV30>()?,
        h::<SelectedFinalAccessV30>()?,
        h::<Vec<SelectedFinalAccessV30>>()?,
        h::<SelectedFinalChoiceJoinV30>()?,
        h::<Vec<SelectedFinalChoiceJoinV30>>()?,
        h::<SelectedFinalEdgeJoinV30>()?,
        h::<Vec<SelectedFinalEdgeJoinV30>>()?,
        h::<Vec<SelectedFinalObligationJoinV30>>()?,
        h::<SelectedFinalObligationJoinV30>()?,
        h::<Vec<SelectedFinalForwardingStepV30>>()?,
        h::<SelectedFinalForwardingStepV30>()?,
        h::<Option<SourceEdgeV30>>()?,
        h::<Option<&index::EdgeProjectionV30>>()?,
        h::<SelectedFinalEdgeOriginV30>()?,
        h::<ActualNode>()?,
        h::<&[ActualNode]>()?,
        h::<&[ActualIncoming]>()?,
        h::<&[ActualChoice]>()?,
        h::<&fe2o3_kernel_ir::CanonicalSelectedSliceAccessV30>()?,
        h::<Option<&fe2o3_kernel_ir::CanonicalSelectedSliceAccessV30>>()?,
        h::<Option<&[ActualChoice]>>()?,
        h::<Result<&[ActualNode], fe2o3_kernel_ir::CanonicalGuardedGlobalReadErrorV1>>()?,
        h::<Result<&[ActualIncoming], fe2o3_kernel_ir::CanonicalGuardedGlobalReadErrorV1>>()?,
        h::<Result<Option<&[ActualChoice]>, fe2o3_kernel_ir::CanonicalGuardedGlobalReadErrorV1>>()?,
        h::<
            Result<
                Option<&fe2o3_kernel_ir::CanonicalSelectedSliceAccessV30>,
                fe2o3_kernel_ir::CanonicalGuardedGlobalReadErrorV1,
            >,
        >()?,
        h::<Range<usize>>()?,
        h::<PendingSourceSelectedLeafV30>()?,
        h::<PendingSourceSelectedGuardV30>()?,
        h::<PendingSourceSelectedObligationV30>()?,
        argument_product_v1(6, h::<Definition>()?)?,
        h::<Edge>()?,
        h::<FunctionCoordinate>()?,
        h::<usize>()?,
        h::<bool>()?,
        h::<()>()?,
        growth_headers()?,
        forwarding::headers()?,
    ])
}

fn growth_headers() -> Result<usize, ArgumentResourceV1> {
    fn h<T>() -> Result<usize, ArgumentResourceV1> {
        argument_sum_v1(&[
            size_of::<Vec<T>>(),
            argument_product_v1(2, size_of::<Result<Vec<T>, ProductionSemanticKirErrorV1>>())?,
        ])
    }
    argument_sum_v1(&[
        h::<SelectedFinalAccessV30>()?,
        h::<SelectedFinalChoiceJoinV30>()?,
        h::<SelectedFinalEdgeJoinV30>()?,
        h::<SelectedFinalObligationJoinV30>()?,
        h::<SelectedFinalForwardingStepV30>()?,
        argument_product_v1(2, size_of::<Result<(), ProductionSemanticKirErrorV1>>())?,
    ])
}

#[cfg(test)]
mod growth_tests {
    use super::*;

    #[test]
    fn selected_final_row_growth_helper_frames_have_independent_typed_envelopes() {
        fn h<T>() -> usize {
            size_of::<Vec<T>>() + 2 * size_of::<Result<Vec<T>, ProductionSemanticKirErrorV1>>()
        }
        let expected = h::<SelectedFinalAccessV30>()
            + h::<SelectedFinalChoiceJoinV30>()
            + h::<SelectedFinalEdgeJoinV30>()
            + h::<SelectedFinalObligationJoinV30>()
            + h::<SelectedFinalForwardingStepV30>()
            + 2 * size_of::<Result<(), ProductionSemanticKirErrorV1>>();
        assert_eq!(growth_headers().unwrap(), expected);
        for limit in [expected, expected - 1] {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
            let mut budget = ArgumentBudgetV1::new(&mut work, limit);
            let result = budget.reserve_storage(growth_headers().unwrap());
            if limit == expected {
                result.unwrap();
                budget.release_storage(expected).unwrap();
            } else {
                assert!(matches!(result, Err(ArgumentResourceV1::Storage(error))
                    if error.actual() == expected && error.limit() == limit));
            }
            assert_eq!(budget.storage(), 0);
        }
    }

    #[test]
    fn selected_final_row_growth_pays_relocation_and_old_new_coexistence() {
        let value = [19usize; 17];
        let width = size_of::<[usize; 17]>();
        let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
        let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
        let mut rows = Vec::new();
        let mut capacity = 0;
        let mut peak = 0;
        let mut expected_work = 0;
        for count in 0..4096usize {
            expected_work += 2;
            if count == capacity {
                let next = if count == 0 { 4 } else { (3 * count + 1) / 2 };
                peak = peak.max((capacity + next) * width);
                capacity = next;
                expected_work += 3 + count;
            }
            push(&mut rows, value, &mut budget).unwrap();
            assert_eq!((rows.len(), rows.capacity()), (count + 1, capacity));
            assert_eq!(budget.work(), expected_work);
            assert_eq!(
                (budget.storage(), budget.peak_storage()),
                (capacity * width, peak)
            );
        }
        assert!(expected_work < 6 * rows.len());
        assert!(rows.iter().all(|row| *row == value));
        drop(rows);
        budget.release_storage(capacity * width).unwrap();
        assert_eq!(budget.storage(), 0);
    }

    #[test]
    fn selected_final_row_growth_keeps_exact_and_one_short_boundary_history() {
        let value = [23usize; 17];
        let width = size_of::<[usize; 17]>();
        for (work_limit, storage_limit, success) in [
            (9, 10 * width, true),
            (8, 10 * width, false),
            (9, 10 * width - 1, false),
        ] {
            let mut rows = vec![value; 4];
            let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
            let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
            budget.reserve_storage(4 * width).unwrap();
            let result = push(&mut rows, value, &mut budget);
            if success {
                result.unwrap();
                assert_eq!((rows.len(), rows.capacity()), (5, 6));
                assert_eq!(budget.work(), 9);
                assert_eq!(
                    (budget.storage(), budget.peak_storage()),
                    (6 * width, 10 * width)
                );
                assert_eq!(
                    (budget.failed_work(), budget.failed_storage()),
                    (None, None)
                );
            } else {
                assert_eq!((rows.len(), rows.capacity()), (4, 4));
                match result.unwrap_err() {
                    ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Work(
                        error,
                    )) => {
                        assert_eq!((error.actual(), error.limit()), (9, 8));
                        assert_eq!((budget.work(), budget.failed_work()), (5, Some(9)));
                        assert_eq!(
                            (budget.storage(), budget.peak_storage()),
                            (10 * width, 10 * width)
                        );
                        assert_eq!(budget.failed_storage(), None);
                    }
                    ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Storage(
                        error,
                    )) => {
                        assert_eq!(
                            (error.actual(), error.limit()),
                            (10 * width, 10 * width - 1)
                        );
                        assert_eq!((budget.work(), budget.failed_work()), (5, None));
                        assert_eq!(
                            (budget.storage(), budget.peak_storage()),
                            (4 * width, 4 * width)
                        );
                        assert_eq!(budget.failed_storage(), Some(10 * width));
                    }
                    error => panic!("unexpected selected row growth refusal: {error:?}"),
                }
            }
            assert!(rows.iter().all(|row| *row == value));
            let retained = budget.storage();
            drop(rows);
            budget.release_storage(retained).unwrap();
            assert_eq!(budget.storage(), 0);
        }
    }
}
