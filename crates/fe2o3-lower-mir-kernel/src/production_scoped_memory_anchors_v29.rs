// Producer-owned physical coordinates and source invalidation occurrences.

#[derive(Clone, Copy)]
struct ScopedMemorySpanV29 {
    site: (u32, Option<u32>),
    block: BlockId,
    start: usize,
    end: usize,
}

fn scoped_memory_operand_role_v29(
    role: ExecutionOperandV29,
) -> Result<bool, ProductionSemanticKirErrorV1> {
    Ok(match role {
        ExecutionOperandV29::RvalueOperand(_)
        | ExecutionOperandV29::StoreValue
        | ExecutionOperandV29::AtomicValue
        | ExecutionOperandV29::AtomicExpected
        | ExecutionOperandV29::AtomicReplacement
        | ExecutionOperandV29::Assume
        | ExecutionOperandV29::CallArgument(_)
        | ExecutionOperandV29::TailCallArgument(_)
        | ExecutionOperandV29::SwitchDiscriminant
        | ExecutionOperandV29::AssertCondition
        | ExecutionOperandV29::AssertMessage(_) => true,
        ExecutionOperandV29::RvaluePlace
        | ExecutionOperandV29::Destination
        | ExecutionOperandV29::StoreDestination
        | ExecutionOperandV29::AtomicAddress
        | ExecutionOperandV29::AtomicDestination
        | ExecutionOperandV29::CallDestinationAddress
        | ExecutionOperandV29::DropPlace
        | ExecutionOperandV29::ReturnValue
        | ExecutionOperandV29::ElidedBorrowDestination
        | ExecutionOperandV29::StatementPlace => false,
        ExecutionOperandV29::StorageLive | ExecutionOperandV29::StorageDead => {
            return Err(scoped_memory_error_v29());
        }
    })
}

fn scoped_memory_site_key_v29(site: ExecutionSiteV29) -> (u32, Option<u32>) {
    match site {
        ExecutionSiteV29::Statement { block, statement } => (block.get(), Some(statement)),
        ExecutionSiteV29::Terminator { block } => (block.get(), None),
    }
}

fn check_scoped_memory_anchors_v29(
    instances: &ExecutionInstancesV29<'_>,
    instance: &ScopedSourceSlotInstanceV29,
    lowered: &LoweredFunctionResultV1,
    slots: &[ScopedSourceSlotV29],
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let anchors = lowered
        .scoped_memory_anchors
        .as_ref()
        .ok_or_else(scoped_memory_error_v29)?;
    if anchors.subject.ledger != budget.work_ledger_identity_v1() {
        return Err(ArgumentResourceV1::Accounting.into());
    }
    budget.charge_work(argument_sum_v1(&[8, slots.len()])?)?;
    let source = instances
        .instance(instance.instance)
        .ok_or_else(scoped_memory_error_v29)?;
    if anchors.subject.instance != instance.instance
        || anchors.subject.function != instance.function
        || anchors.subject.function != source.function()
        || anchors.subject.source != ExecutionCallSourceV29::from_instances(instances, budget)?
        || anchors.placement != instance.placement
        || lowered.source_call_instance != Some(instance.instance)
        || slots.iter().any(|slot| slot.instance != instance.instance)
        || slots
            .windows(2)
            .any(|pair| pair[0].origin.local >= pair[1].origin.local)
    {
        return Err(scoped_memory_error_v29());
    }
    anchors.retained_storage()?;
    let occurrences = instances
        .occurrences(instance.instance)
        .ok_or_else(scoped_memory_error_v29)?;
    let mut expected = emission_vec_v1(occurrences.events().len(), budget)?;
    for event in occurrences.events() {
        budget.charge_work(scoped_initialization_search_work_v29(slots.len()))?;
        let local = event.event().variable().get();
        budget.charge_work(4)?;
        let candidate = slots.binary_search_by_key(&local, |slot| slot.origin.local);
        let expectation = match candidate {
            Ok(index) => {
                let ty = instances
                    .owner()
                    .source_semantic()
                    .types()
                    .get(slots[index].origin.semantic_type.index() as usize)
                    .ok_or_else(scoped_memory_error_v29)?;
                if event.is_reachable()
                    && event.role() == ExecutionEventV29::BaseUse
                    && matches!(ty.shape(), SemanticTypeShapeV1::Array { .. })
                    && scoped_memory_operand_role_v29(event.operand())?
                {
                    match scoped_source_operand_v29(
                        source.declaration(),
                        event.site(),
                        event.operand(),
                    ) {
                        Some(SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place))
                            if place.local().index() == local => {}
                        _ => return Err(scoped_memory_error_v29()),
                    }
                }
                scoped_expected_kill_v29(
                    source.declaration(),
                    event,
                    matches!(ty.shape(), SemanticTypeShapeV1::Array { .. }),
                )
            }
            Err(_) => None,
        };
        if event.is_reachable()
            && candidate.is_ok()
            && matches!(
                event.role(),
                ExecutionEventV29::MoveKill | ExecutionEventV29::StorageKill
            )
            && expectation.is_none()
        {
            return Err(scoped_memory_error_v29());
        }
        expected.push(expectation);
    }
    let count = argument_sum_v1(&[
        lowered.statement_operation_spans.len(),
        lowered.terminator_operation_spans.len(),
    ])?;
    let mut spans = emission_vec_v1(count, budget)?;
    budget.charge_work(argument_product_v1(count, 3)?)?;
    for span in &lowered.statement_operation_spans {
        if span.correspondence_owner != anchors.subject.source.root
            || span.semantic_function != anchors.subject.function
        {
            return Err(scoped_memory_error_v29());
        }
        spans.push(ScopedMemorySpanV29 {
            site: (span.semantic_block.index(), Some(span.statement_ordinal)),
            block: span.kernel_ir_block,
            start: span.first_operation_ordinal as usize,
            end: argument_sum_v1(&[
                span.first_operation_ordinal as usize,
                span.operation_count as usize,
            ])?,
        });
    }
    for span in &lowered.terminator_operation_spans {
        if span.correspondence_owner != anchors.subject.source.root
            || span.semantic_function != anchors.subject.function
        {
            return Err(scoped_memory_error_v29());
        }
        spans.push(ScopedMemorySpanV29 {
            site: (span.semantic_block.index(), None),
            block: span.kernel_ir_block,
            start: span.first_operation_ordinal as usize,
            end: argument_sum_v1(&[
                span.first_operation_ordinal as usize,
                span.operation_count as usize,
            ])?,
        });
    }
    call_splice_sort_work_v1(argument_product_v1(spans.len(), 2)?, budget).map_err(|error| {
        match error {
            CallInstanceEmissionErrorV1::Resource(error) => error.into(),
            _ => scoped_memory_error_v29(),
        }
    })?;
    spans.sort_unstable_by_key(|row| row.site);
    budget.charge_work(argument_product_v1(spans.len(), 2)?)?;
    if spans.windows(2).any(|pair| pair[0].site == pair[1].site) {
        return Err(scoped_memory_error_v29());
    }
    let body = lowered
        .function
        .body
        .as_ref()
        .ok_or_else(scoped_memory_error_v29)?;
    // Producer order follows the original emitted blocks, not sorted BlockId.
    let mut next = 0;
    for block in &body.blocks {
        budget.charge_work(argument_sum_v1(&[1, block.operations.len()])?)?;
        for position in 0..=block.operations.len() {
            loop {
                budget.charge_work(1)?;
                let Some(row) = anchors.rows.get(next) else {
                    break;
                };
                if row.block != block.id
                    || row.position != position
                    || !matches!(row.kind, ScopedMemoryAnchorKindV29::Kill { .. })
                {
                    break;
                }
                next = argument_sum_v1(&[next, 1])?;
            }
            if let Some(pointer) = block
                .operations
                .get(position)
                .and_then(|operation| scoped_memory_pointer_v29(&operation.kind))
            {
                budget.charge_work(1)?;
                let row = anchors.rows.get(next).ok_or_else(scoped_memory_error_v29)?;
                if row.block != block.id
                    || row.position != position
                    || row.kind != (ScopedMemoryAnchorKindV29::Access { pointer })
                {
                    return Err(scoped_memory_error_v29());
                }
                check_scoped_call_memory_frame_v29(
                    source.declaration(),
                    row.source,
                    &block.operations[position].kind,
                    budget,
                )?;
                next = argument_sum_v1(&[next, 1])?;
            }
        }
    }
    if next != anchors.rows.len() {
        return Err(scoped_memory_error_v29());
    }
    for row in &anchors.rows {
        budget.charge_work(3)?;
        if let Some(frame) = row.source {
            let key = scoped_memory_site_key_v29(frame.site);
            budget.charge_work(argument_product_v1(
                2,
                scoped_initialization_search_work_v29(spans.len()),
            )?)?;
            let index = spans
                .binary_search_by_key(&key, |span| span.site)
                .map_err(|_| scoped_memory_error_v29())?;
            let span = spans[index];
            let is_access = matches!(row.kind, ScopedMemoryAnchorKindV29::Access { .. });
            if row.block != anchors.placement.block(key.0)?
                || row.block != span.block
                || row.position < span.start
                || row.position > span.end
                || (is_access && row.position == span.end)
            {
                return Err(scoped_memory_error_v29());
            }
        }
        if let ScopedMemoryAnchorKindV29::Kill {
            event,
            local,
            cause,
        } = row.kind
        {
            let frame = row.source.ok_or_else(scoped_memory_error_v29)?;
            let original = occurrences
                .events()
                .get(event)
                .ok_or_else(scoped_memory_error_v29)?;
            if original.site() != frame.site
                || Some(ScopedMemoryRoleV29::Operand(original.operand())) != frame.role
                || original.role() != cause.event_role()
                || expected.get_mut(event).and_then(Option::take) != Some((local, cause))
            {
                return Err(scoped_memory_error_v29());
            }
        }
    }
    budget.charge_work(expected.len())?;
    if expected.iter().any(Option::is_some) {
        return Err(scoped_memory_error_v29());
    }
    Ok(())
}
// These rows are not an independent access-equivalence or relocation proof.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ScopedMemoryFrameV29 {
    site: ExecutionSiteV29,
    role: Option<ScopedMemoryRoleV29>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ScopedMemoryRoleV29 {
    Operand(ExecutionOperandV29),
    CallResult,
}

impl ScopedMemoryFrameV29 {
    fn operand(site: ExecutionSiteV29, role: Option<ExecutionOperandV29>) -> Self {
        Self {
            site,
            role: role.map(ScopedMemoryRoleV29::Operand),
        }
    }
}

fn scoped_source_call_destination_v29(
    function: &SemanticFunctionDeclV1,
    site: ExecutionSiteV29,
) -> Option<&SemanticPlaceV1> {
    let ExecutionSiteV29::Terminator { block } = site else {
        return None;
    };
    let SemanticTerminatorKindV1::Call(call) = function
        .blocks()
        .get(block.get() as usize)?
        .terminator()
        .kind()
    else {
        return None;
    };
    Some(call.destination()?.place())
}

fn check_scoped_call_memory_frame_v29(
    function: &SemanticFunctionDeclV1,
    frame: Option<ScopedMemoryFrameV29>,
    operation: &OperationKind,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    budget.charge_work(4)?;
    let Some(frame) = frame else {
        return Ok(());
    };
    let valid_kind = match frame.role {
        Some(ScopedMemoryRoleV29::Operand(ExecutionOperandV29::CallDestinationAddress)) => {
            matches!(
                operation,
                OperationKind::Load { .. } | OperationKind::GuardedLoad { .. }
            )
        }
        Some(ScopedMemoryRoleV29::CallResult) => matches!(
            operation,
            OperationKind::Store { .. } | OperationKind::GuardedStore { .. }
        ),
        _ => return Ok(()),
    };
    if !valid_kind || scoped_source_call_destination_v29(function, frame.site).is_none() {
        return Err(scoped_memory_error_v29());
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ScopedMemoryKillV29 {
    StorageLive,
    StorageDead,
    Deinitialize,
    Move,
    ProjectedArrayMove,
}

impl ScopedMemoryKillV29 {
    fn event_role(self) -> ExecutionEventV29 {
        match self {
            Self::StorageLive | Self::StorageDead => ExecutionEventV29::StorageKill,
            Self::Move => ExecutionEventV29::MoveKill,
            Self::Deinitialize | Self::ProjectedArrayMove => ExecutionEventV29::BaseUse,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ScopedMemoryAnchorKindV29 {
    Access {
        pointer: ValueId,
    },
    Kill {
        event: usize,
        local: u32,
        cause: ScopedMemoryKillV29,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ScopedMemoryAnchorV29 {
    block: BlockId,
    // Access: original operation ordinal. Kill: gap before this operation.
    position: usize,
    source: Option<ScopedMemoryFrameV29>,
    kind: ScopedMemoryAnchorKindV29,
}

struct ScopedMemoryAnchorsV29 {
    subject: ScopedInitializationSubjectV29,
    placement: SemanticEmissionPlacementV1,
    rows: Vec<ScopedMemoryAnchorV29>,
}

impl ScopedMemoryAnchorsV29 {
    fn retained_storage(&self) -> Result<usize, ProductionSemanticKirErrorV1> {
        Ok(argument_product_v1(
            self.rows.capacity(),
            std::mem::size_of::<ScopedMemoryAnchorV29>(),
        )?)
    }
}

struct ScopedMemoryRecorderV29 {
    anchors: ScopedMemoryAnchorsV29,
    block: Option<BlockId>,
    frame: Option<ScopedMemoryFrameV29>,
}

impl ScopedMemoryRecorderV29 {
    fn new(cursor: &ExecutionAvailabilityV29<'_>, placement: SemanticEmissionPlacementV1) -> Self {
        Self {
            anchors: ScopedMemoryAnchorsV29 {
                subject: ScopedInitializationSubjectV29::from_cursor(cursor),
                placement,
                rows: Vec::new(),
            },
            block: None,
            frame: None,
        }
    }
}

fn scoped_memory_error_v29() -> ProductionSemanticKirErrorV1 {
    unsupported(
        0,
        None,
        None,
        "scoped memory anchors differ from their source instance",
    )
}

fn scoped_memory_pointer_v29(kind: &OperationKind) -> Option<ValueId> {
    match kind {
        OperationKind::Load { pointer, .. }
        | OperationKind::GuardedLoad { pointer, .. }
        | OperationKind::Store { pointer, .. }
        | OperationKind::GuardedStore { pointer, .. } => Some(*pointer),
        _ => None,
    }
}

fn scoped_source_statement_v29(
    function: &SemanticFunctionDeclV1,
    site: ExecutionSiteV29,
) -> Option<&SemanticStatementKindV1> {
    let ExecutionSiteV29::Statement { block, statement } = site else {
        return None;
    };
    Some(
        function
            .blocks()
            .get(block.get() as usize)?
            .statements()
            .get(statement as usize)?
            .kind(),
    )
}

fn scoped_source_operand_v29(
    function: &SemanticFunctionDeclV1,
    site: ExecutionSiteV29,
    role: ExecutionOperandV29,
) -> Option<&SemanticOperandV1> {
    match (site, role) {
        (ExecutionSiteV29::Terminator { block }, ExecutionOperandV29::CallArgument(index)) => {
            let SemanticTerminatorKindV1::Call(call) = function
                .blocks()
                .get(block.get() as usize)?
                .terminator()
                .kind()
            else {
                return None;
            };
            call.arguments().get(index as usize)
        }
        (ExecutionSiteV29::Terminator { block }, role) => match function
            .blocks()
            .get(block.get() as usize)?
            .terminator()
            .kind()
        {
            SemanticTerminatorKindV1::SwitchInt { discriminant, .. }
                if role == ExecutionOperandV29::SwitchDiscriminant =>
            {
                Some(discriminant)
            }
            SemanticTerminatorKindV1::Assert {
                condition, message, ..
            } => match role {
                ExecutionOperandV29::AssertCondition => Some(condition),
                ExecutionOperandV29::AssertMessage(index) => {
                    execution_assert_operand_v29(message, index)
                }
                _ => None,
            },
            _ => None,
        },
        (
            ExecutionSiteV29::Statement { block, statement },
            ExecutionOperandV29::RvalueOperand(index),
        ) => {
            let SemanticStatementKindV1::Assign(assignment) = function
                .blocks()
                .get(block.get() as usize)?
                .statements()
                .get(statement as usize)?
                .kind()
            else {
                return None;
            };
            match assignment.value().kind() {
                SemanticRvalueKindV1::Use(operand)
                | SemanticRvalueKindV1::Unary { operand, .. }
                | SemanticRvalueKindV1::Cast { operand, .. }
                    if index == 0 =>
                {
                    Some(operand)
                }
                SemanticRvalueKindV1::Binary { left, right, .. } => match index {
                    0 => Some(left),
                    1 => Some(right),
                    _ => None,
                },
                SemanticRvalueKindV1::CheckedBinary(operation) => match index {
                    0 => Some(operation.left()),
                    1 => Some(operation.right()),
                    _ => None,
                },
                SemanticRvalueKindV1::UncheckedBinary(operation) => match index {
                    0 => Some(operation.left()),
                    1 => Some(operation.right()),
                    _ => None,
                },
                SemanticRvalueKindV1::Aggregate(aggregate) => {
                    aggregate.operands().get(index as usize)
                }
                _ => None,
            }
        }
        (ExecutionSiteV29::Statement { block, statement }, ExecutionOperandV29::Assume) => {
            let SemanticStatementKindV1::Assume(condition) = function
                .blocks()
                .get(block.get() as usize)?
                .statements()
                .get(statement as usize)?
                .kind()
            else {
                return None;
            };
            Some(condition)
        }
        (ExecutionSiteV29::Statement { block, statement }, role) => {
            match function
                .blocks()
                .get(block.get() as usize)?
                .statements()
                .get(statement as usize)?
                .kind()
            {
                SemanticStatementKindV1::Store(store)
                    if role == ExecutionOperandV29::StoreValue =>
                {
                    Some(store.value())
                }
                SemanticStatementKindV1::AtomicRmw(atomic)
                    if role == ExecutionOperandV29::AtomicValue =>
                {
                    Some(atomic.value())
                }
                _ => None,
            }
        }
    }
}

fn scoped_source_place_v29(
    function: &SemanticFunctionDeclV1,
    site: ExecutionSiteV29,
    role: ExecutionOperandV29,
) -> Option<&SemanticPlaceV1> {
    if role == ExecutionOperandV29::CallDestinationAddress {
        return scoped_source_call_destination_v29(function, site);
    }
    match scoped_source_statement_v29(function, site)? {
        SemanticStatementKindV1::Assign(assignment) => match role {
            ExecutionOperandV29::Destination => Some(assignment.destination()),
            ExecutionOperandV29::RvaluePlace => match assignment.value().kind() {
                SemanticRvalueKindV1::Borrow { place, .. }
                | SemanticRvalueKindV1::AddressOf { place, .. } => Some(place),
                SemanticRvalueKindV1::Load(load) => Some(load.source()),
                _ => None,
            },
            _ => None,
        },
        SemanticStatementKindV1::Store(store) if role == ExecutionOperandV29::StoreDestination => {
            Some(store.destination())
        }
        SemanticStatementKindV1::AtomicRmw(atomic) => match role {
            ExecutionOperandV29::AtomicAddress => Some(atomic.address()),
            ExecutionOperandV29::AtomicDestination => Some(atomic.destination()),
            _ => None,
        },
        SemanticStatementKindV1::Deinitialize(place)
            if role == ExecutionOperandV29::StatementPlace =>
        {
            Some(place)
        }
        _ => None,
    }
}

fn scoped_expected_kill_v29(
    function: &SemanticFunctionDeclV1,
    event: &fe2o3_pliron::ProductionSemanticSsaEventOccurrenceV1,
    array: bool,
) -> Option<(u32, ScopedMemoryKillV29)> {
    use fe2o3_mir_model::SsaEventV1;
    if !event.is_reachable() {
        return None;
    }
    let local = event.event().variable().get();
    let cause =
        match (event.role(), event.event()) {
            (ExecutionEventV29::StorageKill, SsaEventV1::Kill(_)) => {
                match (
                    event.operand(),
                    scoped_source_statement_v29(function, event.site())?,
                ) {
                    (
                        ExecutionOperandV29::StorageLive,
                        SemanticStatementKindV1::StorageLive(id),
                    ) if id.index() == local => ScopedMemoryKillV29::StorageLive,
                    (
                        ExecutionOperandV29::StorageDead,
                        SemanticStatementKindV1::StorageDead(id),
                    ) if id.index() == local => ScopedMemoryKillV29::StorageDead,
                    _ => return None,
                }
            }
            (ExecutionEventV29::MoveKill, SsaEventV1::Kill(_)) => {
                let SemanticOperandV1::Move(place) =
                    scoped_source_operand_v29(function, event.site(), event.operand())?
                else {
                    return None;
                };
                if place.local().index() != local || !place.projections().is_empty() {
                    return None;
                }
                ScopedMemoryKillV29::Move
            }
            (ExecutionEventV29::BaseUse, SsaEventV1::Use(_)) => {
                if event.operand() == ExecutionOperandV29::StatementPlace {
                    let SemanticStatementKindV1::Deinitialize(place) =
                        scoped_source_statement_v29(function, event.site())?
                    else {
                        return None;
                    };
                    if place.local().index() != local {
                        return None;
                    }
                    ScopedMemoryKillV29::Deinitialize
                } else {
                    let SemanticOperandV1::Move(place) =
                        scoped_source_operand_v29(function, event.site(), event.operand())?
                    else {
                        return None;
                    };
                    if !array || place.local().index() != local || place.projections().is_empty() {
                        return None;
                    }
                    ScopedMemoryKillV29::ProjectedArrayMove
                }
            }
            _ => return None,
        };
    Some((local, cause))
}

impl SemanticFunctionLoweringV1<'_> {
    fn with_scoped_call_memory_frame_v29<T>(
        &mut self,
        block: SemanticBlockIdV1,
        destination: &SemanticPlaceV1,
        result: bool,
        body: impl FnOnce(&mut Self) -> Result<T, ProductionSemanticKirErrorV1>,
    ) -> Result<T, ProductionSemanticKirErrorV1> {
        if self.scoped_memory.is_none() {
            return body(self);
        }
        let site = execution_site_v29(block, None);
        self.with_emission_budget_v1(|this, budget| {
            budget.charge_work(4)?;
            this.execution
                .as_ref()
                .ok_or_else(scoped_memory_error_v29)?
                .check_ledger(budget)?;
            if !scoped_source_call_destination_v29(this.function, site)
                .is_some_and(|source| std::ptr::eq(source, destination))
            {
                return Err(scoped_memory_error_v29());
            }
            Ok(())
        })?;
        let role = if result {
            ScopedMemoryRoleV29::CallResult
        } else {
            ScopedMemoryRoleV29::Operand(ExecutionOperandV29::CallDestinationAddress)
        };
        self.with_scoped_memory_frame_v29(
            ScopedMemoryFrameV29 {
                site,
                role: Some(role),
            },
            body,
        )
    }

    fn consume_scoped_discarded_operand_v29(
        &mut self,
        block: SemanticBlockIdV1,
        role: ExecutionOperandV29,
        operand: &SemanticOperandV1,
        position: usize,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        if self.scoped_memory.is_none() {
            return Ok(());
        }
        let site = execution_site_v29(block, None);
        let invalidation = self.with_emission_budget_v1(|this, budget| {
            budget.charge_work(argument_sum_v1(&[
                8,
                scoped_initialization_search_work_v29(this.retained_local_slots.len()),
                scoped_initialization_search_work_v29(this.retained_local_initialized.len()),
            ])?)?;
            let cursor = this
                .execution
                .as_ref()
                .ok_or_else(scoped_memory_error_v29)?;
            cursor.check_ledger(budget)?;
            if !matches!(
                role,
                ExecutionOperandV29::AssertCondition | ExecutionOperandV29::AssertMessage(_)
            ) || !scoped_source_operand_v29(this.function, site, role)
                .is_some_and(|source| std::ptr::eq(source, operand))
            {
                return Err(scoped_memory_error_v29());
            }
            let SemanticOperandV1::Move(place) = operand else {
                return Ok(None);
            };
            let Some(slot) = this.retained_local_slots.get(&place.local().index()) else {
                return Ok(None);
            };
            if !retained_move_invalidates_local_v1(place, slot.array.is_some()) {
                return Ok(None);
            }
            let cause = if place.projections().is_empty() {
                ScopedMemoryKillV29::Move
            } else {
                ScopedMemoryKillV29::ProjectedArrayMove
            };
            Ok(Some((place.local(), cause)))
        })?;
        let Some((local, cause)) = invalidation else {
            return Ok(());
        };
        let index = self.require_local(block, None, local.index())?;
        self.with_scoped_memory_frame_v29(ScopedMemoryFrameV29::operand(site, Some(role)), |this| {
            // Discarding a diagnostic preserves its move effect, not a physical read.
            this.record_scoped_memory_kill_v29(local, cause, position)?;
            this.locals[index] = None;
            this.retained_local_initialized.remove(&local.index());
            Ok(())
        })
    }

    fn with_scoped_memory_frame_v29<T>(
        &mut self,
        frame: ScopedMemoryFrameV29,
        body: impl FnOnce(&mut Self) -> Result<T, ProductionSemanticKirErrorV1>,
    ) -> Result<T, ProductionSemanticKirErrorV1> {
        if self.scoped_memory.is_none() {
            return body(self);
        }
        self.with_emission_budget_v1(|_, budget| budget.charge_work(8))?;
        let recorder = self
            .scoped_memory
            .as_mut()
            .ok_or_else(scoped_memory_error_v29)?;
        let previous = recorder.frame.replace(frame);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| body(self)));
        self.scoped_memory
            .as_mut()
            .ok_or_else(scoped_memory_error_v29)?
            .frame = previous;
        match result {
            Ok(result) => result,
            Err(payload) => std::panic::resume_unwind(payload),
        }
    }

    fn record_scoped_memory_access_v29(
        &mut self,
        position: usize,
        kind: &OperationKind,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        let Some(pointer) = scoped_memory_pointer_v29(kind) else {
            return Ok(());
        };
        if self.scoped_memory.is_none() {
            return Ok(());
        }
        self.with_emission_budget_v1(|this, budget| {
            let recorder = this
                .scoped_memory
                .as_mut()
                .ok_or_else(scoped_memory_error_v29)?;
            if recorder.anchors.subject.ledger != budget.work_ledger_identity_v1() {
                return Err(ArgumentResourceV1::Accounting.into());
            }
            let row = ScopedMemoryAnchorV29 {
                block: recorder.block.ok_or_else(scoped_memory_error_v29)?,
                position,
                source: recorder.frame,
                kind: ScopedMemoryAnchorKindV29::Access { pointer },
            };
            emission_push_v1(&mut recorder.anchors.rows, row, budget)
        })
    }

    fn record_scoped_memory_kill_v29(
        &mut self,
        local: SemanticLocalIdV1,
        cause: ScopedMemoryKillV29,
        position: usize,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        if self.scoped_memory.is_none() {
            return Ok(());
        }
        self.with_emission_budget_v1(|this, budget| {
            budget.charge_work(scoped_initialization_search_work_v29(
                this.retained_local_slots.len(),
            ))?;
            let Some(slot) = this.retained_local_slots.get(&local.index()) else {
                return Ok(());
            };
            let recorder = this
                .scoped_memory
                .as_mut()
                .ok_or_else(scoped_memory_error_v29)?;
            let frame = recorder.frame.ok_or_else(scoped_memory_error_v29)?;
            let Some(ScopedMemoryRoleV29::Operand(role)) = frame.role else {
                return Err(scoped_memory_error_v29());
            };
            let cursor = this
                .execution
                .as_ref()
                .ok_or_else(scoped_memory_error_v29)?;
            cursor.check_ledger(budget)?;
            let key = unit_local_source_key_v1(frame.site, role, Some(cause.event_role()));
            budget.charge_work(argument_product_v1(
                8,
                scoped_initialization_search_work_v29(cursor.index.len()),
            )?)?;
            let index = cursor
                .index
                .binary_search_by_key(&key, |row| row.key)
                .map_err(|_| scoped_memory_error_v29())?;
            let event_index = cursor.index[index].index;
            let event = &cursor.occurrences.events()[event_index];
            if scoped_expected_kill_v29(cursor.function, event, slot.array.is_some())
                != Some((local.index(), cause))
            {
                return Err(scoped_memory_error_v29());
            }
            let row = ScopedMemoryAnchorV29 {
                block: recorder.block.ok_or_else(scoped_memory_error_v29)?,
                position,
                source: Some(frame),
                kind: ScopedMemoryAnchorKindV29::Kill {
                    event: event_index,
                    local: local.index(),
                    cause,
                },
            };
            emission_push_v1(&mut recorder.anchors.rows, row, budget)
        })
    }
}
