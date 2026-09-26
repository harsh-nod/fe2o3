// Borrowed scalar definitions retained by exact source occurrences. These are
// index inputs only; lifecycle insertion and final source replay still apply.
#[derive(Clone, Copy)]
struct DeferredPartsSourceRowV29<'a> {
    next_value: u32,
    instance: Option<ProductionCallInstanceIdV1>,
    anchors: Option<&'a ScopedMemoryAnchorsV29>,
    events: Option<&'a PendingLifecycleEventsV29>,
    terminators: &'a [SemanticKirTerminatorOperationSpanV1],
}

impl<'a> DeferredPartsSourceRowV29<'a> {
    fn lowered(row: &'a LoweredFunctionResultV1) -> Self {
        Self {
            next_value: row.next_value,
            instance: row.source_call_instance,
            anchors: row.scoped_memory_anchors.as_ref(),
            events: row.lifecycle_events.as_ref(),
            terminators: &row.terminator_operation_spans,
        }
    }

    fn sidecar(row: &'a PendingInstanceSidecarsV29) -> Self {
        Self {
            next_value: row.next_value,
            instance: row.source_call_instance,
            anchors: row.scoped_memory_anchors.as_ref(),
            events: row.lifecycle_events.as_ref(),
            terminators: &row.terminator_operation_spans,
        }
    }
}

#[derive(Clone, Copy)]
enum DeferredPartsScopeV29<'a, 'p, 's> {
    Instance {
        plan: &'p ProductionCallInstancePlanV1<'s>,
        instance: ProductionCallInstanceIdV1,
        lowered: &'a LoweredFunctionResultV1,
    },
    Container {
        map: &'a ProductionInstanceCorrespondenceV1<'p, 's>,
        sidecars: &'a [PendingInstanceSidecarsV29],
        active: &'a PendingActiveInstanceIndexV1,
        container: ProductionCallInstanceIdV1,
    },
    PendingRoot {
        plan: &'p ProductionCallInstancePlanV1<'s>,
        pending: &'a PendingScopedRootEmissionV29,
        relocation: &'a scoped_slot_relocation_v29::RelocationV29,
    },
}

#[derive(Clone, Copy)]
struct DeferredPartsCoordinatesV29<'a> {
    owner: Option<SemanticFunctionIdV1>,
    spans: &'a [InstanceMappedSpanV1],
    controls: &'a [InstanceControlV1],
}

impl<'a> DeferredPartsCoordinatesV29<'a> {
    fn live(map: &'a ProductionInstanceCorrespondenceV1<'_, '_>) -> Self {
        Self {
            owner: map.owner,
            spans: &map.spans.rows,
            controls: &map.controls.rows,
        }
    }

    fn owned(coordinates: &'a OwnedInstanceCoordinatesV1) -> Self {
        Self {
            owner: Some(coordinates.root),
            spans: &coordinates.spans.rows,
            controls: &coordinates.controls.rows,
        }
    }
}

struct ScopedDeferredScalarViewV29<'a, 'p, 's> {
    scope: DeferredPartsScopeV29<'a, 'p, 's>,
    ledger: fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
    definitions: usize,
    inline_seed: Option<usize>,
}

fn deferred_parts_scalar_type_v29(offset: usize, elements: u16) -> &'static Type {
    static U32_TYPE: Type = Type::Scalar(ScalarType::U32);
    static BOOL_TYPE: Type = Type::Scalar(ScalarType::Bool);
    if offset < usize::from(elements) {
        &U32_TYPE
    } else {
        &BOOL_TYPE
    }
}

fn visit_scoped_parts_source_v29<'a>(
    instances: &ProductionCallInstancePlanV1<'_>,
    instance: ProductionCallInstanceIdV1,
    row: DeferredPartsSourceRowV29<'a>,
    budget: &mut ArgumentBudgetV1<'_>,
    mut visit: impl FnMut(
        &'a DeferredLifecycleEventV29,
        &'a DeferredTileEventV29,
        &'a SemanticKirTerminatorOperationSpanV1,
        &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let Some(events) = row.events else {
        return Ok(());
    };
    events.check_identity(instances, instance, budget)?;
    let source = instances
        .instance(instance)
        .ok_or_else(execution_lifecycle_error_v29)?;
    let calls = instances
        .calls(instance)
        .ok_or_else(execution_lifecycle_error_v29)?;
    let anchors = row.anchors.ok_or_else(execution_lifecycle_error_v29)?;
    if anchors.subject.ledger != budget.work_ledger_identity_v1() {
        return Err(ArgumentResourceV1::Accounting.into());
    }
    budget.charge_work(8)?;
    if row.instance != Some(instance)
        || anchors.subject.source != events.source
        || anchors.subject.instance != instance
        || anchors.subject.function != source.function()
        || anchors.placement != events.placement
        || events.rows.len() != events.expected_rows
        || row.next_value < events.placement.first_value
    {
        return Err(execution_lifecycle_error_v29());
    }
    for (event_index, event) in events.rows.iter().enumerate() {
        budget.charge_work(1)?;
        let DeferredLifecycleKindV29::Tile(tile) = &event.kind else {
            continue;
        };
        let DeferredTileInputV29::Parts { fragment } = tile.input else {
            continue;
        };
        budget.charge_work(argument_sum_v1(&[
            20,
            event_index,
            calls.len(),
            row.terminators.len(),
        ])?)?;
        let occurrence = ProductionCallOccurrenceV1 {
            caller: instance,
            block: event.block,
        };
        let range = tile.result_range()?;
        if tile.producer != occurrence
            || range.start < events.placement.first_value
            || range.end > row.next_value
            || events.placement.block(event.block.index())? != event.original_block
            || events.rows[..event_index]
                .iter()
                .any(|prior| prior.block == event.block)
        {
            return Err(execution_lifecycle_error_v29());
        }
        fragment
            .check_type(
                instances.owner().source_semantic().types(),
                SemanticExecutionRoleV29::LaneFragmentU32 {
                    lanes: tile.lanes,
                    elements: tile.elements,
                },
            )
            .map_err(|_| execution_lifecycle_error_v29())?;
        let semantic_block = source
            .declaration()
            .blocks()
            .get(event.block.index() as usize)
            .ok_or_else(execution_lifecycle_error_v29)?;
        let SemanticTerminatorKindV1::Call(source_call) = semantic_block.terminator().kind() else {
            return Err(execution_lifecycle_error_v29());
        };
        let mut matching_calls = calls.iter().filter(|call| call.occurrence() == occurrence);
        let call = matching_calls
            .next()
            .ok_or_else(execution_lifecycle_error_v29)?;
        if matching_calls.next().is_some()
            || !std::ptr::eq(call.source(), source_call)
            || call.child().is_some()
            || event.source
                != (DeferredLifecycleSourceV29::Intrinsic {
                    callee: source_call.callee(),
                })
            || source_call.arguments().len() != 1
            || source_call.arguments()[0].ty() != fragment.semantic_type
            || source_call
                .destination()
                .map(|destination| destination.place().ty())
                != Some(tile.result_type)
            || !matches!(call.callable(), SemanticCallableDeclV1::CompilerIntrinsic {
                operation: SemanticCompilerIntrinsicOperationV1::Execution(operation), ..
            } if *operation == tile.source_operation())
        {
            return Err(execution_lifecycle_error_v29());
        }
        let mut spans = row
            .terminators
            .iter()
            .filter(|span| span.semantic_block == event.block);
        let span = spans.next().ok_or_else(execution_lifecycle_error_v29)?;
        let end = span
            .first_operation_ordinal
            .checked_add(span.operation_count)
            .ok_or(ArgumentResourceV1::Arithmetic)?;
        if spans.next().is_some()
            || span.correspondence_owner != events.source.root
            || span.semantic_function != source.function()
            || span.kernel_ir_block != event.original_block
            || event.original_gap < span.first_operation_ordinal
            || event.original_gap > end
        {
            return Err(execution_lifecycle_error_v29());
        }
        visit(event, tile, span, budget)?;
    }
    Ok(())
}

fn deferred_parts_block_v29<'a>(
    function: &'a Function,
    id: BlockId,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<&'a BasicBlock, ProductionSemanticKirErrorV1> {
    let body = function
        .body
        .as_ref()
        .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
    budget.charge_work(body.blocks.len())?;
    let mut blocks = body.blocks.iter().filter(|block| block.id == id);
    let block = blocks
        .next()
        .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
    if blocks.next().is_some() {
        return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
    }
    Ok(block)
}

fn check_original_parts_location_v29(
    block: &BasicBlock,
    span: &SemanticKirTerminatorOperationSpanV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    budget.charge_work(3)?;
    let end = span
        .first_operation_ordinal
        .checked_add(span.operation_count)
        .ok_or(ArgumentResourceV1::Arithmetic)?;
    if block.id != span.kernel_ir_block || end as usize > block.operations.len() {
        return Err(execution_lifecycle_error_v29());
    }
    Ok(())
}

fn check_mapped_parts_location_v29(
    coordinates: DeferredPartsCoordinatesV29<'_>,
    instance: ProductionCallInstanceIdV1,
    event: &DeferredLifecycleEventV29,
    source_span: &SemanticKirTerminatorOperationSpanV1,
    function: &Function,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<InstancePhysicalSpanV1, ProductionSemanticKirErrorV1> {
    budget.charge_work(argument_sum_v1(&[
        coordinates.spans.len(),
        coordinates.controls.len(),
        8,
    ])?)?;
    if coordinates.owner != Some(source_span.correspondence_owner) {
        return Err(execution_lifecycle_error_v29());
    }
    let mut spans = coordinates.spans.iter().filter(|row| {
        row.instance == instance
            && matches!(row.source,
            InstanceSpanSourceV1::Terminator(original) if original == *source_span)
    });
    let span = spans.next().ok_or_else(execution_lifecycle_error_v29)?;
    let original = span.source.coordinates().2;
    let mapped = span.segments[0].ok_or_else(execution_lifecycle_error_v29)?;
    if spans.next().is_some()
        || span.removed_call.is_some()
        || span.segments[1].is_some()
        || original.count != mapped.count
        || event.original_gap < original.first
        || event.original_gap
            > original
                .end()
                .map_err(pending_scope_correspondence_error_v29)?
    {
        return Err(execution_lifecycle_error_v29());
    }
    let gap = mapped
        .first
        .checked_add(event.original_gap - original.first)
        .ok_or(ArgumentResourceV1::Arithmetic)?;
    let block = deferred_parts_block_v29(function, mapped.block, budget)?;
    if gap as usize > block.operations.len()
        || mapped
            .end()
            .map_err(pending_scope_correspondence_error_v29)? as usize
            > block.operations.len()
    {
        return Err(execution_lifecycle_error_v29());
    }
    let mut controls = coordinates.controls.iter().filter(|row| {
        row.instance == instance
            && row.original_block == event.original_block
            && row.semantic_block == Some(event.block)
    });
    let control = controls.next().ok_or_else(execution_lifecycle_error_v29)?;
    if controls.next().is_some()
        || control.physical_block != block.id
        || control.origin != InstanceControlOriginV1::Retained
        || control.return_values.is_some()
        || control.expected_branch.is_some()
    {
        return Err(execution_lifecycle_error_v29());
    }
    Ok(mapped)
}

impl<'a, 'p, 's> ScopedDeferredScalarViewV29<'a, 'p, 's> {
    fn for_pending_root(
        plan: &'p ProductionCallInstancePlanV1<'s>,
        pending: &'a PendingScopedRootEmissionV29,
        relocation: &'a scoped_slot_relocation_v29::RelocationV29,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Self, ProductionSemanticKirErrorV1> {
        Self::checked(
            DeferredPartsScopeV29::PendingRoot {
                plan,
                pending,
                relocation,
            },
            &pending.function,
            budget,
        )
    }

    fn for_instance(
        plan: &'p ProductionCallInstancePlanV1<'s>,
        instance: ProductionCallInstanceIdV1,
        lowered: &'a LoweredFunctionResultV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Self, ProductionSemanticKirErrorV1> {
        Self::checked(
            DeferredPartsScopeV29::Instance {
                plan,
                instance,
                lowered,
            },
            &lowered.function,
            budget,
        )
    }

    fn for_container(
        map: &'a ProductionInstanceCorrespondenceV1<'p, 's>,
        sidecars: &'a [PendingInstanceSidecarsV29],
        active: &'a PendingActiveInstanceIndexV1,
        container: ProductionCallInstanceIdV1,
        function: &Function,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Self, ProductionSemanticKirErrorV1> {
        Self::checked(
            DeferredPartsScopeV29::Container {
                map,
                sidecars,
                active,
                container,
            },
            function,
            budget,
        )
    }

    fn checked(
        scope: DeferredPartsScopeV29<'a, 'p, 's>,
        function: &Function,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Self, ProductionSemanticKirErrorV1> {
        let mut definitions = 0_usize;
        let mut inline_seed = None;
        match scope {
            DeferredPartsScopeV29::Instance {
                plan,
                instance,
                lowered,
            } => {
                budget.charge_work(1)?;
                if !std::ptr::eq(function, &lowered.function) {
                    return Err(execution_lifecycle_error_v29());
                }
                visit_scoped_parts_source_v29(
                    plan,
                    instance,
                    DeferredPartsSourceRowV29::lowered(lowered),
                    budget,
                    |event, tile, span, budget| {
                        let block =
                            deferred_parts_block_v29(function, event.original_block, budget)?;
                        check_original_parts_location_v29(block, span, budget)?;
                        budget.charge_work(1)?;
                        definitions = definitions
                            .checked_add(tile.result_range()?.len())
                            .ok_or(ArgumentResourceV1::Arithmetic)?;
                        Ok(())
                    },
                )?;
            }
            DeferredPartsScopeV29::Container {
                map,
                sidecars,
                active,
                container,
            } => {
                map.check_coordinates(container, function, budget)
                    .map_err(pending_scope_correspondence_error_v29)?;
                budget.charge_work(2)?;
                active.check_source_plan(map.plan, sidecars, budget)?;
                if map.seeds.rows.len() != sidecars.len() {
                    return Err(execution_lifecycle_error_v29());
                }
                budget.charge_work(map.seeds.rows.len())?;
                for (ordinal, seed) in map.seeds.rows.iter().enumerate() {
                    if seed.container != container {
                        continue;
                    }
                    if seed.instance == container {
                        if inline_seed.replace(ordinal).is_some() {
                            return Err(execution_lifecycle_error_v29());
                        }
                    }
                    budget.charge_work(2)?;
                    let ordinal = active.rows.get(seed.instance.index()).copied().flatten()
                        .ok_or_else(execution_lifecycle_error_v29)?;
                    let row = sidecars
                        .get(ordinal)
                        .ok_or_else(execution_lifecycle_error_v29)?;
                    if row.source_call_instance != Some(seed.instance) {
                        return Err(execution_lifecycle_error_v29());
                    }
                    visit_scoped_parts_source_v29(
                        map.plan,
                        seed.instance,
                        DeferredPartsSourceRowV29::sidecar(row),
                        budget,
                        |event, tile, span, budget| {
                            check_mapped_parts_location_v29(
                                DeferredPartsCoordinatesV29::live(map),
                                seed.instance,
                                event,
                                span,
                                function,
                                budget,
                            )?;
                            budget.charge_work(1)?;
                            definitions = definitions
                                .checked_add(tile.result_range()?.len())
                                .ok_or(ArgumentResourceV1::Arithmetic)?;
                            Ok(())
                        },
                    )?;
                }
            }
            DeferredPartsScopeV29::PendingRoot {
                plan,
                pending,
                relocation,
            } => {
                let root_events = pending
                    .sidecars
                    .rows
                    .first()
                    .and_then(|row| row.lifecycle_events.as_ref())
                    .ok_or_else(execution_lifecycle_error_v29)?;
                if root_events.ledger != budget.work_ledger_identity_v1() {
                    return Err(ArgumentResourceV1::Accounting.into());
                }
                budget.charge_work(8)?;
                if !std::ptr::eq(function, &pending.function)
                    || pending
                        .slot_relocation
                        .as_ref()
                        .is_some_and(|retained| !std::ptr::eq(retained, relocation))
                {
                    return Err(execution_lifecycle_error_v29());
                }
                let coordinates = &pending.coordinates;
                coordinates
                    .check_source_plan(plan, budget)
                    .map_err(pending_scope_correspondence_error_v29)?;
                let count = plan.instances().len();
                let active = &pending.active_instances;
                active.check_source_plan(plan, &pending.sidecars.rows, budget)?;
                if coordinates.seeds.rows.len() != pending.sidecars.rows.len()
                    || coordinates.sources.rows.len() != count
                {
                    return Err(execution_lifecycle_error_v29());
                }
                budget.charge_work(count)?;
                for (ordinal, sidecar_ordinal) in active.rows.iter().copied().enumerate() {
                    let Some(sidecar_ordinal) = sidecar_ordinal else { continue };
                    let sidecar = &pending.sidecars.rows[sidecar_ordinal];
                    let instance = plan
                        .id_at(ordinal)
                        .ok_or_else(execution_lifecycle_error_v29)?;
                    budget.charge_work(argument_sum_v1(&[coordinates.seeds.rows.len(), 3])?)?;
                    let mut seeds = coordinates
                        .seeds
                        .rows
                        .iter()
                        .filter(|seed| seed.instance == instance);
                    let seed = seeds.next().ok_or_else(execution_lifecycle_error_v29)?;
                    if seeds.next().is_some()
                        || seed.container != plan.root()
                        || sidecar.source_call_instance != Some(instance)
                    {
                        return Err(execution_lifecycle_error_v29());
                    }
                    if instance == plan.root() {
                        budget.charge_work(argument_sum_v1(&[
                            seed.function_name.len(),
                            function.id.as_str().len(),
                            2,
                        ])?)?;
                        if seed.function_name != function.id.as_str() {
                            return Err(execution_lifecycle_error_v29());
                        }
                    }
                    let events = sidecar
                        .lifecycle_events
                        .as_ref()
                        .ok_or_else(execution_lifecycle_error_v29)?;
                    let source = lifecycle_source_row_v29(coordinates, events, budget)?;
                    if source.instance != instance {
                        return Err(execution_lifecycle_error_v29());
                    }
                    visit_scoped_parts_source_v29(
                        plan,
                        instance,
                        DeferredPartsSourceRowV29::sidecar(sidecar),
                        budget,
                        |event, tile, span, budget| {
                            let mapped = check_mapped_parts_location_v29(
                                DeferredPartsCoordinatesV29::owned(coordinates),
                                instance,
                                event,
                                span,
                                function,
                                budget,
                            )?;
                            let original = InstanceSpanSourceV1::Terminator(*span).coordinates().2;
                            // Parts terminators are not defined-call splice sites.
                            let expected = relocation.assertion_span(original, budget)?;
                            if mapped != expected {
                                return Err(execution_lifecycle_error_v29());
                            }
                            budget.charge_work(1)?;
                            definitions = definitions
                                .checked_add(tile.result_range()?.len())
                                .ok_or(ArgumentResourceV1::Arithmetic)?;
                            Ok(())
                        },
                    )?;
                }
            }
        }
        Ok(Self {
            scope,
            ledger: budget.work_ledger_identity_v1(),
            definitions,
            inline_seed,
        })
    }
}

impl ScopedDeferredScalarViewV29<'_, '_, '_> {
    fn check_function(
        &self,
        function: &Function,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), CallInstanceEmissionErrorV1> {
        self.check_ledger(budget)?;
        let checked = Self::checked(self.scope, function, budget).map_err(|error| match error {
            ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(error) => error.into(),
            _ => CallInstanceEmissionErrorV1::ExecutionTransport,
        })?;
        if checked.definitions != self.definitions || checked.inline_seed != self.inline_seed {
            return Err(ArgumentResourceV1::Accounting.into());
        }
        Ok(())
    }

    fn check_ledger(&self, budget: &ArgumentBudgetV1<'_>) -> Result<(), ArgumentResourceV1> {
        if self.ledger != budget.work_ledger_identity_v1() {
            return Err(ArgumentResourceV1::Accounting);
        }
        Ok(())
    }

    fn visit<E: From<ArgumentResourceV1>>(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
        mut emit: impl FnMut(ValueId, &'static Type, &mut ArgumentBudgetV1<'_>) -> Result<(), E>,
    ) -> Result<(), E> {
        self.check_ledger(budget)?;
        let mut seen = 0_usize;
        match self.scope {
            DeferredPartsScopeV29::Instance { lowered, .. } => {
                budget.charge_work(1)?;
                emit_checked_parts_events_v29(
                    lowered.lifecycle_events.as_ref(),
                    self.definitions,
                    &mut seen,
                    budget,
                    &mut emit,
                )?;
            }
            DeferredPartsScopeV29::Container {
                map,
                sidecars,
                active,
                container,
            } => {
                budget.charge_work(map.seeds.rows.len())?;
                for seed in &map.seeds.rows {
                    if seed.container != container {
                        continue;
                    }
                    let ordinal = active.sidecar_ordinal(seed.instance.index(),
                        map.plan.instances().len(), sidecars, budget)
                        .map_err(|error| match error {
                            ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(error) => error,
                            _ => ArgumentResourceV1::Accounting,
                        })?
                        .ok_or(ArgumentResourceV1::Accounting)?;
                    let row = &sidecars[ordinal];
                    emit_checked_parts_events_v29(
                        row.lifecycle_events.as_ref(),
                        self.definitions,
                        &mut seen,
                        budget,
                        &mut emit,
                    )?;
                }
            }
            DeferredPartsScopeV29::PendingRoot { pending, .. } => {
                budget.charge_work(pending.sidecars.rows.len())?;
                for row in &pending.sidecars.rows {
                    emit_checked_parts_events_v29(
                        row.lifecycle_events.as_ref(),
                        self.definitions,
                        &mut seen,
                        budget,
                        &mut emit,
                    )?;
                }
            }
        }
        if seen != self.definitions {
            return Err(ArgumentResourceV1::Accounting.into());
        }
        Ok(())
    }
}

fn emit_checked_parts_events_v29<E: From<ArgumentResourceV1>>(
    events: Option<&PendingLifecycleEventsV29>,
    expected: usize,
    seen: &mut usize,
    budget: &mut ArgumentBudgetV1<'_>,
    emit: &mut impl FnMut(ValueId, &'static Type, &mut ArgumentBudgetV1<'_>) -> Result<(), E>,
) -> Result<(), E> {
    let Some(events) = events else {
        return Ok(());
    };
    budget.charge_work(events.rows.len())?;
    for event in &events.rows {
        let DeferredLifecycleKindV29::Tile(tile) = event.kind else {
            continue;
        };
        if !matches!(tile.input, DeferredTileInputV29::Parts { .. }) {
            continue;
        }
        let count = u32::from(tile.elements)
            .checked_mul(2)
            .ok_or(ArgumentResourceV1::Arithmetic)?;
        let end = tile
            .first_result
            .0
            .checked_add(count)
            .ok_or(ArgumentResourceV1::Arithmetic)?;
        budget.charge_work(usize::try_from(count).map_err(|_| ArgumentResourceV1::Arithmetic)?)?;
        for (offset, value) in (tile.first_result.0..end).enumerate() {
            let next = seen.checked_add(1).ok_or(ArgumentResourceV1::Arithmetic)?;
            if next > expected {
                return Err(ArgumentResourceV1::Accounting.into());
            }
            *seen = next;
            emit(
                ValueId(value),
                deferred_parts_scalar_type_v29(offset, tile.elements),
                budget,
            )?;
        }
    }
    Ok(())
}

fn scoped_call_index_with_deferred_parts_v29<'a>(
    instances: &ProductionCallInstancePlanV1<'_>,
    instance: ProductionCallInstanceIdV1,
    lowered: &'a LoweredFunctionResultV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<CallFunctionIndexV1<'a>, ProductionSemanticKirErrorV1> {
    let mut index = CallFunctionIndexV1::new(&lowered.function, budget)?;
    let mut added = false;
    visit_scoped_parts_source_v29(
        instances,
        instance,
        DeferredPartsSourceRowV29::lowered(lowered),
        budget,
        |event, tile, span, budget| {
            let block = index.block(event.original_block, budget)?;
            check_original_parts_location_v29(block, span, budget)?;
            let range = tile.result_range()?;
            budget.charge_work(range.len())?;
            for (offset, value) in range.enumerate() {
                emission_push_v1(
                    &mut index.values,
                    (
                        ValueId(value),
                        deferred_parts_scalar_type_v29(offset, tile.elements),
                    ),
                    budget,
                )?;
            }
            added = true;
            Ok(())
        },
    )?;
    if added {
        budget.charge_work(argument_product_v1(index.values.len(), 100)?)?;
        sort_correspondence_keys_v1(&mut index.values, 31, &|row| u64::from(row.0.0));
        if index.values.windows(2).any(|pair| pair[0].0 == pair[1].0) {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
    }
    Ok(index)
}

fn call_splice_index_with_deferred_parts_v29<'a>(
    function: &'a Function,
    parts: Option<&ScopedDeferredScalarViewV29<'_, '_, '_>>,
    budget: &mut ArgumentBudgetV1<'_>,
    scratch: &mut usize,
) -> Result<CallSpliceIndexV1<'a>, CallInstanceEmissionErrorV1> {
    let mut index = call_splice_index_v1(function, budget, scratch)?;
    let Some(parts) = parts else {
        return Ok(index);
    };
    parts.check_function(function, budget)?;
    if parts.definitions == 0 {
        return Ok(index);
    }
    let count = index
        .values
        .len()
        .checked_add(parts.definitions)
        .ok_or_else(call_splice_arithmetic_v1)?;
    let old_bytes = index
        .values
        .len()
        .checked_mul(std::mem::size_of::<(ValueId, &Type)>())
        .ok_or_else(call_splice_arithmetic_v1)?;
    let mut values = call_splice_vec_v1(count, budget, scratch)?;
    budget.charge_work(index.values.len())?;
    values.extend_from_slice(&index.values);
    parts.visit(budget, |id, ty, _| {
        if values.len() >= count {
            return Err(ArgumentResourceV1::Accounting.into());
        }
        values.push((id, ty));
        Ok::<_, CallInstanceEmissionErrorV1>(())
    })?;
    if values.len() != count {
        return Err(ArgumentResourceV1::Accounting.into());
    }
    call_splice_sort_work_v1(values.len(), budget)?;
    values.sort_unstable_by_key(|row| row.0);
    budget.charge_work(values.len())?;
    if values.windows(2).any(|rows| rows[0].0 == rows[1].0) {
        return Err(CallInstanceEmissionErrorV1::DuplicateIdentity);
    }
    let remaining = scratch
        .checked_sub(old_bytes)
        .ok_or(ArgumentResourceV1::Accounting)?;
    let old = std::mem::replace(&mut index.values, values);
    drop(old);
    budget.release_storage(old_bytes)?;
    *scratch = remaining;
    Ok(index)
}

impl ProductionInstanceCorrespondenceV1<'_, '_> {
    #[allow(clippy::too_many_arguments)]
    fn splice_with_scoped_parts_v29(
        &mut self,
        call: &ProductionInstanceCallV1<'_>,
        caller: Function,
        callee: Function,
        entry: BlockId,
        continuation: BlockId,
        frame: Option<&scoped_slot_relocation_v29::FramePermitV29<'_, '_>>,
        sidecars: &[PendingInstanceSidecarsV29],
        active: &PendingActiveInstanceIndexV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> InstanceMapResultV1<SplicedCallInstanceV1> {
        self.splice_with_scoped_storage_v29(call, caller, callee, entry, continuation, frame, sidecars, active, None, budget)
    }

    #[allow(clippy::too_many_arguments)]
    fn splice_with_scoped_storage_v29(
        &mut self,
        call: &ProductionInstanceCallV1<'_>,
        caller: Function,
        callee: Function,
        entry: BlockId,
        continuation: BlockId,
        frame: Option<&scoped_slot_relocation_v29::FramePermitV29<'_, '_>>,
        sidecars: &[PendingInstanceSidecarsV29],
        active: &PendingActiveInstanceIndexV1,
        storage: Option<&ScopedStorageTransportV29>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> InstanceMapResultV1<SplicedCallInstanceV1> {
        self.splice_with_scoped_queries_v29(call, caller, callee, entry, continuation, frame,
            sidecars, active, storage, None, budget)
    }

    #[allow(clippy::too_many_arguments)]
    fn splice_with_scoped_queries_v29(
        &mut self,
        call: &ProductionInstanceCallV1<'_>,
        caller: Function,
        callee: Function,
        entry: BlockId,
        continuation: BlockId,
        frame: Option<&scoped_slot_relocation_v29::FramePermitV29<'_, '_>>,
        sidecars: &[PendingInstanceSidecarsV29],
        active: &PendingActiveInstanceIndexV1,
        storage: Option<&ScopedStorageTransportV29>,
        lane: Option<&ScopedLaneQueryTransportV29>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> InstanceMapResultV1<SplicedCallInstanceV1> {
        self.check_live_ledger_v1(budget)?;
        let result = self.splice_inner(
            call,
            caller,
            callee,
            entry,
            continuation,
            frame,
            Some((sidecars, active)),
            storage,
            lane,
            budget,
        );
        self.failed = result.is_err();
        result
    }
}
