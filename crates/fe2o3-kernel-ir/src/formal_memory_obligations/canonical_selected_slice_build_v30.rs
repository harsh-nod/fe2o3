//! Shared per-function analysis, followed by exact selected leaf injection checks.
use super::*;
use crate::{IndexKind, IntrinsicKind};

impl SelectedFunctionV30 {
    fn empty(launch: ExplicitLaunchExtent) -> Self {
        Self {
            graph: PointerGraphV30::empty(),
            parameters: Vec::new(),
            accesses: Vec::new(),
            choices: Vec::new(),
            launch,
            counts: [0; 2],
        }
    }

    pub(super) fn retained_bytes(&self) -> std::result::Result<usize, ResourceError> {
        let mut bytes = self.graph.retained_bytes()?;
        for (count, width) in [
            (
                self.parameters.capacity(),
                size_of::<Option<CanonicalSelectedSliceParameterV30>>(),
            ),
            (
                self.accesses.capacity(),
                size_of::<CanonicalSelectedSliceAccessV30>(),
            ),
            (
                self.choices.capacity(),
                size_of::<CanonicalSelectedSliceChoiceV30>(),
            ),
        ] {
            bytes = bytes
                .checked_add(count.checked_mul(width).ok_or(ResourceError::Arithmetic)?)
                .ok_or(ResourceError::Arithmetic)?;
        }
        Ok(bytes)
    }
}

fn canonical_index_v30(index: ReadIndex) -> CanonicalGuardedReadIndexOriginV1 {
    match index {
        ReadIndex::ProvenOrigin(value) => CanonicalGuardedReadIndexOriginV1::ProvenOrigin(value),
        ReadIndex::ExactBlockParameter(value) => {
            CanonicalGuardedReadIndexOriginV1::ExactBlockParameter(value)
        }
    }
}

fn projection_v30<M: GuardMeter>(
    analysis: &mut GuardedAnalysisV1<'_, M>,
    index: ReadIndex,
) -> std::result::Result<Option<(Axis, ValueId)>, ResourceError> {
    analysis.ledger.charge(4)?;
    let ReadIndex::ProvenOrigin(value) = index else {
        return Ok(None);
    };
    let Some(operation) = analysis.definition(value)? else {
        return Ok(None);
    };
    let OperationKind::Intrinsic(intrinsic) = &operation.kind else {
        return Ok(None);
    };
    let IntrinsicKind::InvocationIndex {
        kind: IndexKind::Global,
        axis,
    } = intrinsic.kind
    else {
        return Ok(None);
    };
    if !matches!(operation.results.as_slice(), [result] if result.id == value && result.ty == Type::INDEX)
    {
        return Ok(None);
    }
    Ok(Some((axis, value)))
}

fn exact_axis_launch_v30(
    launch: ExplicitLaunchExtent,
    width: FormalIndexWidth,
    axis: Axis,
) -> bool {
    let ExplicitLaunchExtent::Exact { rank, extents } = launch else {
        return false;
    };
    let maximum = match width {
        FormalIndexWidth::Bits32 => 1_u64 << 32,
        FormalIndexWidth::Bits64 => u64::MAX,
        FormalIndexWidth::Unknown => return false,
    };
    let selected = match axis {
        Axis::X => 0,
        Axis::Y => 1,
        Axis::Z => 2,
    };
    (1..=3).contains(&rank)
        && extents.iter().all(|value| *value != 0)
        && extents[usize::from(rank)..].iter().all(|value| *value == 1)
        && extents[selected] <= maximum
        && extents
            .iter()
            .enumerate()
            .all(|(index, value)| index == selected || *value == 1)
}

fn add_choice<'g, M: GuardMeter>(
    function: &'g Function,
    graph: &PointerGraphV30,
    choices: &mut Vec<CanonicalSelectedSliceChoiceV30>,
    analysis: &mut GuardedAnalysisV1<'g, M>,
    access: &CanonicalSelectedSliceAccessV30,
    injection: CanonicalSelectedSliceInjectionV30,
    node: usize,
    source: BlockId,
) -> Result<bool> {
    analysis.ledger.charge(12)?;
    let Some(leaf) = graph.terminal.get(node).copied().flatten() else {
        return Ok(false);
    };
    let row = graph.nodes[leaf];
    if !matches!(row.step, CanonicalSelectedPointerStepV30::Formation { .. }) {
        return Ok(true);
    }
    if row.scalar != Some(access.scalar)
        || !matches!(row.space, AddressSpace::Global | AddressSpace::Generic)
        || !(row.access == AccessMode::ReadWrite
            || row.access
                == if access.writing {
                    AccessMode::WriteOnly
                } else {
                    AccessMode::ReadOnly
                })
    {
        return Ok(false);
    }
    let body = function.body.as_ref().ok_or(ResourceError::Accounting)?;
    let access_block = body.blocks[access.operation.block.block as usize].id;
    let memory = MemoryAccess {
        address_space: row.space,
        ..access.memory
    };
    let mut checked = None;
    let mut at_access = false;
    for (block, at) in [(access_block, true), (source, false)] {
        analysis.ledger.charge(2)?;
        let location = FunctionOperationLocation::new(block, 0);
        checked = if access.writing {
            analysis.runtime_slice_access_conditions_profile_v30::<true, true>(
                location,
                row.value,
                FormalMemoryAccessKind::Write,
                memory,
                None,
            )?
        } else {
            analysis.runtime_slice_access_conditions_profile_v30::<false, true>(
                location,
                row.value,
                FormalMemoryAccessKind::Read,
                memory,
                None,
            )?
        };
        if checked.is_some() {
            at_access = at;
            break;
        }
        if access_block == source {
            break;
        }
    }
    let Some(conditions) = checked else {
        return Ok(false);
    };
    let parameter = conditions.domain.allocation().parameter_index() as usize;
    let Some(Type::Slice(root)) = function.signature.parameters.get(parameter) else {
        return Ok(false);
    };
    if body.parameters.get(parameter) != Some(&conditions.domain.slice())
        || root.element.as_scalar() != Some(access.scalar)
        || !matches!(
            root.address_space,
            AddressSpace::Global | AddressSpace::Generic
        )
    {
        return Ok(false);
    }
    let projection = projection_v30(analysis, conditions.index_origin)?;
    analysis.ledger.push(
        choices,
        CanonicalSelectedSliceChoiceV30 {
            injection,
            leaf,
            domain: if access.writing {
                CanonicalSelectedSliceDomainV30::Store(conditions.domain)
            } else {
                CanonicalSelectedSliceDomainV30::Read(conditions.domain)
            },
            root_space: root.address_space,
            root_access: root.access,
            index_origin: canonical_index_v30(conditions.index_origin),
            length_origin: conditions.length_origin,
            invocation_projection: projection,
            at_access,
        },
    )?;
    Ok(true)
}

fn check_access<'g, M: GuardMeter>(
    function: &'g Function,
    graph: &PointerGraphV30,
    choices: &mut Vec<CanonicalSelectedSliceChoiceV30>,
    analysis: &mut GuardedAnalysisV1<'g, M>,
    access: &mut CanonicalSelectedSliceAccessV30,
    marks: &mut [usize],
    queue: &mut Vec<usize>,
    generation: usize,
) -> Result<bool> {
    let body = function.body.as_ref().ok_or(ResourceError::Accounting)?;
    let block = body.blocks[access.operation.block.block as usize].id;
    let first = choices.len();
    if !add_choice(
        function,
        graph,
        choices,
        analysis,
        access,
        CanonicalSelectedSliceInjectionV30::Access,
        access.node,
        block,
    )? {
        return Ok(false);
    }
    queue.clear();
    marks[access.node] = generation;
    queue.push(access.node);
    let mut next = 0;
    while let Some(&node) = queue.get(next) {
        analysis.ledger.charge(8)?;
        next += 1;
        if !graph.seeded[node] {
            return Ok(false);
        }
        let row = graph.nodes[node];
        if row.scalar != Some(access.scalar)
            || !matches!(row.space, AddressSpace::Global | AddressSpace::Generic)
            || !(row.access == AccessMode::ReadWrite
                || row.access
                    == if access.writing {
                        AccessMode::WriteOnly
                    } else {
                        AccessMode::ReadOnly
                    })
        {
            return Ok(false);
        }
        match row.step {
            CanonicalSelectedPointerStepV30::Unsupported => return Ok(false),
            CanonicalSelectedPointerStepV30::Formation { .. } => (),
            CanonicalSelectedPointerStepV30::Cast { input } => {
                if marks[input] != generation {
                    marks[input] = generation;
                    queue.push(input);
                }
            }
            CanonicalSelectedPointerStepV30::Parameter { first, count } => {
                let mut reachable = 0usize;
                for ordinal in first..first.checked_add(count).ok_or(ResourceError::Arithmetic)? {
                    analysis.ledger.charge(4)?;
                    let edge = graph.incoming[ordinal];
                    if !edge.reachable {
                        continue;
                    }
                    reachable = reachable.checked_add(1).ok_or(ResourceError::Arithmetic)?;
                    if !add_choice(
                        function,
                        graph,
                        choices,
                        analysis,
                        access,
                        CanonicalSelectedSliceInjectionV30::Incoming(ordinal),
                        edge.argument,
                        edge.source,
                    )? {
                        return Ok(false);
                    }
                    if marks[edge.argument] != generation {
                        marks[edge.argument] = generation;
                        queue.push(edge.argument);
                    }
                }
                if reachable == 0 {
                    return Ok(false);
                }
            }
        }
    }
    if choices.len() == first {
        return Ok(false);
    }
    access.choices = first..choices.len();
    Ok(true)
}

fn add_parameter_choices<M: GuardMeter>(
    result: &mut SelectedFunctionV30,
    access: &CanonicalSelectedSliceAccessV30,
    width: FormalIndexWidth,
    meter: &mut M,
) -> Result<bool> {
    for choice in &result.choices[access.choices.clone()] {
        meter.charge(20)?;
        let ordinal = choice.domain.allocation().parameter_index() as usize;
        let Some(Some(parameter)) = result.parameters.get_mut(ordinal) else {
            return Ok(false);
        };
        if parameter.value != choice.domain.slice()
            || parameter.scalar != access.scalar
            || parameter.space != choice.root_space
            || parameter.access != choice.root_access
        {
            return Ok(false);
        }
        let axis = choice.invocation_projection.map(|(axis, _)| axis);
        if parameter.reads == 0 && parameter.writes == 0 {
            parameter.axis = axis;
        }
        parameter.different_projection |= axis.is_none() || parameter.axis != axis;
        if access.writing {
            if !matches!(
                parameter.access,
                AccessMode::WriteOnly | AccessMode::ReadWrite
            ) || parameter.different_projection
                || axis.is_none_or(|axis| !exact_axis_launch_v30(result.launch, width, axis))
            {
                return Ok(false);
            }
            parameter.writes = parameter
                .writes
                .checked_add(1)
                .ok_or(ResourceError::Arithmetic)?;
        } else {
            if !matches!(
                parameter.access,
                AccessMode::ReadOnly | AccessMode::ReadWrite
            ) || parameter.writes != 0 && parameter.different_projection
            {
                return Ok(false);
            }
            parameter.reads = parameter
                .reads
                .checked_add(1)
                .ok_or(ResourceError::Arithmetic)?;
        }
    }
    Ok(true)
}

pub(super) fn function<'g>(
    function: &'g Function,
    coordinate: FunctionCoordinate,
    launch: ExplicitLaunchExtent,
    width: FormalIndexWidth,
    limits: CanonicalGuardedGlobalReadLimitsV1,
    budget: &mut Budget<'_>,
) -> Result<Option<SelectedFunctionV30>> {
    let floor = budget.storage();
    let mut result = SelectedFunctionV30::empty(launch);
    let Some(body) = function.body.as_ref() else {
        return Ok(Some(result));
    };
    let flow = crate::control_flow::analyze_control_flow_with_verification_budget_v1(
        function,
        limits.control_flow,
        budget,
    )
    .map_err(|error| match error {
        crate::control_flow::MeteredControlFlowErrorV1::ControlFlow(error) => {
            Failure::ControlFlow(error)
        }
        crate::control_flow::MeteredControlFlowErrorV1::Resource(error) => error.into(),
    })?;
    let mut complete = false;
    {
        let mut meter = LiveGuardMeter::new(
            budget,
            limits.per_function_work,
            limits.per_function_new_bytes,
            limits.per_function_records,
        );
        meter.storage(build_headers_v30()?)?;
        let seed = GuardedControlV1::collect_preserving_ledger_v24::<true>(
            function,
            flow.indexed_v15(),
            meter,
        )?;
        let seed = match seed {
            GuardedControlCollectionV1::Unselected(meter) => {
                GuardedControlV1::collect_preserving_ledger_v24::<false>(
                    function,
                    flow.indexed_v15(),
                    meter,
                )?
            }
            selected => selected,
        };
        if let GuardedControlCollectionV1::Selected(seed) = seed {
            let entry = seed.entry;
            let mut analysis = GuardedAnalysisV1::empty(seed, false);
            collect_actual_definitions(&mut analysis, function)?;
            collect_actual_origins(&mut analysis, function, flow.indexed_v15())?;
            analysis.collect_parameters_and_carried_truths(function, entry)?;
            analysis.collect_runtime_access_guards_profile_v30::<true, true>(function)?;
            result.graph =
                PointerGraphV30::build(function, coordinate, flow.indexed_v15(), &mut analysis)?;
            analysis
                .ledger
                .reserve(&mut result.parameters, function.signature.parameters.len())?;
            for (ordinal, ty) in function.signature.parameters.iter().enumerate() {
                analysis.ledger.charge(6)?;
                let row = match ty {
                    Type::Slice(slice)
                        if matches!(
                            slice.address_space,
                            AddressSpace::Global | AddressSpace::Generic
                        ) =>
                    {
                        slice
                            .element
                            .as_scalar()
                            .map(|scalar| CanonicalSelectedSliceParameterV30 {
                                value: body.parameters[ordinal],
                                scalar,
                                space: slice.address_space,
                                access: slice.access,
                                reads: 0,
                                writes: 0,
                                axis: None,
                                different_projection: false,
                            })
                    }
                    _ => None,
                };
                result.parameters.push(row);
            }
            let mut marks = Vec::new();
            let mut queue = Vec::new();
            analysis
                .ledger
                .reserve(&mut marks, result.graph.nodes.len())?;
            analysis
                .ledger
                .reserve(&mut queue, result.graph.nodes.len())?;
            analysis.ledger.charge(result.graph.nodes.len())?;
            marks.resize(result.graph.nodes.len(), 0usize);
            let mut census = FunctionFacts {
                function,
                controls: Vec::new(),
                predicates: Vec::new(),
                reads: Vec::new(),
                global_read_occurrences: 0,
                other_global_effects: 0,
                unresolved_calls: 0,
            };
            complete = true;
            'blocks: for (block, contents) in body.blocks.iter().enumerate() {
                for (operation, actual) in contents.operations.iter().enumerate() {
                    analysis.ledger.charge(8)?;
                    let generic_may_be_external = match actual.kind {
                        OperationKind::Load { pointer, access }
                        | OperationKind::Store {
                            pointer, access, ..
                        } if access.address_space == AddressSpace::Generic => analysis
                            .proven_pointer_space_v18(pointer)?
                            .is_none_or(|space| {
                                matches!(space, AddressSpace::Global | AddressSpace::Generic)
                            }),
                        _ => true,
                    };
                    effect_counts::<false, _>(
                        &mut census,
                        actual,
                        generic_may_be_external,
                        &mut analysis.ledger,
                    )?;
                    let candidate = match actual.kind {
                        OperationKind::Load { pointer, access }
                            if matches!(
                                access.address_space,
                                AddressSpace::Global | AddressSpace::Generic
                            ) && generic_may_be_external =>
                        {
                            let [value] = actual.results.as_slice() else {
                                complete = false;
                                break 'blocks;
                            };
                            Some((pointer, value.id, value.ty.as_scalar(), access, false))
                        }
                        OperationKind::Store {
                            pointer,
                            value,
                            access,
                        } if matches!(
                            access.address_space,
                            AddressSpace::Global | AddressSpace::Generic
                        ) && generic_may_be_external =>
                        {
                            if !actual.results.is_empty() {
                                complete = false;
                                break 'blocks;
                            }
                            Some((
                                pointer,
                                value,
                                analysis.runtime_type(value)?.and_then(Type::as_scalar),
                                access,
                                true,
                            ))
                        }
                        _ => None,
                    };
                    let Some((pointer, value, scalar, memory, writing)) = candidate else {
                        continue;
                    };
                    let (Some(scalar), Some(node)) =
                        (scalar, result.graph.find(pointer, &mut analysis.ledger)?)
                    else {
                        complete = false;
                        break 'blocks;
                    };
                    if memory.volatile
                        || result.graph.nodes[node].space != memory.address_space
                        || scalar
                            .bit_width()
                            .is_none_or(|bits| !matches!(bits, 8 | 16 | 32 | 64))
                    {
                        complete = false;
                        break 'blocks;
                    }
                    let mut access = CanonicalSelectedSliceAccessV30 {
                        operation: operation_coordinate(coordinate, block, operation)?,
                        pointer,
                        value,
                        scalar,
                        memory,
                        writing,
                        node,
                        choices: 0..0,
                    };
                    let generation = result
                        .accesses
                        .len()
                        .checked_add(1)
                        .ok_or(ResourceError::Arithmetic)?;
                    if !check_access(
                        function,
                        &result.graph,
                        &mut result.choices,
                        &mut analysis,
                        &mut access,
                        &mut marks,
                        &mut queue,
                        generation,
                    )? || !add_parameter_choices(
                        &mut result,
                        &access,
                        width,
                        &mut analysis.ledger,
                    )? {
                        complete = false;
                        break 'blocks;
                    }
                    result.counts[usize::from(writing)] = result.counts[usize::from(writing)]
                        .checked_add(1)
                        .ok_or(ResourceError::Arithmetic)?;
                    analysis.ledger.push(&mut result.accesses, access)?;
                }
            }
            complete &= census.global_read_occurrences == result.counts[0]
                && census.other_global_effects == result.counts[1]
                && census.unresolved_calls == 0;
        } else if let GuardedControlCollectionV1::Unselected(mut meter) = seed {
            let mut census = FunctionFacts {
                function,
                controls: Vec::new(),
                predicates: Vec::new(),
                reads: Vec::new(),
                global_read_occurrences: 0,
                other_global_effects: 0,
                unresolved_calls: 0,
            };
            meter.reserve(&mut result.parameters, function.signature.parameters.len())?;
            for _ in &function.signature.parameters {
                meter.charge(1)?;
                result.parameters.push(None);
            }
            for block in &body.blocks {
                meter.charge(1)?;
                for operation in &block.operations {
                    effect_counts::<false, _>(&mut census, operation, true, &mut meter)?;
                }
            }
            complete = census.global_read_occurrences == 0
                && census.other_global_effects == 0
                && census.unresolved_calls == 0;
        }
    }
    flow.release(budget)?;
    if !complete {
        drop(result);
        let owned = budget
            .storage()
            .checked_sub(floor)
            .ok_or(ResourceError::Accounting)?;
        budget.release_storage(owned)?;
        return Ok(None);
    }
    let retained = result.retained_bytes()?;
    let excess = budget
        .storage()
        .checked_sub(floor)
        .and_then(|bytes| bytes.checked_sub(retained))
        .ok_or(ResourceError::Accounting)?;
    budget.release_storage(excess)?;
    Ok(Some(result))
}

fn build_headers_v30() -> std::result::Result<usize, ResourceError> {
    type Frames<'a> = (
        SelectedFunctionV30,
        CanonicalSelectedSliceAccessV30,
        CanonicalSelectedSliceChoiceV30,
        Option<CanonicalSelectedSliceParameterV30>,
        FunctionFacts<'a>,
        GuardedControlCollectionV1<LiveGuardMeter<'a, 'a>>,
        Vec<usize>,
        Vec<usize>,
        RuntimeSliceReadConditionsV1,
        Option<RuntimeSliceReadConditionsV1>,
        MemoryAccess,
        &'a Function,
        &'a PointerGraphV30,
        &'a mut Vec<CanonicalSelectedSliceChoiceV30>,
        &'a mut CanonicalSelectedSliceAccessV30,
        &'a mut [usize],
        &'a mut Vec<usize>,
        CanonicalSelectedSliceInjectionV30,
        CanonicalSelectedPointerNodeV30,
        CanonicalSelectedPointerIncomingV30,
        [usize; 16],
    );
    size_of::<Frames<'_>>()
        .checked_add(2 * size_of::<Result<Option<SelectedFunctionV30>>>())
        .and_then(|bytes| bytes.checked_add(4 * size_of::<Result<bool>>()))
        .and_then(|bytes| {
            bytes.checked_add(2 * size_of::<Result<CanonicalSelectedSliceChoiceV30>>())
        })
        .ok_or(ResourceError::Arithmetic)
}
