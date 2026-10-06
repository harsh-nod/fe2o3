// Movement-only proof for an exact source-issued lane query. This does not
// discharge source capabilities, final execution, launch or convergence rules.
#[derive(Clone, Copy, Debug, Default)]
struct ScopedLaneChainV29 {
    seed: Option<usize>,
    first: Option<usize>,
    last: Option<usize>,
    count: usize,
}

#[derive(Clone, Copy, Debug)]
struct ScopedLaneSourceV29 {
    instance: ProductionCallInstanceIdV1,
    source: SemanticKirTerminatorOperationSpanV1,
    span: usize,
    value: ValueId,
    next: Option<usize>,
}

#[cfg_attr(test, derive(Clone))]
struct ScopedLaneQueryTransportV29 {
    source_plan: usize,
    map: usize,
    ledger: ArgumentLedgerV1,
    slot: usize,
    floor: usize,
    chains: Vec<ScopedLaneChainV29>,
    rows: Vec<ScopedLaneSourceV29>,
}

struct ScopedLaneCalleeSourceV29<'a, 'p, 's> {
    transport: &'a ScopedLaneQueryTransportV29,
    map: &'a ProductionInstanceCorrespondenceV1<'p, 's>,
    child: ProductionCallInstanceIdV1,
}

struct ScopedLaneCalleeV29<'a> {
    function: &'a Function,
    ledger: ArgumentLedgerV1,
    slot: usize,
    floor: usize,
}

struct ScopedLaneActualV29<'a> {
    point: (BlockId, u32),
    operation: &'a Operation,
    seen: bool,
}

#[cfg(test)]
type ScopedLaneObserverV29 = fn(
    &SourceReferencePlanV29<'_, '_>,
    &mut ProductionInstanceCorrespondenceV1<'_, '_>,
    &mut [Option<LoweredFunctionResultV1>],
    Option<&ScopedLaneQueryTransportV29>,
    &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1>;

#[cfg(test)]
thread_local! {
    static SCOPED_LANE_OBSERVER_V29: std::cell::Cell<Option<ScopedLaneObserverV29>> = const { std::cell::Cell::new(None) };
}

fn scoped_lane_source_error_v29(
    error: ProductionSemanticKirErrorV1,
) -> CallInstanceEmissionErrorV1 {
    match error {
        ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(error) => error.into(),
        _ => CallInstanceEmissionErrorV1::CalleeCollective,
    }
}

fn scoped_lane_map_error_v29(error: InstanceCorrespondenceErrorV1) -> CallInstanceEmissionErrorV1 {
    match error {
        InstanceCorrespondenceErrorV1::Resource(error) => error.into(),
        _ => CallInstanceEmissionErrorV1::CalleeCollective,
    }
}

// Only the two closed constructor blocks below use this unit scratch scope.
// No client callback, returned allocation or mutable source owner enters it.
fn with_scoped_lane_scratch_v29<'w>(
    budget: &mut ArgumentBudgetV1<'w>,
    body: impl FnOnce(&mut ArgumentBudgetV1<'w>) -> Result<(), ProductionSemanticKirErrorV1>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    with_canonical_call_scratch_v1(budget, |budget| {
        budget.reserve_storage(argument_sum_v1(&[
            std::mem::size_of_val(&body),
            2 * std::mem::size_of::<Result<(), ProductionSemanticKirErrorV1>>(),
            std::mem::size_of::<std::thread::Result<Result<(), ProductionSemanticKirErrorV1>>>(),
            std::mem::size_of::<ArgumentLedgerV1>(),
            2 * std::mem::size_of::<usize>(),
        ])?)?;
        body(budget)
    })
}

fn scoped_lane_actual_v29<'a>(
    function: &'a Function,
    budget: &mut ArgumentBudgetV1<'_>,
    scratch: &mut usize,
) -> Result<Vec<ScopedLaneActualV29<'a>>, CallInstanceEmissionErrorV1> {
    use CallInstanceEmissionErrorV1::CalleeCollective as Refused;
    let body = function.body.as_ref().ok_or(Refused)?;
    let mut count = 0usize;
    budget.charge_work(body.blocks.len())?;
    for block in &body.blocks {
        budget.charge_work(block.operations.len())?;
        for operation in &block.operations {
            if matches!(operation.kind, OperationKind::Wave(_)) {
                count = count.checked_add(1).ok_or_else(call_splice_arithmetic_v1)?;
            }
        }
    }
    call_splice_charge_storage_v1(
        argument_sum_v1(&[
            std::mem::size_of::<Vec<ScopedLaneActualV29<'_>>>(),
            2 * std::mem::size_of::<
                Result<Vec<ScopedLaneActualV29<'_>>, CallInstanceEmissionErrorV1>,
            >(),
        ])?,
        budget,
        scratch,
    )?;
    let mut actual = call_splice_vec_v1(count, budget, scratch)?;
    budget.charge_work(body.blocks.len())?;
    for block in &body.blocks {
        budget.charge_work(block.operations.len())?;
        for (position, operation) in block.operations.iter().enumerate() {
            if matches!(operation.kind, OperationKind::Wave(_)) {
                actual.push(ScopedLaneActualV29 {
                    point: (
                        block.id,
                        u32::try_from(position).map_err(|_| call_splice_arithmetic_v1())?,
                    ),
                    operation,
                    seen: false,
                });
            }
        }
    }
    call_splice_sort_work_v1(actual.len(), budget)?;
    actual.sort_unstable_by_key(|row| row.point);
    budget.charge_work(actual.len())?;
    if actual.windows(2).any(|pair| pair[0].point == pair[1].point) {
        return Err(Refused);
    }
    Ok(actual)
}

fn check_scoped_lane_operation_v29(
    operation: &Operation,
    value: ValueId,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), CallInstanceEmissionErrorV1> {
    budget.charge_work(5)?;
    if operation.kind
        != OperationKind::Wave(WaveOperation::full(
            WaveOperationKind::LaneId,
            WaveWidth::Wave64,
        ))
        || !matches!(operation.results.as_slice(), [result]
            if result.id == value && result.ty == Type::Scalar(ScalarType::U32))
    {
        return Err(CallInstanceEmissionErrorV1::CalleeCollective);
    }
    Ok(())
}

impl ScopedLaneQueryTransportV29 {
    fn new(
        references: &SourceReferencePlanV29<'_, '_>,
        map: &ProductionInstanceCorrespondenceV1<'_, '_>,
        emitted: &[Option<LoweredFunctionResultV1>],
        budget: &mut ArgumentBudgetV1<'_>,
        scratch: &mut usize,
    ) -> Result<Option<Self>, CallInstanceEmissionErrorV1> {
        use CallInstanceEmissionErrorV1::CalleeCollective as Refused;
        references
            .check_owner(map.plan, budget)
            .map_err(scoped_lane_source_error_v29)?;
        map.check_live_ledger_v1(budget)
            .map_err(scoped_lane_map_error_v29)?;
        budget.charge_work(2)?;
        if emitted.len() != map.plan.instances().len() {
            return Err(Refused);
        }
        let root = map.plan.root();
        let mut count = 0usize;
        // Count original calls, never candidate operations. The zero-query
        // path returns no permit; unknown actual Waves remain globally refused.
        budget.charge_work(emitted.len())?;
        for index in 0..emitted.len() {
            let instance = map.plan.id_at(index).ok_or(Refused)?;
            if instance == root || map.plan.instance_reachable(instance) == Some(false) {
                continue;
            }
            let calls = map.plan.calls(instance).ok_or(Refused)?;
            budget.charge_work(calls.len())?;
            for call in calls {
                if matches!(
                    call.callable(),
                    SemanticCallableDeclV1::CompilerIntrinsic {
                        operation: SemanticCompilerIntrinsicOperationV1::WaveLaneCurrent { .. },
                        ..
                    }
                ) {
                    budget.charge_work(2)?;
                    match map.plan.call_control(call.occurrence()) {
                        Some(ProductionCallControlV1::Unreachable) => continue,
                        Some(ProductionCallControlV1::MayReturn) if call.child().is_none() => {}
                        _ => return Err(Refused),
                    }
                    count = count.checked_add(1).ok_or_else(call_splice_arithmetic_v1)?;
                }
            }
        }
        call_splice_charge_storage_v1(
            argument_sum_v1(&[
                std::mem::size_of::<Option<Self>>(),
                2 * std::mem::size_of::<Result<Option<Self>, CallInstanceEmissionErrorV1>>(),
            ])?,
            budget,
            scratch,
        )?;
        if count == 0 {
            return Ok(None);
        }
        let mut result = Self {
            source_plan: std::ptr::from_ref(map.plan) as usize,
            map: std::ptr::from_ref(map) as usize,
            ledger: budget.work_ledger_identity_v1(),
            slot: std::ptr::from_ref(&*budget) as usize,
            floor: 0,
            chains: call_splice_vec_v1(emitted.len(), budget, scratch)?,
            rows: call_splice_vec_v1(count, budget, scratch)?,
        };
        budget.charge_work(emitted.len())?;
        result
            .chains
            .resize(emitted.len(), ScopedLaneChainV29::default());
        budget.charge_work(map.seeds.rows.len())?;
        for (ordinal, seed) in map.seeds.rows.iter().enumerate() {
            let chain = result
                .chains
                .get_mut(seed.instance.index())
                .ok_or(Refused)?;
            if chain.seed.replace(ordinal).is_some() || seed.container != seed.instance {
                return Err(Refused);
            }
        }
        with_scoped_lane_scratch_v29(budget, |budget| {
            let mut temporary = 0;
            type Span = ((usize, SemanticBlockIdV1), usize);
            call_splice_charge_storage_v1(
                argument_sum_v1(&[
                    std::mem::size_of::<Vec<Span>>(),
                    2 * std::mem::size_of::<Result<Vec<Span>, CallInstanceEmissionErrorV1>>(),
                ])?,
                budget,
                &mut temporary,
            )
            .map_err(source_address_call_error_v29)?;
            budget.charge_work(map.spans.rows.len())?;
            let span_count = map
                .spans
                .rows
                .iter()
                .filter(|row| matches!(row.source, InstanceSpanSourceV1::Terminator(_)))
                .count();
            let mut spans = call_splice_vec_v1(span_count, budget, &mut temporary)
                .map_err(source_address_call_error_v29)?;
            budget.charge_work(map.spans.rows.len())?;
            for (ordinal, mapped) in map.spans.rows.iter().enumerate() {
                if let InstanceSpanSourceV1::Terminator(source) = mapped.source {
                    spans.push(((mapped.instance.index(), source.semantic_block), ordinal));
                }
            }
            call_splice_sort_work_v1(spans.len(), budget).map_err(source_address_call_error_v29)?;
            spans.sort_unstable_by_key(|row| row.0);
            budget.charge_work(spans.len())?;
            if spans.windows(2).any(|pair| pair[0].0 == pair[1].0) {
                return Err(source_address_call_error_v29(Refused));
            }
            for (index, lowered) in emitted.iter().enumerate() {
                budget.charge_work(2)?;
                let instance = map
                    .plan
                    .id_at(index)
                    .ok_or_else(execution_archive_error_v29)?;
                if instance == root {
                    continue;
                }
                if map.plan.instance_reachable(instance) == Some(false) {
                    if lowered.is_some() {
                        return Err(execution_archive_error_v29());
                    }
                    continue;
                }
                let lowered = lowered.as_ref().ok_or_else(execution_archive_error_v29)?;
                if lowered.source_call_instance != Some(instance) {
                    return Err(execution_archive_error_v29());
                }
                let original = map
                    .plan
                    .instance(instance)
                    .ok_or_else(execution_archive_error_v29)?;
                let occurrences = map
                    .plan
                    .occurrences(instance)
                    .ok_or_else(execution_archive_error_v29)?;
                let archive = lowered
                    .execution_observation
                    .as_ref()
                    .ok_or_else(execution_archive_error_v29)?;
                archive.check_original_v29(map.plan, instance, budget)?;
                with_scoped_lane_scratch_v29(budget, |budget| {
                    let mut local_scratch = 0;
                    let mut actual =
                        scoped_lane_actual_v29(&lowered.function, budget, &mut local_scratch)
                            .map_err(source_address_call_error_v29)?;
                    let first = result.rows.len();
                    budget.charge_work(occurrences.edge_definitions().len())?;
                    for edge in occurrences.edge_definitions() {
                        let block = SemanticBlockIdV1::from_index(edge.edge().source().get());
                        let Some(SemanticTerminatorKindV1::Call(call)) = original
                            .declaration()
                            .blocks()
                            .get(block.index() as usize)
                            .map(|block| block.terminator().kind())
                        else {
                            continue;
                        };
                        let Some(SemanticCallableDeclV1::CompilerIntrinsic {
                            operation:
                                SemanticCompilerIntrinsicOperationV1::WaveLaneCurrent {
                                    lane,
                                    wave_width,
                                },
                            ..
                        }) = map
                            .plan
                            .owner()
                            .source_semantic()
                            .callables()
                            .get(call.callee().index() as usize)
                        else {
                            continue;
                        };
                        budget.charge_work(12)?;
                        match map.plan.call_control(ProductionCallOccurrenceV1 {
                            caller: instance,
                            block,
                        }) {
                            Some(ProductionCallControlV1::Unreachable) => continue,
                            Some(ProductionCallControlV1::MayReturn) => {}
                            _ => return Err(execution_archive_error_v29()),
                        }
                        let destination =
                            call.destination().ok_or_else(execution_archive_error_v29)?;
                        if !edge.is_reachable()
                            || !edge.is_promoted()
                            || edge.edge().ordinal() != 0
                            || edge.ordinal() != 0
                            || *wave_width != 64
                            || !call.arguments().is_empty()
                            || !destination.place().projections().is_empty()
                            || destination.place().ty() != *lane
                            || destination.place().local().index() != edge.variable().get()
                            || result.rows.len() >= count
                        {
                            return Err(execution_archive_error_v29());
                        }
                        let value = edge.value().ok_or_else(execution_archive_error_v29)?;
                        let SemanticValueBindingV1::WaveLane { value, wave } =
                            archive.lookup_original_v29(map.plan, instance, value, budget)?
                        else {
                            return Err(execution_archive_error_v29());
                        };
                        if *wave != SemanticCurrentWaveV1::new(64) {
                            return Err(execution_archive_error_v29());
                        }
                        budget.charge_work(call_splice_search_work_v1(spans.len()))?;
                        let span = spans
                            .binary_search_by_key(&(index, block), |row| row.0)
                            .ok()
                            .map(|position| spans[position].1)
                            .ok_or_else(execution_archive_error_v29)?;
                        let mapped = &map.spans.rows[span];
                        let InstanceSpanSourceV1::Terminator(source) = mapped.source else {
                            return Err(execution_archive_error_v29());
                        };
                        let source_point = mapped.source.coordinates().2;
                        if mapped.instance != instance
                            || source.semantic_function != original.function()
                            || source_point.count != 1
                            || mapped.removed_call.is_some()
                        {
                            return Err(execution_archive_error_v29());
                        }
                        budget.charge_work(call_splice_search_work_v1(actual.len()))?;
                        let position = actual
                            .binary_search_by_key(
                                &(source_point.block, source_point.first),
                                |row| row.point,
                            )
                            .map_err(|_| execution_archive_error_v29())?;
                        let actual = &mut actual[position];
                        if actual.seen {
                            return Err(execution_archive_error_v29());
                        }
                        check_scoped_lane_operation_v29(actual.operation, *value, budget)
                            .map_err(source_address_call_error_v29)?;
                        actual.seen = true;
                        result.rows.push(ScopedLaneSourceV29 {
                            instance,
                            source,
                            span,
                            value: *value,
                            next: None,
                        });
                    }
                    budget.charge_work(actual.len())?;
                    if actual.iter().any(|row| !row.seen) {
                        return Err(execution_archive_error_v29());
                    }
                    let end = result.rows.len();
                    budget.charge_work(end - first)?;
                    for position in first..end.saturating_sub(1) {
                        result.rows[position].next = Some(position + 1);
                    }
                    result.chains[index] = ScopedLaneChainV29 {
                        seed: result.chains[index].seed,
                        first: (first != end).then_some(first),
                        last: end.checked_sub(1).filter(|_| first != end),
                        count: end - first,
                    };
                    Ok(())
                })?;
            }
            if result.rows.len() != count {
                return Err(execution_archive_error_v29());
            }
            Ok(())
        })
        .map_err(scoped_lane_source_error_v29)?;
        result.floor = budget.storage();
        Ok(Some(result))
    }

    fn check(
        &self,
        map: &ProductionInstanceCorrespondenceV1<'_, '_>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), CallInstanceEmissionErrorV1> {
        if self.ledger != budget.work_ledger_identity_v1()
            || self.slot != std::ptr::from_ref(&*budget) as usize
            || budget.storage() < self.floor
        {
            return Err(ArgumentResourceV1::Accounting.into());
        }
        map.check_live_ledger_v1(budget)
            .map_err(scoped_lane_map_error_v29)?;
        budget.charge_work(1)?;
        if self.source_plan != std::ptr::from_ref(map.plan) as usize
            || self.map != std::ptr::from_ref(map) as usize
        {
            return Err(CallInstanceEmissionErrorV1::CalleeCollective);
        }
        Ok(())
    }

    fn join(
        &mut self,
        caller: ProductionCallInstanceIdV1,
        child: ProductionCallInstanceIdV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), CallInstanceEmissionErrorV1> {
        use CallInstanceEmissionErrorV1::CalleeCollective as Refused;
        if self.ledger != budget.work_ledger_identity_v1()
            || self.slot != std::ptr::from_ref(&*budget) as usize
            || budget.storage() < self.floor
        {
            return Err(ArgumentResourceV1::Accounting.into());
        }
        budget.charge_work(6)?;
        if caller == child {
            return Err(Refused);
        }
        let left = *self.chains.get(caller.index()).ok_or(Refused)?;
        let right = *self.chains.get(child.index()).ok_or(Refused)?;
        let count = left
            .count
            .checked_add(right.count)
            .ok_or_else(call_splice_arithmetic_v1)?;
        if let Some(last) = left.last {
            let tail = self.rows.get_mut(last).ok_or(Refused)?;
            if tail.next.is_some() {
                return Err(Refused);
            }
            tail.next = right.first;
        }
        self.chains[caller.index()] = ScopedLaneChainV29 {
            seed: left.seed,
            first: left.first.or(right.first),
            last: right.last.or(left.last),
            count,
        };
        self.chains[child.index()] = ScopedLaneChainV29::default();
        Ok(())
    }
}

impl ScopedLaneCalleeSourceV29<'_, '_, '_> {
    fn prepare<'a>(
        &self,
        function: &'a Function,
        budget: &mut ArgumentBudgetV1<'_>,
        scratch: &mut usize,
    ) -> Result<ScopedLaneCalleeV29<'a>, CallInstanceEmissionErrorV1> {
        use CallInstanceEmissionErrorV1::CalleeCollective as Refused;
        self.transport.check(self.map, budget)?;
        budget.charge_work(2)?;
        let chain = *self
            .transport
            .chains
            .get(self.child.index())
            .ok_or(Refused)?;
        let seed = self
            .map
            .seeds
            .rows
            .get(chain.seed.ok_or(Refused)?)
            .ok_or(Refused)?;
        budget.charge_work(function.id.as_str().len())?;
        if seed.instance != self.child
            || seed.container != self.child
            || seed.function_name != function.id.as_str()
        {
            return Err(Refused);
        }
        let mut actual = scoped_lane_actual_v29(function, budget, scratch)?;
        if actual.len() != chain.count {
            return Err(Refused);
        }
        let mut next = chain.first;
        let mut last = None;
        for _ in 0..chain.count {
            budget.charge_work(8)?;
            let ordinal = next.ok_or(Refused)?;
            let source = self.transport.rows.get(ordinal).ok_or(Refused)?;
            let mapped = self.map.spans.rows.get(source.span).ok_or(Refused)?;
            let [Some(segment), None] = mapped.segments else {
                return Err(Refused);
            };
            if mapped.instance != source.instance
                || mapped.source != InstanceSpanSourceV1::Terminator(source.source)
                || mapped.removed_call.is_some()
                || self.map.owner != Some(source.source.correspondence_owner)
                || segment.count != 1
            {
                return Err(Refused);
            }
            budget.charge_work(call_splice_search_work_v1(actual.len()))?;
            let position = actual
                .binary_search_by_key(&(segment.block, segment.first), |row| row.point)
                .map_err(|_| Refused)?;
            let row = &mut actual[position];
            if row.seen {
                return Err(Refused);
            }
            check_scoped_lane_operation_v29(row.operation, source.value, budget)?;
            row.seen = true;
            next = source.next;
            last = Some(ordinal);
        }
        budget.charge_work(actual.len())?;
        if next.is_some() || last != chain.last || actual.iter().any(|row| !row.seen) {
            return Err(Refused);
        }
        drop(actual);
        call_splice_charge_storage_v1(
            argument_sum_v1(&[
                std::mem::size_of::<ScopedLaneCalleeV29<'_>>(),
                2 * std::mem::size_of::<Result<ScopedLaneCalleeV29<'_>, CallInstanceEmissionErrorV1>>(
                ),
            ])?,
            budget,
            scratch,
        )?;
        Ok(ScopedLaneCalleeV29 {
            function,
            ledger: self.transport.ledger,
            slot: self.transport.slot,
            floor: budget.storage(),
        })
    }
}

impl ScopedLaneCalleeV29<'_> {
    fn check(
        &self,
        function: &Function,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), CallInstanceEmissionErrorV1> {
        if self.ledger != budget.work_ledger_identity_v1()
            || self.slot != std::ptr::from_ref(&*budget) as usize
            || budget.storage() < self.floor
        {
            return Err(ArgumentResourceV1::Accounting.into());
        }
        budget.charge_work(1)?;
        if !std::ptr::eq(self.function, function) {
            return Err(CallInstanceEmissionErrorV1::CalleeCollective);
        }
        Ok(())
    }
}
