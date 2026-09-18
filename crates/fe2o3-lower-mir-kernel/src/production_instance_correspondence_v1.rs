// Live occurrence transport around the existing splice, not source replay or
// scope authority. All owned rows use the caller's cumulative logical ledger.

use production_call_instances_v1::{ProductionCallInstancePlanV1, ProductionInstanceCallV1};

#[derive(Debug, Eq, PartialEq)]
enum InstanceCorrespondenceErrorV1 {
    Resource(ArgumentResourceV1),
    Emission(CallInstanceEmissionErrorV1),
    Source,
    DuplicateInstance,
    SpanCoverage,
    CallAnchor,
    Control,
}

impl std::fmt::Display for InstanceCorrespondenceErrorV1 {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "instance occurrence transport refused: {self:?}")
    }
}

impl std::error::Error for InstanceCorrespondenceErrorV1 {}

impl From<ArgumentResourceV1> for InstanceCorrespondenceErrorV1 {
    fn from(error: ArgumentResourceV1) -> Self {
        Self::Resource(error)
    }
}

impl From<CallInstanceEmissionErrorV1> for InstanceCorrespondenceErrorV1 {
    fn from(error: CallInstanceEmissionErrorV1) -> Self {
        Self::Emission(error)
    }
}

fn instance_anchor_error_v1(error: ProductionSemanticKirErrorV1) -> InstanceCorrespondenceErrorV1 {
    match error {
        ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(error) => {
            InstanceCorrespondenceErrorV1::Resource(error)
        }
        _ => InstanceCorrespondenceErrorV1::CallAnchor,
    }
}

type InstanceMapResultV1<T> = Result<T, InstanceCorrespondenceErrorV1>;

include!("production_instance_correspondence_source_v1.rs");

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct InstancePhysicalSpanV1 {
    block: BlockId,
    first: u32,
    count: u32,
}

impl InstancePhysicalSpanV1 {
    fn end(self) -> InstanceMapResultV1<u32> {
        self.first
            .checked_add(self.count)
            .ok_or(ArgumentResourceV1::Arithmetic.into())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum InstanceSpanSourceV1 {
    Statement(SemanticKirStatementOperationSpanV1),
    Terminator(SemanticKirTerminatorOperationSpanV1),
    Synthetic(SemanticKirSyntheticOperationSpanV1),
}

impl InstanceSpanSourceV1 {
    fn coordinates(
        self,
    ) -> (
        SemanticFunctionIdV1,
        SemanticFunctionIdV1,
        InstancePhysicalSpanV1,
    ) {
        match self {
            Self::Statement(row) => (
                row.correspondence_owner,
                row.semantic_function,
                InstancePhysicalSpanV1 {
                    block: row.kernel_ir_block,
                    first: row.first_operation_ordinal,
                    count: row.operation_count,
                },
            ),
            Self::Terminator(row) => (
                row.correspondence_owner,
                row.semantic_function,
                InstancePhysicalSpanV1 {
                    block: row.kernel_ir_block,
                    first: row.first_operation_ordinal,
                    count: row.operation_count,
                },
            ),
            Self::Synthetic(row) => (
                row.correspondence_owner,
                row.semantic_function,
                InstancePhysicalSpanV1 {
                    block: row.kernel_ir_block,
                    first: row.first_operation_ordinal,
                    count: row.operation_count,
                },
            ),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct InstanceMappedSpanV1 {
    instance: ProductionCallInstanceIdV1,
    source: InstanceSpanSourceV1,
    // One source block has at most one defined-call terminator. Its one
    // removed Call can split an original span into at most two ordered pieces.
    segments: [Option<InstancePhysicalSpanV1>; 2],
    removed_call: Option<ProductionCallOccurrenceV1>,
}

impl InstanceMappedSpanV1 {
    fn after_splice(
        self,
        call: ProductionCallOccurrenceV1,
        split: CallInstanceSplitV1,
    ) -> InstanceMapResultV1<Self> {
        let ordinal = u32::try_from(split.call.operation_index)
            .map_err(|_| ArgumentResourceV1::Arithmetic)?;
        let after = ordinal
            .checked_add(1)
            .ok_or(ArgumentResourceV1::Arithmetic)?;
        let mut result = self;
        let mut segments = [None, None];
        let mut count = 0;
        let mut removed = false;
        for span in self.segments.into_iter().flatten() {
            let end = span.end()?;
            let mut emit = |span| -> InstanceMapResultV1<()> {
                let slot = segments
                    .get_mut(count)
                    .ok_or(InstanceCorrespondenceErrorV1::SpanCoverage)?;
                *slot = Some(span);
                count += 1;
                Ok(())
            };
            if span.block != split.call.block || end <= ordinal {
                emit(span)?;
            } else if span.first >= after {
                emit(InstancePhysicalSpanV1 {
                    block: split.continuation,
                    first: span.first - after,
                    count: span.count,
                })?;
            } else if span.count == 0 {
                emit(span)?;
            } else {
                if self.instance != call.caller || self.removed_call.is_some() {
                    return Err(InstanceCorrespondenceErrorV1::SpanCoverage);
                }
                removed = true;
                if span.first < ordinal {
                    emit(InstancePhysicalSpanV1 {
                        count: ordinal - span.first,
                        ..span
                    })?;
                }
                if end > after {
                    emit(InstancePhysicalSpanV1 {
                        block: split.continuation,
                        first: 0,
                        count: end - after,
                    })?;
                }
            }
        }
        result.segments = segments;
        if removed {
            result.removed_call = Some(call);
        }
        Ok(result)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum InstanceControlOriginV1 {
    Retained,
    CallEntry {
        call: ProductionCallOccurrenceV1,
        child: ProductionCallInstanceIdV1,
    },
    ParameterPreheader {
        call: ProductionCallOccurrenceV1,
        child: ProductionCallInstanceIdV1,
    },
    ExpandedReturn {
        call: ProductionCallOccurrenceV1,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct InstanceControlV1 {
    instance: ProductionCallInstanceIdV1,
    original_block: BlockId,
    semantic_block: Option<SemanticBlockIdV1>,
    physical_block: BlockId,
    origin: InstanceControlOriginV1,
    return_values: Option<std::ops::Range<usize>>,
    expected_branch: Option<(BlockId, std::ops::Range<usize>)>,
}

struct InstanceSeedV1 {
    instance: ProductionCallInstanceIdV1,
    container: ProductionCallInstanceIdV1,
    function_name: String,
    parameters: std::ops::Range<usize>,
}

struct InstanceCallAnchorV1 {
    instance: ProductionCallInstanceIdV1,
    source: SemanticKirCallReturnV1,
    physical: FunctionOperationLocation,
    arguments: std::ops::Range<usize>,
    results: std::ops::Range<usize>,
    removed: bool,
}

struct InstanceReturnAnchorV1 {
    instance: ProductionCallInstanceIdV1,
    source: SemanticKirCallReturnV1,
    original_block: BlockId,
}

// Capacity is logical, not allocator/RSS telemetry. Relocation work and the
// coexistence of old/new allocations are charged before reserve.
struct InstanceRowsV1<T> {
    rows: Vec<T>,
    capacity: usize,
}

impl<T> InstanceRowsV1<T> {
    fn new() -> Self {
        Self {
            rows: Vec::new(),
            capacity: 0,
        }
    }
    fn reserve(
        &mut self,
        additional: usize,
        budget: &mut ArgumentBudgetV1<'_>,
        storage: &mut usize,
    ) -> InstanceMapResultV1<()> {
        let needed = self
            .rows
            .len()
            .checked_add(additional)
            .ok_or(ArgumentResourceV1::Arithmetic)?;
        if needed <= self.capacity {
            return Ok(());
        }
        let capacity = needed.max(
            self.capacity
                .max(2)
                .checked_mul(2)
                .ok_or(ArgumentResourceV1::Arithmetic)?,
        );
        let bytes = capacity
            .checked_mul(std::mem::size_of::<T>())
            .ok_or(ArgumentResourceV1::Arithmetic)?;
        let old = self
            .capacity
            .checked_mul(std::mem::size_of::<T>())
            .ok_or(ArgumentResourceV1::Arithmetic)?;
        let next = storage
            .checked_add(bytes)
            .ok_or(ArgumentResourceV1::Arithmetic)?;
        budget.charge_work(self.rows.len())?;
        budget.reserve_storage(bytes)?;
        *storage = next;
        self.rows
            .try_reserve_exact(capacity - self.rows.len())
            .map_err(|_| ArgumentResourceV1::Allocation)?;
        budget.release_storage(old)?;
        *storage -= old;
        self.capacity = capacity;
        Ok(())
    }
}

struct ProductionInstanceCorrespondenceV1<'p, 's> {
    plan: &'p ProductionCallInstancePlanV1<'s>,
    owner: Option<SemanticFunctionIdV1>,
    seeds: InstanceRowsV1<InstanceSeedV1>,
    spans: InstanceRowsV1<InstanceMappedSpanV1>,
    controls: InstanceRowsV1<InstanceControlV1>,
    anchors: InstanceRowsV1<InstanceCallAnchorV1>,
    returns: InstanceRowsV1<InstanceReturnAnchorV1>,
    components: InstanceRowsV1<CallResultComponentV1>,
    values: InstanceRowsV1<ValueId>,
    storage: usize,
    failed: bool,
}

fn with_production_instance_correspondence_v1<'p, 's, 'work, R, E>(
    plan: &'p ProductionCallInstancePlanV1<'s>,
    budget: &mut ArgumentBudgetV1<'work>,
    consume: impl FnOnce(
        &mut ProductionInstanceCorrespondenceV1<'p, 's>,
        &mut ArgumentBudgetV1<'work>,
    ) -> Result<R, E>,
) -> Result<R, E>
where
    E: From<InstanceCorrespondenceErrorV1>,
{
    let mut map = ProductionInstanceCorrespondenceV1 {
        plan,
        owner: None,
        seeds: InstanceRowsV1::new(),
        spans: InstanceRowsV1::new(),
        controls: InstanceRowsV1::new(),
        anchors: InstanceRowsV1::new(),
        returns: InstanceRowsV1::new(),
        components: InstanceRowsV1::new(),
        values: InstanceRowsV1::new(),
        storage: 0,
        failed: false,
    };
    let result = consume(&mut map, budget);
    let storage = map.storage;
    drop(map);
    budget
        .release_storage(storage)
        .map_err(InstanceCorrespondenceErrorV1::from)?;
    result
}

impl ProductionInstanceCorrespondenceV1<'_, '_> {
    fn spans(&self) -> &[InstanceMappedSpanV1] {
        &self.spans.rows
    }
    fn controls(&self) -> &[InstanceControlV1] {
        &self.controls.rows
    }

    fn seed_index(
        &self,
        instance: ProductionCallInstanceIdV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> InstanceMapResultV1<usize> {
        budget.charge_work(self.seeds.rows.len())?;
        self.seeds
            .rows
            .iter()
            .position(|row| row.instance == instance)
            .ok_or(InstanceCorrespondenceErrorV1::Source)
    }

    fn append_values(
        &mut self,
        values: &[ValueId],
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> InstanceMapResultV1<std::ops::Range<usize>> {
        budget.charge_work(values.len())?;
        self.values
            .reserve(values.len(), budget, &mut self.storage)?;
        let first = self.values.rows.len();
        self.values.rows.extend_from_slice(values);
        Ok(first..self.values.rows.len())
    }

    fn append_lowered(
        &mut self,
        instance: ProductionCallInstanceIdV1,
        lowered: &LoweredFunctionResultV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> InstanceMapResultV1<()> {
        if self.failed {
            return Err(InstanceCorrespondenceErrorV1::Source);
        }
        let result = self.append_lowered_inner(instance, lowered, budget);
        self.failed = result.is_err();
        result
    }

    fn append_lowered_inner(
        &mut self,
        instance: ProductionCallInstanceIdV1,
        lowered: &LoweredFunctionResultV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> InstanceMapResultV1<()> {
        budget.charge_work(1)?;
        if lowered
            .source_call_instance
            .is_some_and(|selected| selected != instance)
        {
            return Err(InstanceCorrespondenceErrorV1::Source);
        }
        let plan = self.plan;
        let source = plan
            .instance(instance)
            .ok_or(InstanceCorrespondenceErrorV1::Source)?;
        budget.charge_work(self.seeds.rows.len())?;
        if self.seeds.rows.iter().any(|row| row.instance == instance) {
            return Err(InstanceCorrespondenceErrorV1::DuplicateInstance);
        }
        if self.seeds.rows.is_empty() && instance != self.plan.root() {
            return Err(InstanceCorrespondenceErrorV1::Source);
        }
        let body = lowered
            .function
            .body
            .as_ref()
            .ok_or(InstanceCorrespondenceErrorV1::Source)?;
        let owner = plan
            .instance(plan.root())
            .ok_or(InstanceCorrespondenceErrorV1::Source)?
            .function();
        if self.owner.is_some_and(|expected| expected != owner) {
            return Err(InstanceCorrespondenceErrorV1::Source);
        }
        instance_check_source_rows_v1(source, owner, lowered, budget)?;
        let span_count = lowered
            .statement_operation_spans
            .len()
            .checked_add(lowered.terminator_operation_spans.len())
            .and_then(|n| n.checked_add(lowered.synthetic_operation_spans.len()))
            .ok_or(ArgumentResourceV1::Arithmetic)?;
        budget.charge_work(span_count)?;
        self.spans.reserve(span_count, budget, &mut self.storage)?;
        let first_span = self.spans.rows.len();
        for origin in lowered
            .statement_operation_spans
            .iter()
            .copied()
            .map(InstanceSpanSourceV1::Statement)
            .chain(
                lowered
                    .terminator_operation_spans
                    .iter()
                    .copied()
                    .map(InstanceSpanSourceV1::Terminator),
            )
            .chain(
                lowered
                    .synthetic_operation_spans
                    .iter()
                    .copied()
                    .map(InstanceSpanSourceV1::Synthetic),
            )
        {
            let (row_owner, function, span) = origin.coordinates();
            if row_owner != owner || function != source.function() {
                return Err(InstanceCorrespondenceErrorV1::Source);
            }
            span.end()?;
            self.spans.rows.push(InstanceMappedSpanV1 {
                instance,
                source: origin,
                segments: [Some(span), None],
                removed_call: None,
            });
        }
        call_splice_sort_work_v1(span_count, budget)?;
        self.spans.rows[first_span..].sort_unstable_by_key(|row| {
            let span = row.source.coordinates().2;
            (span.block, span.first, span.count)
        });
        budget.charge_work(body.blocks.len())?;
        self.controls
            .reserve(body.blocks.len(), budget, &mut self.storage)?;
        for block in &body.blocks {
            budget.charge_work(self.controls.rows.len())?;
            if self
                .controls
                .rows
                .iter()
                .any(|row| row.physical_block == block.id)
            {
                return Err(InstanceCorrespondenceErrorV1::Control);
            }
            budget.charge_work(span_count)?;
            let mut cursor = 0;
            for span in self.spans.rows[first_span..]
                .iter()
                .map(|row| row.source.coordinates().2)
                .filter(|span| span.block == block.id)
            {
                if span.first != cursor {
                    return Err(InstanceCorrespondenceErrorV1::SpanCoverage);
                }
                cursor = span.end()?;
            }
            if cursor as usize != block.operations.len() {
                return Err(InstanceCorrespondenceErrorV1::SpanCoverage);
            }
            budget.charge_work(lowered.blocks.len())?;
            let mut origins = lowered
                .blocks
                .iter()
                .filter(|row| row.kernel_ir_block == block.id);
            let semantic_block = origins
                .next()
                .map(|row| {
                    if row.semantic_function != source.function()
                        || row.correspondence_owner != owner
                    {
                        return Err(InstanceCorrespondenceErrorV1::Source);
                    }
                    Ok(row.semantic_block)
                })
                .transpose()?;
            if origins.next().is_some() {
                return Err(InstanceCorrespondenceErrorV1::Source);
            }
            let return_values = match block
                .terminator
                .as_ref()
                .ok_or(InstanceCorrespondenceErrorV1::Control)?
            {
                Terminator::Return { values } => Some(self.append_values(values, budget)?),
                _ => None,
            };
            self.controls.rows.push(InstanceControlV1 {
                instance,
                original_block: block.id,
                semantic_block,
                physical_block: block.id,
                origin: InstanceControlOriginV1::Retained,
                return_values,
                expected_branch: None,
            });
        }
        budget.charge_work(
            span_count
                .checked_mul(body.blocks.len().max(1))
                .ok_or(ArgumentResourceV1::Arithmetic)?,
        )?;
        if self.spans.rows[first_span..].iter().any(|row| {
            !body
                .blocks
                .iter()
                .any(|block| block.id == row.source.coordinates().2.block)
        }) {
            return Err(InstanceCorrespondenceErrorV1::SpanCoverage);
        }
        validate_call_component_pool_v1(
            &lowered.call_returns.sites.rows,
            &lowered.call_returns.components.rows,
            budget,
        )
        .map_err(instance_anchor_error_v1)?;
        let component_offset = u32::try_from(self.components.rows.len())
            .map_err(|_| ArgumentResourceV1::Arithmetic)?;
        budget.charge_work(lowered.call_returns.components.rows.len())?;
        self.components.reserve(
            lowered.call_returns.components.rows.len(),
            budget,
            &mut self.storage,
        )?;
        self.components
            .rows
            .extend_from_slice(&lowered.call_returns.components.rows);
        budget.charge_work(lowered.call_returns.sites.rows.len())?;
        self.anchors.reserve(
            lowered.call_returns.sites.rows.len(),
            budget,
            &mut self.storage,
        )?;
        self.returns.reserve(
            lowered.call_returns.sites.rows.len(),
            budget,
            &mut self.storage,
        )?;
        budget.charge_work(lowered.call_returns.sites.rows.len())?;
        if lowered
            .call_returns
            .sites
            .rows
            .windows(2)
            .any(|rows| rows[0].semantic_block >= rows[1].semantic_block)
        {
            return Err(InstanceCorrespondenceErrorV1::CallAnchor);
        }
        for source_anchor in &lowered.call_returns.sites.rows {
            if source_anchor.correspondence_owner != owner
                || source_anchor.semantic_function != source.function()
            {
                return Err(InstanceCorrespondenceErrorV1::CallAnchor);
            }
            let mut anchor = *source_anchor;
            anchor
                .rebase_components(component_offset)
                .map_err(instance_anchor_error_v1)?;
            if anchor
                .components()
                .range()
                .map_err(instance_anchor_error_v1)?
                .end
                > self.components.rows.len()
            {
                return Err(InstanceCorrespondenceErrorV1::CallAnchor);
            }
            budget.charge_work(lowered.blocks.len())?;
            let block_id = lowered
                .blocks
                .iter()
                .find(|row| row.semantic_block == anchor.semantic_block)
                .ok_or(InstanceCorrespondenceErrorV1::CallAnchor)?
                .kernel_ir_block;
            budget.charge_work(body.blocks.len())?;
            let block = body
                .blocks
                .iter()
                .find(|row| row.id == block_id)
                .ok_or(InstanceCorrespondenceErrorV1::CallAnchor)?;
            let semantic_block = source
                .declaration()
                .blocks()
                .get(anchor.semantic_block.index() as usize)
                .ok_or(InstanceCorrespondenceErrorV1::CallAnchor)?;
            let SemanticKirCallReturnKindV1::Call {
                arguments_first,
                call_operation,
                destination_end,
                ..
            } = anchor.kind
            else {
                if !matches!(
                    semantic_block.terminator().kind(),
                    SemanticTerminatorKindV1::Return
                ) || !matches!(block.terminator, Some(Terminator::Return { .. }))
                {
                    return Err(InstanceCorrespondenceErrorV1::CallAnchor);
                }
                self.returns.rows.push(InstanceReturnAnchorV1 {
                    instance,
                    source: anchor,
                    original_block: block_id,
                });
                continue;
            };
            if !matches!(
                semantic_block.terminator().kind(),
                SemanticTerminatorKindV1::Call(_)
            ) {
                return Err(InstanceCorrespondenceErrorV1::CallAnchor);
            }
            budget.charge_work(lowered.terminator_operation_spans.len())?;
            let mut spans = lowered
                .terminator_operation_spans
                .iter()
                .filter(|span| span.semantic_block == anchor.semantic_block);
            let span = spans
                .next()
                .ok_or(InstanceCorrespondenceErrorV1::CallAnchor)?;
            let span_end = span
                .first_operation_ordinal
                .checked_add(span.operation_count)
                .ok_or(ArgumentResourceV1::Arithmetic)?;
            if spans.next().is_some()
                || span.kernel_ir_block != block_id
                || arguments_first < span.first_operation_ordinal
                || call_operation < arguments_first
                || destination_end <= call_operation
                || destination_end > span_end
            {
                return Err(InstanceCorrespondenceErrorV1::CallAnchor);
            }
            let operation = block
                .operations
                .get(call_operation as usize)
                .ok_or(InstanceCorrespondenceErrorV1::CallAnchor)?;
            let OperationKind::Call { arguments, .. } = &operation.kind else {
                return Err(InstanceCorrespondenceErrorV1::CallAnchor);
            };
            let arguments = self.append_values(arguments, budget)?;
            budget.charge_work(operation.results.len())?;
            self.values
                .reserve(operation.results.len(), budget, &mut self.storage)?;
            let first = self.values.rows.len();
            self.values
                .rows
                .extend(operation.results.iter().map(|value| value.id));
            let results = first..self.values.rows.len();
            self.anchors.rows.push(InstanceCallAnchorV1 {
                instance,
                source: anchor,
                physical: FunctionOperationLocation::new(block_id, call_operation as usize),
                arguments,
                results,
                removed: false,
            });
        }
        let parameters = self.append_values(&body.parameters, budget)?;
        let name = lowered.function.id.as_str();
        budget.charge_work(name.len())?;
        let next = self
            .storage
            .checked_add(name.len())
            .ok_or(ArgumentResourceV1::Arithmetic)?;
        budget.reserve_storage(name.len())?;
        self.storage = next;
        let mut function_name = String::new();
        function_name
            .try_reserve_exact(name.len())
            .map_err(|_| ArgumentResourceV1::Allocation)?;
        function_name.push_str(name);
        self.seeds.reserve(1, budget, &mut self.storage)?;
        self.seeds.rows.push(InstanceSeedV1 {
            instance,
            container: instance,
            function_name,
            parameters,
        });
        self.owner = Some(owner);
        Ok(())
    }

    fn splice(
        &mut self,
        call: &ProductionInstanceCallV1<'_>,
        caller: Function,
        callee: Function,
        entry: BlockId,
        continuation: BlockId,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> InstanceMapResultV1<SplicedCallInstanceV1> {
        if self.failed {
            return Err(InstanceCorrespondenceErrorV1::Source);
        }
        let result = self.splice_inner(call, caller, callee, entry, continuation, budget);
        self.failed = result.is_err();
        result
    }

    fn splice_inner(
        &mut self,
        call: &ProductionInstanceCallV1<'_>,
        caller: Function,
        callee: Function,
        entry: BlockId,
        continuation: BlockId,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> InstanceMapResultV1<SplicedCallInstanceV1> {
        let occurrence = call.occurrence();
        let calls = self
            .plan
            .calls(occurrence.caller)
            .ok_or(InstanceCorrespondenceErrorV1::Source)?;
        budget.charge_work(calls.len())?;
        if !calls.iter().any(|row| std::ptr::eq(row, call)) {
            return Err(InstanceCorrespondenceErrorV1::Source);
        }
        let child = call.child().ok_or(InstanceCorrespondenceErrorV1::Source)?;
        let caller_seed = self.seed_index(occurrence.caller, budget)?;
        let child_seed = self.seed_index(child, budget)?;
        let container = self.seeds.rows[caller_seed].container;
        let container_seed = self.seed_index(container, budget)?;
        if self.seeds.rows[child_seed].container != child {
            return Err(InstanceCorrespondenceErrorV1::CallAnchor);
        }
        budget.charge_work(
            caller
                .id
                .as_str()
                .len()
                .checked_add(callee.id.as_str().len())
                .ok_or(ArgumentResourceV1::Arithmetic)?,
        )?;
        if caller.id.as_str() != self.seeds.rows[container_seed].function_name
            || callee.id.as_str() != self.seeds.rows[child_seed].function_name
        {
            return Err(InstanceCorrespondenceErrorV1::CallAnchor);
        }
        budget.charge_work(self.anchors.rows.len())?;
        let mut matching = self.anchors.rows.iter().enumerate().filter(|(_, row)| {
            row.instance == occurrence.caller && row.source.semantic_block == occurrence.block
        });
        let anchor_index = matching
            .next()
            .map(|(index, _)| index)
            .ok_or(InstanceCorrespondenceErrorV1::CallAnchor)?;
        if matching.next().is_some() || self.anchors.rows[anchor_index].removed {
            return Err(InstanceCorrespondenceErrorV1::CallAnchor);
        }
        let site = self.anchors.rows[anchor_index].physical;
        budget.charge_work(self.controls.rows.len())?;
        if self
            .controls
            .rows
            .iter()
            .any(|row| row.physical_block == entry || row.physical_block == continuation)
        {
            return Err(InstanceCorrespondenceErrorV1::Control);
        }
        budget.charge_work(self.controls.rows.len())?;
        let caller_control = self
            .controls
            .rows
            .iter()
            .position(|row| row.physical_block == site.block)
            .ok_or(InstanceCorrespondenceErrorV1::Control)?;
        if self.controls.rows[caller_control].instance != occurrence.caller
            || self.controls.rows[caller_control].semantic_block != Some(occurrence.block)
            || self.controls.rows[caller_control].origin != InstanceControlOriginV1::Retained
        {
            return Err(InstanceCorrespondenceErrorV1::Control);
        }
        let callee_body = callee
            .body
            .as_ref()
            .ok_or(InstanceCorrespondenceErrorV1::Control)?;
        let callee_entry = callee_body
            .blocks
            .first()
            .ok_or(InstanceCorrespondenceErrorV1::Control)?
            .id;
        let split = CallInstanceSplitV1 {
            call: site,
            entry,
            callee_entry,
            continuation,
            callee_blocks: callee_body.blocks.len(),
            returns: 0,
            result_components: self.anchors.rows[anchor_index].results.len(),
        };
        budget.charge_work(self.spans.rows.len())?;
        let mut removed = 0_usize;
        for row in &self.spans.rows {
            let mapped = row.after_splice(occurrence, split)?;
            removed += usize::from(mapped.removed_call != row.removed_call);
        }
        if removed != 1 {
            return Err(InstanceCorrespondenceErrorV1::SpanCoverage);
        }
        self.controls.reserve(2, budget, &mut self.storage)?;
        let result =
            splice_production_call_instance_v1(caller, callee, site, entry, continuation, budget)?;
        let checked =
            self.check_new_edges(occurrence, child, anchor_index, child_seed, &result, budget);
        if let Err(error) = checked {
            let retained = result.additional_storage_bytes;
            drop(result);
            budget.release_storage(retained)?;
            return Err(error);
        }
        // The preflight above made all row rewrites infallible. Pay this second
        // traversal separately, preserving first-failure work history.
        let updates = self
            .spans
            .rows
            .len()
            .checked_add(self.controls.rows.len())
            .and_then(|n| n.checked_add(self.seeds.rows.len()))
            .ok_or(ArgumentResourceV1::Arithmetic);
        let update_work = updates.and_then(|n| budget.charge_work(n));
        if let Err(error) = update_work {
            let retained = result.additional_storage_bytes;
            drop(result);
            budget.release_storage(retained)?;
            return Err(error.into());
        }
        for row in &mut self.spans.rows {
            match row.after_splice(occurrence, result.split) {
                Ok(mapped) => *row = mapped,
                Err(error) => {
                    let retained = result.additional_storage_bytes;
                    drop(result);
                    budget.release_storage(retained)?;
                    return Err(error);
                }
            }
        }
        self.controls.rows[caller_control].physical_block = continuation;
        for row in &mut self.controls.rows {
            if row.instance == child
                && row.origin == InstanceControlOriginV1::Retained
                && let Some(values) = &row.return_values
            {
                row.origin = InstanceControlOriginV1::ExpandedReturn { call: occurrence };
                row.expected_branch = Some((continuation, values.clone()));
            }
        }
        self.controls.rows.push(InstanceControlV1 {
            instance: occurrence.caller,
            original_block: site.block,
            semantic_block: Some(occurrence.block),
            physical_block: site.block,
            origin: InstanceControlOriginV1::CallEntry {
                call: occurrence,
                child,
            },
            return_values: None,
            expected_branch: Some((entry, self.anchors.rows[anchor_index].arguments.clone())),
        });
        self.controls.rows.push(InstanceControlV1 {
            instance: child,
            original_block: callee_entry,
            semantic_block: None,
            physical_block: entry,
            origin: InstanceControlOriginV1::ParameterPreheader {
                call: occurrence,
                child,
            },
            return_values: None,
            expected_branch: Some((callee_entry, 0..0)),
        });
        for seed in &mut self.seeds.rows {
            if seed.container == child {
                seed.container = container;
            }
        }
        self.anchors.rows[anchor_index].removed = true;
        if let Err(error) = self.check_coordinates(container, &result.caller, budget) {
            let retained = result.additional_storage_bytes;
            drop(result);
            budget.release_storage(retained)?;
            return Err(error);
        }
        Ok(result)
    }

    // This checks exact generated edges/parameter IDs, not operation payloads
    // or the semantic meaning of independently supplied source spans.
    fn check_new_edges(
        &self,
        call: ProductionCallOccurrenceV1,
        child: ProductionCallInstanceIdV1,
        anchor_index: usize,
        child_seed: usize,
        result: &SplicedCallInstanceV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> InstanceMapResultV1<()> {
        let body = result
            .caller
            .body
            .as_ref()
            .ok_or(InstanceCorrespondenceErrorV1::Control)?;
        let find = |id, budget: &mut ArgumentBudgetV1<'_>| -> InstanceMapResultV1<&BasicBlock> {
            budget.charge_work(body.blocks.len())?;
            let mut rows = body.blocks.iter().filter(|block| block.id == id);
            let block = rows.next().ok_or(InstanceCorrespondenceErrorV1::Control)?;
            if rows.next().is_some() {
                return Err(InstanceCorrespondenceErrorV1::Control);
            }
            Ok(block)
        };
        let anchor = &self.anchors.rows[anchor_index];
        let prefix = find(result.split.call.block, budget)?;
        let entry = find(result.split.entry, budget)?;
        let continuation = find(result.split.continuation, budget)?;
        instance_check_branch_v1(
            prefix,
            result.split.entry,
            &self.values.rows[anchor.arguments.clone()],
            budget,
        )?;
        instance_check_branch_v1(entry, result.split.callee_entry, &[], budget)?;
        instance_check_parameters_v1(
            entry,
            &self.values.rows[self.seeds.rows[child_seed].parameters.clone()],
            budget,
        )?;
        instance_check_parameters_v1(
            continuation,
            &self.values.rows[anchor.results.clone()],
            budget,
        )?;
        budget.charge_work(self.controls.rows.len())?;
        let mut returns = 0;
        for row in &self.controls.rows {
            if row.instance == child
                && row.origin == InstanceControlOriginV1::Retained
                && let Some(values) = &row.return_values
            {
                budget.charge_work(self.returns.rows.len())?;
                let mut anchors = self.returns.rows.iter().filter(|anchor| {
                    anchor.instance == child && anchor.original_block == row.original_block
                });
                let return_anchor = anchors
                    .next()
                    .ok_or(InstanceCorrespondenceErrorV1::CallAnchor)?;
                if anchors.next().is_some()
                    || row.semantic_block != Some(return_anchor.source.semantic_block)
                {
                    return Err(InstanceCorrespondenceErrorV1::CallAnchor);
                }
                instance_check_branch_v1(
                    find(row.physical_block, budget)?,
                    result.split.continuation,
                    &self.values.rows[values.clone()],
                    budget,
                )?;
                returns += 1;
            }
        }
        if returns != result.split.returns || anchor.source.semantic_block != call.block {
            return Err(InstanceCorrespondenceErrorV1::Control);
        }
        Ok(())
    }

    // Bounds and generated control transport only. Retained operation payloads,
    // original terminators and their source meaning still need lowering replay.
    fn check_coordinates(
        &self,
        container: ProductionCallInstanceIdV1,
        function: &Function,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> InstanceMapResultV1<()> {
        if self.failed {
            return Err(InstanceCorrespondenceErrorV1::Source);
        }
        let seed = self.seed_index(container, budget)?;
        budget.charge_work(function.id.as_str().len())?;
        if self.seeds.rows[seed].container != container
            || function.id.as_str() != self.seeds.rows[seed].function_name
        {
            return Err(InstanceCorrespondenceErrorV1::Source);
        }
        let body = function
            .body
            .as_ref()
            .ok_or(InstanceCorrespondenceErrorV1::Control)?;
        budget.charge_work(self.controls.rows.len())?;
        let mut controls = 0;
        for row in &self.controls.rows {
            let seed = self.seed_index(row.instance, budget)?;
            if self.seeds.rows[seed].container != container {
                continue;
            }
            budget.charge_work(body.blocks.len())?;
            let mut matching = body
                .blocks
                .iter()
                .filter(|block| block.id == row.physical_block);
            let block = matching
                .next()
                .ok_or(InstanceCorrespondenceErrorV1::Control)?;
            if matching.next().is_some() {
                return Err(InstanceCorrespondenceErrorV1::Control);
            }
            if let Some((target, arguments)) = &row.expected_branch {
                instance_check_branch_v1(
                    block,
                    *target,
                    &self.values.rows[arguments.clone()],
                    budget,
                )?;
            }
            controls += 1;
        }
        if controls != body.blocks.len() {
            return Err(InstanceCorrespondenceErrorV1::Control);
        }
        budget.charge_work(self.spans.rows.len())?;
        for row in &self.spans.rows {
            let seed = self.seed_index(row.instance, budget)?;
            if self.seeds.rows[seed].container != container {
                continue;
            }
            for segment in row.segments.into_iter().flatten() {
                budget.charge_work(body.blocks.len())?;
                let block = body
                    .blocks
                    .iter()
                    .find(|block| block.id == segment.block)
                    .ok_or(InstanceCorrespondenceErrorV1::SpanCoverage)?;
                if segment.end()? as usize > block.operations.len() {
                    return Err(InstanceCorrespondenceErrorV1::SpanCoverage);
                }
            }
        }
        Ok(())
    }
}

fn instance_check_branch_v1(
    block: &BasicBlock,
    target: BlockId,
    expected: &[ValueId],
    budget: &mut ArgumentBudgetV1<'_>,
) -> InstanceMapResultV1<()> {
    budget.charge_work(
        expected
            .len()
            .checked_add(1)
            .ok_or(ArgumentResourceV1::Arithmetic)?,
    )?;
    if !matches!(&block.terminator, Some(Terminator::Branch { target: actual, arguments }) if *actual == target && arguments.as_slice() == expected)
    {
        return Err(InstanceCorrespondenceErrorV1::Control);
    }
    Ok(())
}

fn instance_check_parameters_v1(
    block: &BasicBlock,
    expected: &[ValueId],
    budget: &mut ArgumentBudgetV1<'_>,
) -> InstanceMapResultV1<()> {
    budget.charge_work(
        expected
            .len()
            .checked_add(1)
            .ok_or(ArgumentResourceV1::Arithmetic)?,
    )?;
    if block.parameters.len() != expected.len()
        || !block
            .parameters
            .iter()
            .zip(expected)
            .all(|(actual, expected)| actual.id == *expected)
    {
        return Err(InstanceCorrespondenceErrorV1::Control);
    }
    Ok(())
}

#[cfg(test)]
#[path = "production_instance_correspondence_v1_tests.rs"]
mod instance_correspondence_tests;
