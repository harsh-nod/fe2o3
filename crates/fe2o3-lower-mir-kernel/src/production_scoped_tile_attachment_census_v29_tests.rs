// Child of projection tests. No production attachment visitor is used.
// Enumerating every retained field does not claim every optional source witness.
use super::*;

fn declared_family(field: Field) -> Family {
    match field {
        Field::Span => Family::InstanceSpans,
        Field::Parameters => Family::InstanceSeeds,
        Field::PhysicalBlock
        | Field::Terminator
        | Field::Edge
        | Field::EdgeArgument
        | Field::ReturnDefinition
        | Field::ReturnUse
        | Field::ExpectedTarget
        | Field::ExpectedArgument => Family::InstanceControls,
        Field::ArgumentPreparation
        | Field::CallSite
        | Field::DestinationPreparation
        | Field::DestinationRange
        | Field::DestinationPointer
        | Field::ArgumentDefinition
        | Field::ArgumentUse
        | Field::ResultDefinition => Family::InstanceCalls,
        Field::ReturnSite
        | Field::ReturnComponentInput
        | Field::ReturnComponentConversion
        | Field::ReturnComponentOutput
        | Field::ReturnComponentUse
        | Field::TransportComponentConversion
        | Field::TransportComponentOutput
        | Field::TransportComponentUse => Family::InstanceReturns,
        Field::RawBlock
        | Field::RawStatementSpan
        | Field::RawTerminatorSpan
        | Field::RawSyntheticSpan
        | Field::RawInvocationSpan
        | Field::RawInvocationPreheader
        | Field::RawInvocationEntry
        | Field::RawInvocationTerminator
        | Field::RawInvocationEdge
        | Field::RawInvocationArgument
        | Field::RawInvocationInputMap
        | Field::RawInvocationInput
        | Field::RawInvocationParameter
        | Field::RawInvocationOutput
        | Field::RawInvocationConversion
        | Field::RawParameter
        | Field::RawParameterComponent
        | Field::RawIgnoredParameter
        | Field::RawGeneratedInput
        | Field::RawGeneratedOutput
        | Field::RawCallArguments
        | Field::RawCallOperation
        | Field::RawCallDestination
        | Field::RawCallDestinationPointer
        | Field::RawNoNormalReturnTerminator
        | Field::RawReturnTerminator
        | Field::RawReturnInput
        | Field::RawReturnConversion
        | Field::RawTransportArgument
        | Field::RawTransportConversion => Family::RawSidecar,
        Field::SlotRawPointer
        | Field::SlotPointer
        | Field::SlotCount
        | Field::SlotCountLocation
        | Field::SlotAllocation => Family::SourceSlot,
        Field::MemoryPosition | Field::MemoryPointer | Field::MemoryLoadResult
        | Field::MemoryStoreValue | Field::MemoryStoreUse
        | Field::ObjectOperand | Field::ObjectOperandUse | Field::ObjectResult => Family::MemoryAnchor,
        Field::ArrayCountLocation
        | Field::ArrayAllocation
        | Field::ArrayPointer
        | Field::ArrayCount
        | Field::ArraySourceRange
        | Field::ArrayOriginalIndex
        | Field::ArrayDirectDefinition
        | Field::ArrayLiteralValue
        | Field::ArrayLiteralDefinition
        | Field::ArrayOffsetLocation
        | Field::ArrayGepLocation
        | Field::ArrayMemoryLocation
        | Field::ArrayOffset
        | Field::ArrayGep => Family::PrivateArray,
        Field::LifecycleOriginalGap
        | Field::LifecycleBeforeGap
        | Field::LifecycleOperation
        | Field::LifecycleOperand
        | Field::LifecycleResult => Family::Lifecycle,
        Field::AssertSourceRange
        | Field::AssertSourceBlock
        | Field::AssertSuccessBlock
        | Field::AssertFailureBlock
        | Field::AssertCapturedCondition
        | Field::AssertCapturedArgument
        | Field::AssertConditionUse
        | Field::AssertConditionDefinition
        | Field::AssertSuccessEdge
        | Field::AssertFailureEdge
        | Field::AssertSuccessArgument => Family::Assertion,
        Field::FailureSourceBlock
        | Field::FailureSourceEdge
        | Field::FailureOriginalTarget
        | Field::FailureOriginalDiagnostic
        | Field::FailureBlock
        | Field::FailureGap
        | Field::FailureCleanup
        | Field::FailureOperand
        | Field::FailureDiagnostic
        | Field::FailureTerminator => Family::TerminalFailure,
    }
}

#[derive(Default, Debug)]
pub(super) struct SidecarWitnesses {
    pub(super) private_constant: usize,
    pub(super) private_local_direct: usize,
    pub(super) private_local_indirect: usize,
    pub(super) private_literal: usize,
    pub(super) offset_present: usize,
    pub(super) offset_absent: usize,
    pub(super) slot_count_present: usize,
    pub(super) slot_count_absent: usize,
    pub(super) memory_access: usize,
    pub(super) memory_kill: usize,
    pub(super) memory_failure_read: usize,
    pub(super) discarded_scope_ends: usize,
    pub(super) assertions_emitted: usize,
    pub(super) assertions_elided: usize,
    pub(super) raw_conversions_present: usize,
    pub(super) raw_conversions_absent: usize,
    pub(super) raw_ignored_parameters: usize,
}

#[derive(Clone, Copy, Debug)]
enum Presence {
    Present,
    Absent,
    Exact(Location),
}

struct Census {
    expected: Vec<(TileAttachmentKeyV29, Presence)>,
    root: usize,
    family: Family,
    instance: usize,
    row: usize,
}
impl Census {
    fn new() -> Self {
        Self {
            expected: Vec::new(),
            root: 0,
            family: Family::RawSidecar,
            instance: 0,
            row: 0,
        }
    }
    fn context(&mut self, family: Family, instance: usize) {
        self.family = family;
        self.instance = instance;
        self.row = 0;
    }
    fn component(&mut self, field: Field, component: usize, part: usize, presence: Presence) {
        let mut key = projection_key(self.root, self.family, self.instance, self.row, field);
        key.component = component;
        key.part = part;
        self.expected.push((key, presence));
    }
    fn one(&mut self, field: Field, presence: Presence) {
        self.component(field, 0, 0, presence);
    }
    fn required(&mut self, fields: &[Field]) {
        for &field in fields {
            self.one(field, Presence::Present);
        }
    }
    fn optional(&mut self, field: Field, present: bool) {
        self.one(
            field,
            if present {
                Presence::Present
            } else {
                Presence::Absent
            },
        );
    }
    fn components(&mut self, field: Field, count: usize) {
        if count == 0 {
            self.one(field, Presence::Absent);
        }
        for component in 0..count {
            self.component(field, component, 0, Presence::Present);
        }
    }
    fn range(&mut self, field: Field, component: usize, count: usize) {
        for part in 0..count.max(1) {
            self.component(field, component, part, Presence::Present);
        }
    }
    fn sources(&mut self, field: Field, sources: Vec<Location>) {
        assert!(!sources.is_empty());
        for (part, source) in sources.into_iter().enumerate() {
            self.component(field, 0, part, Presence::Exact(source));
        }
    }
}

fn span_sources(
    candidate: &ScopedTileScalarCandidateV29,
    root: &ScopedModuleRootV29,
    instance: ProductionCallInstanceIdV1,
    source: InstanceSpanSourceV1,
) -> Vec<Location> {
    let matches: Vec<_> = root
        .coordinates
        .spans
        .rows
        .iter()
        .enumerate()
        .filter(|(_, row)| row.instance == instance && row.source == source)
        .collect();
    assert_eq!(matches.len(), 1);
    expected_span_sources(root, candidate.input.pending.pending_module(), matches[0].0)
}

fn definition_source(graph: &Module, function: usize, value: ValueId) -> Location {
    let body = graph.functions[function].body.as_ref().unwrap();
    let mut found = Vec::new();
    for (parameter, &id) in body.parameters.iter().enumerate() {
        if id == value {
            found.push(Source::FunctionParameter {
                function,
                parameter,
            });
        }
    }
    for (block, item) in body.blocks.iter().enumerate() {
        for (parameter, item) in item.parameters.iter().enumerate() {
            if item.id == value {
                found.push(Source::BlockParameter {
                    function,
                    block,
                    parameter,
                });
            }
        }
        for (operation, item) in item.operations.iter().enumerate() {
            for (result, item) in item.results.iter().enumerate() {
                if item.id == value {
                    found.push(Source::Result {
                        operation: TileScalarPointV29 {
                            function,
                            block,
                            operation,
                        },
                        result,
                    });
                }
            }
        }
    }
    assert_eq!(found.len(), 1, "original canonical definition of {value:?}");
    Location::Origin(found[0])
}

fn record_conversion(
    census: &mut Census,
    witnesses: &mut SidecarWitnesses,
    field: Field,
    component: usize,
    conversion: Option<u32>,
) {
    if conversion.is_some() {
        witnesses.raw_conversions_present += 1;
    } else {
        witnesses.raw_conversions_absent += 1;
    }
    census.component(
        field,
        component,
        0,
        if conversion.is_some() {
            Presence::Present
        } else {
            Presence::Absent
        },
    );
}

fn raw_call_census(
    census: &mut Census,
    witnesses: &mut SidecarWitnesses,
    sidecar: &PendingInstanceSidecarsV29,
    site: &SemanticKirCallReturnV1,
) {
    match site.kind {
        SemanticKirCallReturnKindV1::Call {
            arguments_first,
            call_operation,
            destination_end,
            destination,
            transport,
        } => {
            let spans: Vec<_> = sidecar
                .terminator_operation_spans
                .iter()
                .filter(|span| span.semantic_block == site.semantic_block)
                .collect();
            assert_eq!(spans.len(), 1);
            census.range(
                Field::RawCallArguments,
                0,
                (call_operation - arguments_first) as usize,
            );
            census.required(&[Field::RawCallOperation]);
            census.range(
                Field::RawCallDestination,
                0,
                (arguments_first - spans[0].first_operation_ordinal) as usize,
            );
            census.range(
                Field::RawCallDestination,
                1,
                (destination_end - call_operation - 1) as usize,
            );
            census.optional(
                Field::RawCallDestinationPointer,
                !matches!(destination, SemanticKirCallDestinationV1::Local),
            );
            let components = &sidecar.call_returns.components.rows[transport.range().unwrap()];
            census.components(Field::RawTransportArgument, components.len());
            if components.is_empty() {
                census.one(Field::RawTransportConversion, Presence::Absent);
            }
            for (component, item) in components.iter().enumerate() {
                let CallResultComponentV1::Transport { conversion, .. } = *item else {
                    panic!("transport");
                };
                record_conversion(
                    census,
                    witnesses,
                    Field::RawTransportConversion,
                    component,
                    conversion,
                );
            }
        }
        SemanticKirCallReturnKindV1::NoNormalReturnCall { arguments_first, call_operation, destination } => {
            let spans: Vec<_> = sidecar.terminator_operation_spans.iter()
                .filter(|span| span.semantic_block == site.semantic_block).collect();
            assert_eq!(spans.len(), 1);
            census.range(Field::RawCallArguments, 0, (call_operation - arguments_first) as usize);
            census.required(&[Field::RawCallOperation, Field::RawNoNormalReturnTerminator]);
            census.range(Field::RawCallDestination, 0,
                (arguments_first - spans[0].first_operation_ordinal) as usize);
            census.range(Field::RawCallDestination, 1, 0);
            census.optional(Field::RawCallDestinationPointer,
                matches!(destination, Some(SemanticKirCallDestinationV1::Retained { .. }
                    | SemanticKirCallDestinationV1::Projected { .. })));
            census.components(Field::RawTransportArgument, 0);
            census.one(Field::RawTransportConversion, Presence::Absent);
        }
        SemanticKirCallReturnKindV1::Return { components } => {
            census.required(&[Field::RawReturnTerminator]);
            let components = &sidecar.call_returns.components.rows[components.range().unwrap()];
            census.components(Field::RawReturnInput, components.len());
            if components.is_empty() {
                census.one(Field::RawReturnConversion, Presence::Absent);
            }
            for (component, item) in components.iter().enumerate() {
                let CallResultComponentV1::Return { conversion, .. } = *item else {
                    panic!("return");
                };
                record_conversion(
                    census,
                    witnesses,
                    Field::RawReturnConversion,
                    component,
                    conversion,
                );
            }
        }
    }
}

fn raw_census(
    candidate: &ScopedTileScalarCandidateV29,
    root: &ScopedModuleRootV29,
    census: &mut Census,
    witnesses: &mut SidecarWitnesses,
) {
    for sidecar in &root.sidecars.rows {
        let source_instance = sidecar.source_call_instance.unwrap();
        census.context(Family::RawSidecar, source_instance.index());
        assert_eq!(root.coordinates.sources.rows[source_instance.index()].instance, source_instance);
        assert_eq!(sidecar.source_call_instance, Some(source_instance));
        if let Some(invocation) = &sidecar.invocation_entry {
            census.sources(
                Field::RawInvocationSpan,
                span_sources(
                    candidate,
                    root,
                    source_instance,
                    InstanceSpanSourceV1::InvocationEntry(invocation.span),
                ),
            );
            census.required(&[
                Field::RawInvocationPreheader,
                Field::RawInvocationEntry,
                Field::RawInvocationTerminator,
                Field::RawInvocationEdge,
            ]);
            census.row += 1;
            for _ in &invocation.arguments {
                census.one(Field::RawInvocationArgument, Presence::Absent);
                census.row += 1;
            }
            for _ in &invocation.inputs {
                census.one(Field::RawInvocationInputMap, Presence::Absent);
                census.row += 1;
            }
            for component in &invocation.components {
                census.required(&[
                    Field::RawInvocationInput,
                    Field::RawInvocationParameter,
                    Field::RawInvocationOutput,
                ]);
                census.optional(
                    Field::RawInvocationConversion,
                    component.conversion.is_some(),
                );
                census.row += 1;
            }
        }
        for _ in &sidecar.blocks {
            census.required(&[Field::RawBlock]);
            census.row += 1;
        }
        for &span in &sidecar.statement_operation_spans {
            census.sources(
                Field::RawStatementSpan,
                span_sources(
                    candidate,
                    root,
                    source_instance,
                    InstanceSpanSourceV1::Statement(span),
                ),
            );
            census.row += 1;
        }
        for &span in &sidecar.terminator_operation_spans {
            census.sources(
                Field::RawTerminatorSpan,
                span_sources(
                    candidate,
                    root,
                    source_instance,
                    InstanceSpanSourceV1::Terminator(span),
                ),
            );
            census.row += 1;
        }
        for _ in &sidecar.generated_terminator_values {
            census.required(&[Field::RawGeneratedInput, Field::RawGeneratedOutput]);
            census.row += 1;
        }
        for call in &sidecar.call_returns.sites.rows {
            raw_call_census(census, witnesses, sidecar, call);
            census.row += 1;
        }
        for &span in &sidecar.synthetic_operation_spans {
            census.sources(
                Field::RawSyntheticSpan,
                span_sources(
                    candidate,
                    root,
                    source_instance,
                    InstanceSpanSourceV1::Synthetic(span),
                ),
            );
            census.row += 1;
        }
        for _ in &sidecar.parameter_bindings {
            census.required(&[Field::RawParameter]);
            census.row += 1;
        }
        for _ in &sidecar.parameter_component_bindings {
            census.required(&[Field::RawParameterComponent]);
            census.row += 1;
        }
        for _ in &sidecar.ignored_parameter_bindings {
            census.one(Field::RawIgnoredParameter, Presence::Absent);
            witnesses.raw_ignored_parameters += 1;
            census.row += 1;
        }
    }
}

fn slot_census(root: &ScopedModuleRootV29, census: &mut Census, witnesses: &mut SidecarWitnesses) {
    assert_eq!(root.source_slots.instances.len(), root.sidecars.rows.len());
    for (instance, frame) in root.source_slots.instances.iter().enumerate() {
        census.context(Family::SourceSlot, frame.instance.index());
        assert_eq!(
            frame.instance,
            root.coordinates.sources.rows[frame.instance.index()].instance
        );
        assert_eq!(root.sidecars.rows[instance].source_call_instance, Some(frame.instance));
        if let Some(origins) = &root.sidecars.rows[instance].scoped_slot_origins {
            for _ in origins {
                census.required(&[Field::SlotRawPointer]);
                census.row += 1;
            }
        }
        for slot in &root.source_slots.slots[frame.slots.clone()] {
            assert_eq!(slot.instance, frame.instance);
            census.required(&[Field::SlotPointer]);
            census.optional(Field::SlotCount, slot.representation.count().is_some());
            census.optional(Field::SlotCountLocation, slot.representation.count().is_some());
            census.required(&[Field::SlotAllocation]);
            if slot.representation.count().is_some() {
                witnesses.slot_count_present += 1;
            } else {
                witnesses.slot_count_absent += 1;
            }
            census.row += 1;
        }
    }
}

fn memory_census(
    root: &ScopedModuleRootV29,
    census: &mut Census,
    witnesses: &mut SidecarWitnesses,
) {
    for sidecar in &root.sidecars.rows {
        census.context(Family::MemoryAnchor, sidecar.source_call_instance.unwrap().index());
        let Some(anchors) = &sidecar.scoped_memory_anchors else {
            continue;
        };
        for anchor in &anchors.rows {
            census.required(&[Field::MemoryPosition]);
            match anchor.kind {
                ScopedMemoryAnchorKindV29::Object(index) => {
                    census.one(Field::MemoryPointer, Presence::Absent);
                    let payload = anchors.objects[index];
                    let (operands, result) = match payload.operation {
                        fe2o3_kernel_ir::StorageOperationV1::Project { step, .. } => (
                            if matches!(step, fe2o3_kernel_ir::StorageProjectionV1::ArrayIndex(_)) { 2 } else { 1 }, true),
                        fe2o3_kernel_ir::StorageOperationV1::ReadValue { .. }
                        | fe2o3_kernel_ir::StorageOperationV1::ReadDiscriminant { .. } => (1, true),
                        fe2o3_kernel_ir::StorageOperationV1::WriteValue { .. }
                        | fe2o3_kernel_ir::StorageOperationV1::CopyObject { .. } => (2, false),
                        fe2o3_kernel_ir::StorageOperationV1::SetDiscriminant { .. } => (1, false),
                    };
                    for component in 0..2 {
                        let presence = if component < operands { Presence::Present } else { Presence::Absent };
                        census.component(Field::ObjectOperand, component, 0, presence);
                        census.component(Field::ObjectOperandUse, component, 0, presence);
                    }
                    census.one(Field::ObjectResult, if result { Presence::Present } else { Presence::Absent });
                }
                ScopedMemoryAnchorKindV29::Access { .. } => {
                    census.required(&[Field::MemoryPointer]);
                    witnesses.memory_access += 1;
                }
                ScopedMemoryAnchorKindV29::Kill { .. } => {
                    census.one(Field::MemoryPointer, Presence::Absent);
                    witnesses.memory_kill += 1;
                }
                ScopedMemoryAnchorKindV29::FailureRead { .. } => {
                    census.one(Field::MemoryPointer, Presence::Absent);
                    witnesses.memory_failure_read += 1;
                }
            }
            let (load, store) = match anchor.kind {
                ScopedMemoryAnchorKindV29::Access { payload: Some(ScopedMemoryPayloadV29::Load { .. } | ScopedMemoryPayloadV29::IndexLoad { .. }), .. } => (true, false),
                ScopedMemoryAnchorKindV29::Access { payload: Some(ScopedMemoryPayloadV29::Store { .. }), .. } => (false, true),
                _ => (false, false),
            };
            if load { census.required(&[Field::MemoryLoadResult]); }
            else { census.one(Field::MemoryLoadResult, Presence::Absent); }
            if store { census.required(&[Field::MemoryStoreValue, Field::MemoryStoreUse]); }
            else {
                census.one(Field::MemoryStoreValue, Presence::Absent);
                census.one(Field::MemoryStoreUse, Presence::Absent);
            }
            census.row += 1;
        }
    }
}

fn private_census(
    root: &ScopedModuleRootV29,
    census: &mut Census,
    witnesses: &mut SidecarWitnesses,
) {
    for sidecar in &root.sidecars.rows {
        census.context(Family::PrivateArray, sidecar.source_call_instance.unwrap().index());
        let arrays = &sidecar.private_arrays;
        for _ in &arrays.slots {
            census.required(&[
                Field::ArrayCountLocation,
                Field::ArrayAllocation,
                Field::ArrayPointer,
                Field::ArrayCount,
            ]);
            census.row += 1;
        }
        for effect in &arrays.effects {
            census.range(
                Field::ArraySourceRange,
                0,
                effect
                    .source_end_operation
                    .checked_sub(effect.source_first_operation)
                    .unwrap(),
            );
            let (original, direct, literal) = match effect.original_index {
                PrivateArrayIndexV1::ConstantIndex { .. } => {
                    witnesses.private_constant += 1;
                    (false, false, false)
                }
                PrivateArrayIndexV1::Local {
                    direct_definition, ..
                } => {
                    if direct_definition.is_some() {
                        witnesses.private_local_direct += 1;
                    } else {
                        witnesses.private_local_indirect += 1;
                    }
                    (true, direct_definition.is_some(), false)
                }
                PrivateArrayIndexV1::InitializerElement {
                    value: PrivateArrayInitializerValueV1::LiteralScalar { .. },
                    ..
                } => {
                    witnesses.private_literal += 1;
                    (false, false, true)
                }
            };
            census.optional(Field::ArrayOriginalIndex, original);
            census.optional(Field::ArrayDirectDefinition, direct);
            census.optional(Field::ArrayLiteralValue, literal);
            census.optional(Field::ArrayLiteralDefinition, literal);
            census.optional(Field::ArrayOffsetLocation, effect.offset_location.is_some());
            census.required(&[
                Field::ArrayGepLocation,
                Field::ArrayMemoryLocation,
                Field::ArrayOffset,
                Field::ArrayGep,
            ]);
            if effect.offset_location.is_some() {
                witnesses.offset_present += 1;
            } else {
                witnesses.offset_absent += 1;
            }
            census.row += 1;
        }
    }
}

fn lifecycle_values(
    operation: &Operation,
    witnesses: &mut SidecarWitnesses,
) -> (Vec<ValueId>, usize) {
    use kir::ExecutionOperationV15 as Execution;
    let OperationKind::Execution(execution) = &operation.kind else {
        panic!("inserted execution operation");
    };
    match execution {
        Execution::ContextIssue => (vec![], 1),
        Execution::WorkgroupDerive { context } => (vec![*context], 1),
        Execution::ScopeEnd {
            workgroup,
            discarded,
        } => {
            let mut operands = vec![*workgroup];
            operands.extend(discarded);
            if !discarded.is_empty() {
                witnesses.discarded_scope_ends += 1;
            }
            (operands, 0)
        }
        Execution::MaskedTileLoadU32 {
            workgroup,
            input,
            base,
            ..
        } => (vec![*workgroup, *input, *base], 1),
        Execution::TileIntoFragmentU32 { tile, .. } => (vec![*tile], 1),
        Execution::FragmentIntoPartsU32 {
            fragment, elements, ..
        } => (vec![*fragment], 2 * usize::from(*elements)),
    }
}

fn lifecycle_census(
    candidate: &ScopedTileScalarCandidateV29,
    root: &ScopedModuleRootV29,
    census: &mut Census,
    witnesses: &mut SidecarWitnesses,
) {
    let graph = candidate.input.pending.pending_module();
    let mut total = 0;
    for sidecar in &root.sidecars.rows {
        census.context(Family::Lifecycle, sidecar.source_call_instance.unwrap().index());
        let Some(events) = &sidecar.lifecycle_events else {
            continue;
        };
        for (event, _) in events.rows.iter().enumerate() {
            let insertions: Vec<_> = root
                .insertions
                .iter()
                .filter(|row| row.instance == events.instance && row.event == event)
                .collect();
            assert_eq!(insertions.len(), 1);
            let insertion = insertions[0];
            let block = original_block(root, graph, insertion.after.block);
            let point = TileScalarPointV29 {
                function: root.function_ordinal,
                block,
                operation: insertion.after.first as usize,
            };
            let operation = &graph.functions[point.function]
                .body
                .as_ref()
                .unwrap()
                .blocks[point.block]
                .operations[point.operation];
            let (operands, results) = lifecycle_values(operation, witnesses);
            assert_eq!(operation.results.len(), results);
            census.required(&[Field::LifecycleOriginalGap, Field::LifecycleBeforeGap]);
            census.one(
                Field::LifecycleOperation,
                Presence::Exact(Location::Origin(Source::Operation(point))),
            );
            if operands.is_empty() {
                census.one(Field::LifecycleOperand, Presence::Absent);
            }
            for (component, value) in operands.into_iter().enumerate() {
                census.component(
                    Field::LifecycleOperand,
                    component,
                    0,
                    Presence::Exact(definition_source(graph, root.function_ordinal, value)),
                );
            }
            if results == 0 {
                census.one(Field::LifecycleResult, Presence::Absent);
            }
            for (component, value) in operation.results.iter().enumerate() {
                census.component(
                    Field::LifecycleResult,
                    component,
                    0,
                    Presence::Exact(definition_source(graph, root.function_ordinal, value.id)),
                );
            }
            census.row += 1;
            total += 1;
        }
    }
    assert_eq!(total, root.insertions.len());
}

fn edge_source(edge: kir::CanonicalKirEdgeCoordinateV1) -> Location {
    Location::Origin(Source::Edge {
        function: edge.source.function.0 as usize,
        block: edge.source.block as usize,
        edge: edge.successor as usize,
    })
}

fn assertion_census(
    candidate: &ScopedTileScalarCandidateV29,
    root: &ScopedModuleRootV29,
    census: &mut Census,
    witnesses: &mut SidecarWitnesses,
) {
    let mut total = 0;
    for sidecar in &root.sidecars.rows {
        census.context(Family::Assertion, sidecar.source_call_instance.unwrap().index());
        let Some(capture) = &sidecar.instance_assert_origins else {
            continue;
        };
        let mut argument_end = 0;
        for source in &capture.records {
            let bindings: Vec<_> = candidate
                .input
                .pending
                .inner
                .assertions
                .iter()
                .filter(|row| {
                    row.binding.block().function.0 as usize == root.function_ordinal
                        && row.instance == capture.instance
                        && row.site == source.site
                })
                .collect();
            assert_eq!(bindings.len(), 1);
            let binding = bindings[0].binding;
            census.range(Field::AssertSourceRange, 0, source.operation_count as usize);
            census.required(&[Field::AssertSourceBlock, Field::AssertSuccessBlock]);
            let emitted = matches!(source.outcome, PendingAssertOutcomeV1::Emitted { .. });
            census.optional(Field::AssertFailureBlock, emitted);
            census.optional(Field::AssertCapturedCondition, emitted);
            assert_eq!(source.argument_start, argument_end);
            argument_end += source.argument_count;
            assert!(argument_end <= capture.arguments.len());
            census.components(Field::AssertCapturedArgument, source.argument_count);
            let success = match binding.outcome() {
                SemanticKirAssertConditionOutcomeV1::Emitted {
                    condition_use,
                    definition,
                    success_edge,
                    failure_edge,
                } => {
                    assert!(emitted);
                    census.one(
                        Field::AssertConditionUse,
                        Presence::Exact(Location::Use(condition_use)),
                    );
                    census.one(
                        Field::AssertConditionDefinition,
                        Presence::Exact(Location::Origin(source_definition(definition))),
                    );
                    census.one(
                        Field::AssertSuccessEdge,
                        Presence::Exact(edge_source(success_edge)),
                    );
                    census.one(
                        Field::AssertFailureEdge,
                        Presence::Exact(edge_source(failure_edge)),
                    );
                    witnesses.assertions_emitted += 1;
                    success_edge
                }
                SemanticKirAssertConditionOutcomeV1::ElidedByExistingRule { success_edge } => {
                    assert!(!emitted);
                    census.one(Field::AssertConditionUse, Presence::Absent);
                    census.one(Field::AssertConditionDefinition, Presence::Absent);
                    census.one(
                        Field::AssertSuccessEdge,
                        Presence::Exact(edge_source(success_edge)),
                    );
                    census.one(Field::AssertFailureEdge, Presence::Absent);
                    witnesses.assertions_elided += 1;
                    success_edge
                }
            };
            if source.argument_count == 0 {
                census.one(Field::AssertSuccessArgument, Presence::Absent);
            }
            for component in 0..source.argument_count {
                census.component(
                    Field::AssertSuccessArgument,
                    component,
                    0,
                    Presence::Exact(Location::EdgeArgument(
                        kir::CanonicalKirEdgeArgumentCoordinateV1 {
                            edge: success,
                            argument: u32::try_from(component).unwrap(),
                        },
                    )),
                );
            }
            census.row += 1;
            total += 1;
        }
        assert_eq!(argument_end, capture.arguments.len());
    }
    assert_eq!(
        total,
        candidate
            .input
            .pending
            .inner
            .assertions
            .iter()
            .filter(|row| row.binding.block().function.0 as usize == root.function_ordinal)
            .count()
    );
}

pub(super) fn assert_sidecar_census(candidate: &ScopedTileScalarCandidateV29) -> SidecarWitnesses {
    let mut census = Census::new();
    let mut witnesses = SidecarWitnesses::default();
    for (root, source) in candidate
        .input
        .pending
        .inner
        .pending
        .roots
        .iter()
        .enumerate()
    {
        census.root = root;
        raw_census(candidate, source, &mut census, &mut witnesses);
        slot_census(source, &mut census, &mut witnesses);
        memory_census(source, &mut census, &mut witnesses);
        private_census(source, &mut census, &mut witnesses);
        lifecycle_census(candidate, source, &mut census, &mut witnesses);
        assertion_census(candidate, source, &mut census, &mut witnesses);
        terminal_failure_census(candidate, source, &mut census);
    }
    let actual: Vec<_> = candidate
        .projections
        .rows
        .iter()
        .filter(|row| !is_core(row.key.family))
        .collect();
    let expected_keys: Vec<_> = census.expected.iter().map(|(key, _)| *key).collect();
    assert_eq!(
        actual.iter().map(|row| row.key).collect::<Vec<_>>(),
        expected_keys,
        "exact ordered source-field census, including all absent optional fields"
    );
    for ((key, presence), actual) in census.expected.iter().zip(actual) {
        assert_eq!(key.family, declared_family(key.field));
        assert_eq!(
            projected_row(candidate, *key).key,
            actual.key,
            "keys are unique"
        );
        match presence {
            Presence::Present => assert_ne!(actual.source, Location::NoOutput, "{key:?}"),
            Presence::Absent => assert_eq!(
                (actual.source, actual.target),
                (Location::NoOutput, Target::NoOutput),
                "{key:?}"
            ),
            Presence::Exact(source) => assert_eq!(&actual.source, source, "{key:?}"),
        }
    }
    witnesses
}

fn terminal_failure_census(
    candidate: &ScopedTileScalarCandidateV29, root: &ScopedModuleRootV29, census: &mut Census,
) {
    let Some(relation) = &root.terminal_failures else { return; };
    assert_eq!(relation.origins.rows.len(), relation.closures.len());
    let graph = candidate.input.pending.pending_module();
    let body = graph.functions[root.function_ordinal].body.as_ref().unwrap();
    let locate = |id| {
        let rows: Vec<_> = body.blocks.iter().enumerate().filter(|(_, block)| block.id == id).collect();
        assert_eq!(rows.len(), 1);
        rows[0]
    };
    let block_location = |block| Location::Origin(Source::Block { function: root.function_ordinal, block });
    let operation_location = |block, operation| Location::Origin(Source::Operation(TileScalarPointV29 {
        function: root.function_ordinal, block, operation,
    }));
    for (row, (origin, closure)) in relation.origins.rows.iter().zip(&relation.closures).enumerate() {
        assert_eq!(closure.origin, row);
        census.context(Family::TerminalFailure, origin.instance.index());
        census.row = row;
        let (source, successor, original) = match origin.site {
            TerminalFailureSiteV18::Edge { block, successor, target } => (block, Some(successor), Some(target)),
            TerminalFailureSiteV18::Operation { block, .. } => (block, None, None),
        };
        let source = locate(source).0;
        let (terminal, terminal_body) = locate(closure.block);
        census.one(Field::FailureSourceBlock, Presence::Exact(block_location(source)));
        census.one(Field::FailureSourceEdge, successor.map_or(Presence::Absent, |edge| Presence::Exact(
            Location::Origin(Source::Edge { function: root.function_ordinal, block: source, edge: edge as usize }))));
        census.one(Field::FailureOriginalTarget, original.map_or(Presence::Absent,
            |block| Presence::Exact(block_location(locate(block).0))));
        census.one(Field::FailureOriginalDiagnostic, Presence::Exact(operation_location(
            original.map_or(terminal, |block| locate(block).0),
            if closure.generated { 0 } else { closure.diagnostic as usize })));
        census.one(Field::FailureBlock, Presence::Exact(block_location(terminal)));
        census.one(Field::FailureGap, Presence::Exact(Location::Gap(TileScalarPointV29 {
            function: root.function_ordinal, block: terminal, operation: closure.first as usize,
        })));
        let cleanup = &terminal_body.operations[closure.first as usize..closure.diagnostic as usize];
        assert_eq!(cleanup.len(), closure.scope_ends as usize);
        if cleanup.is_empty() {
            census.one(Field::FailureCleanup, Presence::Absent);
            census.one(Field::FailureOperand, Presence::Absent);
        }
        for (component, operation) in cleanup.iter().enumerate() {
            census.component(Field::FailureCleanup, component, 0, Presence::Exact(operation_location(
                terminal, closure.first as usize + component)));
            let OperationKind::Execution(kir::ExecutionOperationV15::ScopeEnd { workgroup, discarded }) = &operation.kind
                else { panic!("terminal prefix is not lifecycle cleanup"); };
            for (part, &value) in std::iter::once(workgroup).chain(discarded).enumerate() {
                census.component(Field::FailureOperand, component, part,
                    Presence::Exact(definition_source(graph, root.function_ordinal, value)));
            }
        }
        census.one(Field::FailureDiagnostic, Presence::Exact(operation_location(terminal, closure.diagnostic as usize)));
        census.one(Field::FailureTerminator, Presence::Exact(Location::Origin(Source::Terminator {
            function: root.function_ordinal, block: terminal,
        })));
    }
}

#[test]
fn retained_sidecar_keys_equal_independent_ordered_census() {
    for case in [
        SourceCase::Repeated,
        SourceCase::Slots,
        SourceCase::Shifted,
        SourceCase::Discard,
        SourceCase::BranchParts,
    ] {
        with_projection_candidate(case, |candidate, _| {
            let witnessed = assert_sidecar_census(candidate);
            if matches!(case, SourceCase::Slots | SourceCase::Shifted) {
                assert!(witnessed.slot_count_absent > 0 && witnessed.memory_access > 0);
            }
            if matches!(case, SourceCase::Discard) {
                assert!(witnessed.discarded_scope_ends > 0);
            }
            eprintln!("{case:?} source witness census: {witnessed:?}");
        });
    }
}

#[test]
fn assertion_and_lifetime_source_variants_have_complete_sidecar_keys() {
    for variant in [
        ProjectionSourceVariant::Assertion,
        ProjectionSourceVariant::LifetimeKills,
    ] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(SCHEDULE_LIMIT);
        let mut budget = ArgumentBudgetV1::new(&mut work, SCHEDULE_LIMIT);
        budget.reserve_storage(SCHEDULE_FLOOR).unwrap();
        let candidate = variant_candidate(variant, &mut budget);
        let witnesses = assert_sidecar_census(&candidate);
        match variant {
            ProjectionSourceVariant::Assertion => {
                assert_eq!(witnesses.assertions_emitted, 2);
                assert_eq!(witnesses.memory_failure_read, 2);
            }
            ProjectionSourceVariant::LifetimeKills => assert!(witnesses.memory_kill >= 4),
        }
        eprintln!("modified source witness census: {witnesses:?}");
        candidate.replay_with_budget(&mut budget).unwrap();
        drop_scalar_candidate(candidate, &mut budget);
        assert_eq!(budget.storage(), SCHEDULE_FLOOR);
        budget.release_storage(SCHEDULE_FLOOR).unwrap();
    }
}

#[test]
fn scalar_payload_operand_visitor_preserves_occurrences_and_exact_work() {
    for guarded in [false, true] {
        let kind = if guarded {
            OperationKind::GuardedStore {
                pointer: ValueId(7), predicate: ValueId(7), value: ValueId(7),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            }
        } else {
            OperationKind::Store { pointer: ValueId(7), value: ValueId(7),
                access: MemoryAccess::new(AddressSpace::Global, 4) }
        };
        let operation = Operation::new(vec![], kind);
        let required = if guarded { 4 } else { 3 };
        for limit in [required - 1, required] {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(limit);
            let mut budget = ArgumentBudgetV1::new(&mut work, 0);
            let result = tile_store_payload_operand_v18(&operation, ValueId(7), ValueId(7), &mut budget);
            if limit == required {
                assert_eq!(result.unwrap(), if guarded { 2 } else { 1 });
            } else {
                assert!(matches!(result, Err(ScopedTileFailureKindV29::Resource(ArgumentResourceV1::Work(_)))));
            }
            assert_eq!(budget.work(), limit);
            assert_eq!(budget.storage(), 0);
            assert_eq!(work.failed_work().is_some(), limit < required);
        }
        for (pointer, value) in [(ValueId(8), ValueId(7)), (ValueId(7), ValueId(8))] {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(10);
            let mut budget = ArgumentBudgetV1::new(&mut work, 0);
            assert!(matches!(tile_store_payload_operand_v18(&operation, pointer, value, &mut budget),
                Err(ScopedTileFailureKindV29::ReplayMismatch)));
            assert_eq!(budget.work(), 1);
        }
        let illegal_result = Operation::new(vec![ValueDef::new(ValueId(11), Type::Scalar(ScalarType::U32))],
            operation.kind.clone());
        let mut work = CanonicalKernelIrWorkBudgetV1::new(10);
        let mut budget = ArgumentBudgetV1::new(&mut work, 0);
        assert!(matches!(tile_store_payload_operand_v18(&illegal_result, ValueId(7), ValueId(7), &mut budget),
            Err(ScopedTileFailureKindV29::ReplayMismatch)));
        assert_eq!(budget.work(), 1);
    }
}

fn scalar_payload_definition_value(graph: &Module, location: Location) -> ValueId {
    match location {
        Location::Origin(Source::FunctionParameter { function, parameter }) =>
            graph.functions[function].body.as_ref().unwrap().parameters[parameter],
        Location::Origin(Source::BlockParameter { function, block, parameter }) =>
            graph.functions[function].body.as_ref().unwrap().blocks[block].parameters[parameter].id,
        Location::Origin(Source::Result { operation, result }) =>
            graph.functions[operation.function].body.as_ref().unwrap().blocks[operation.block]
                .operations[operation.operation].results[result].id,
        other => panic!("payload has no original definition: {other:?}"),
    }
}

#[test]
fn actual_repeated_source_payloads_keep_load_result_store_definition_and_store_use_distinct() {
    let completed = std::cell::Cell::new(false);
    with_projection_candidate(SourceCase::Slots, |candidate, _| {
        let graph = candidate.input.pending.inner.pending.graph.module();
        let mut loads = 0;
        let mut stores = 0;
        let mut payload_instances = std::collections::BTreeSet::new();
        for (root_ordinal, root) in candidate.input.pending.inner.pending.roots.iter().enumerate() {
            for (instance, sidecar) in root.sidecars.rows.iter().enumerate() {
                let Some(anchors) = &sidecar.scoped_memory_anchors else { continue; };
                for (row, anchor) in anchors.rows.iter().enumerate() {
                    let key = |field| projection_key(root_ordinal, Family::MemoryAnchor, instance, row, field);
                    let (pointer, payload) = match anchor.kind {
                        ScopedMemoryAnchorKindV29::Access { pointer, payload: Some(payload) } => (pointer, payload),
                        _ => {
                            for field in [Field::MemoryLoadResult, Field::MemoryStoreValue, Field::MemoryStoreUse] {
                                assert_eq!(projected_row(candidate, key(field)).source, Location::NoOutput);
                            }
                            continue;
                        }
                    };
                    payload_instances.insert((root_ordinal, instance));
                    let Location::Origin(Source::Operation(point)) = projected_row(candidate, key(Field::MemoryPosition)).source else {
                        panic!("memory position must name its actual operation");
                    };
                    let operation = &graph.functions[point.function].body.as_ref().unwrap().blocks[point.block]
                        .operations[point.operation];
                    assert_eq!(scalar_payload_definition_value(graph, projected_row(candidate, key(Field::MemoryPointer)).source), pointer);
                    match payload {
                        ScopedMemoryPayloadV29::Load { result, .. } | ScopedMemoryPayloadV29::IndexLoad { result, .. } => {
                            assert_eq!(operation.results.len(), 1);
                            assert_eq!(operation.results[0].id, result);
                            assert_eq!(scalar_payload_definition_value(graph, projected_row(candidate, key(Field::MemoryLoadResult)).source), result);
                            for field in [Field::MemoryStoreValue, Field::MemoryStoreUse] {
                                assert_eq!(projected_row(candidate, key(field)).source, Location::NoOutput);
                            }
                            loads += 1;
                        }
                        ScopedMemoryPayloadV29::Store { value, .. } => {
                            assert_eq!(scalar_payload_definition_value(graph, projected_row(candidate, key(Field::MemoryStoreValue)).source), value);
                            assert_eq!(projected_row(candidate, key(Field::MemoryLoadResult)).source, Location::NoOutput);
                            let expected_operand = match &operation.kind {
                                OperationKind::Store { pointer: actual, value: rhs, .. } => {
                                    assert_eq!((*actual, *rhs), (pointer, value)); 1
                                }
                                OperationKind::GuardedStore { pointer: actual, value: rhs, .. } => {
                                    assert_eq!((*actual, *rhs), (pointer, value)); 2
                                }
                                other => panic!("payload Store names {other:?}"),
                            };
                            let expected = UseCoordinate::OperationOperand {
                                operation: kir::CanonicalKirOperationCoordinateV1 {
                                    block: kir::CanonicalKirBlockCoordinateV1 {
                                        function: kir::CanonicalKirFunctionCoordinateV1(u32::try_from(point.function).unwrap()),
                                        block: u32::try_from(point.block).unwrap(),
                                    }, operation: u32::try_from(point.operation).unwrap(),
                                }, operand: expected_operand,
                            };
                            assert_eq!(projected_row(candidate, key(Field::MemoryStoreUse)).source, Location::Use(expected));
                            stores += 1;
                        }
                    }
                }
            }
        }
        assert!(loads >= 2 && stores >= 2, "actual repeated helpers must carry scalar Load and Store payloads");
        assert!(payload_instances.len() >= 2, "payloads must retain original instance identity");
        assert_sidecar_census(candidate);
        completed.set(true);
    });
    assert!(completed.get(), "the actual source callback must run to completion");
}

// Remaining positive witness requirements, not weakened into optional assertions:
// private-array variants are supplied by the parallel source-fixture proposal;
// offset_location=None may currently fail source admission and is not fabricated;
// elided assertions, nonempty raw conversions, ignored parameters, and multi-root
// assertions sharing (instance, site) need explicit witness qualification.

mod private_array_source_tests {
    // Proposal: child of the NEW projection tests alongside sidecar_census_tests.
    // Reuses the primary's shared source candidate/pending helpers, never a parallel
    // admission pipeline. Only inert source is edited and readmitted, never KIR or
    // retained private-array records. This file is not execution evidence.
    //
    // Reachability at this implementation boundary:
    // * ConstantIndex -> offset Some; no original/direct/literal selector.
    // * Local literal -> direct Some, offset Some.
    // * Local cast-literal -> direct None, offset Some. The emitter propagates its
    //   numeric constant through Cast, but the private recorder indexes only literals.
    // * InitializerElement(LiteralScalar) -> literal definition and offset Some.
    // * Truly dynamic Local -> direct None, offset None; admission requires the
    //   checked symbolic bounds and current-value relation, not a guessed literal.
    // ConstantIndex/initializer offset None and literal-local direct Some/offset None
    // are not source-producible in this boundary; do not forge them for coverage.
    use super::*;

    #[derive(Clone, Copy, Debug)]
    enum PrivateArraySourceVariantV29 {
        ConstantIndex,
        LocalLiteral,
        LocalCastLiteral,
        DynamicLocal,
        UnboundedLocal,
    }

    fn private_array_literal_v29(value: u128) -> SemanticOperandV1 {
        SemanticOperandV1::Constant(SemanticConstantV1::new(
            U32,
            SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(value, 4).unwrap()),
        ))
    }

    fn private_array_assign_v29(
        destination: SemanticPlaceV1,
        value: SemanticRvalueKindV1,
    ) -> SemanticStatementV1 {
        let ty = destination.ty();
        SemanticStatementV1::new(
            source(),
            SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                destination,
                SemanticRvalueV1::new(ty, value),
            )),
        )
    }

    // Two actual helper invocations each own a distinct array allocation. Their
    // literal initializer, element write, and element read are all source statements.
    fn private_array_source_owner_v29(
        variant: PrivateArraySourceVariantV29,
    ) -> ProductionSemanticSsaOwnerV1 {
        let template = source_owner(SourceCase::Slots);
        let semantic = template.source_semantic();
        let mut functions = semantic.functions().to_vec();
        let helper = functions[3].clone();
        let types = semantic.types();
        let parts = helper.locals()[6].ty();
        let SemanticTypeShapeV1::Tuple(parts_fields) = types[parts.index() as usize].shape() else {
            panic!("the existing helper's Parts result must be a tuple");
        };
        let array_type = parts_fields.fields()[0];
        assert!(matches!(
            types[array_type.index() as usize].shape(),
            SemanticTypeShapeV1::Array { element, length: 2 } if *element == U32
        ));
        let workgroup = semantic.functions()[2].abi().source_input_types()[0];
        let SemanticTypeShapeV1::Aggregate(workgroup_fields) =
            types[workgroup.index() as usize].shape()
        else {
            panic!("the source Workgroup must be an aggregate");
        };
        let usize_type = workgroup_fields.fields()[0];
        let index_type = if matches!(variant, PrivateArraySourceVariantV29::LocalCastLiteral) {
            usize_type
        } else {
            U32
        };
        let mut locals = helper.locals().to_vec();
        let first = u32::try_from(locals.len()).unwrap();
        let array_local = SemanticLocalIdV1::from_index(first);
        let index_local = SemanticLocalIdV1::from_index(first + 1);
        let read_local = SemanticLocalIdV1::from_index(first + 2);
        for (tag, ty) in [(230_u8, array_type), (231, index_type), (232, U32)] {
            locals.push(SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256([tag; 32]),
                ty,
                SemanticLocalRoleV1::Temporary,
                source(),
            ));
        }
        let whole_array = SemanticPlaceV1::new(array_local, vec![], array_type).unwrap();
        let array_element = SemanticPlaceV1::new(
            array_local,
            vec![
                SemanticProjectionV1::new(
                    if matches!(variant, PrivateArraySourceVariantV29::ConstantIndex) {
                        SemanticProjectionKindV1::ConstantIndex {
                            offset: 1,
                            minimum_length: 2,
                            from_end: false,
                        }
                    } else {
                        SemanticProjectionKindV1::Index(index_local)
                    },
                    U32,
                )
                .unwrap(),
            ],
            U32,
        )
        .unwrap();
        let mut statements = vec![private_array_assign_v29(
            whole_array,
            SemanticRvalueKindV1::Aggregate(
                SemanticAggregateRvalueV1::new(
                    SemanticAggregateKindV1::Array,
                    vec![private_array_literal_v29(17), private_array_literal_v29(29)],
                )
                .unwrap(),
            ),
        )];
        let index_value = match variant {
            PrivateArraySourceVariantV29::ConstantIndex => None,
            PrivateArraySourceVariantV29::LocalLiteral => {
                Some(SemanticRvalueKindV1::Use(private_array_literal_v29(1)))
            }
            PrivateArraySourceVariantV29::LocalCastLiteral => Some(SemanticRvalueKindV1::Cast {
                kind: SemanticCastKindV1::Integer,
                operand: private_array_literal_v29(1),
            }),
            PrivateArraySourceVariantV29::DynamicLocal
            | PrivateArraySourceVariantV29::UnboundedLocal => Some(SemanticRvalueKindV1::Binary {
                operation: SemanticBinaryOpV1::BitAnd,
                left: SemanticOperandV1::Copy(place(1, U32)),
                right: private_array_literal_v29(if matches!(variant, PrivateArraySourceVariantV29::DynamicLocal) { 1 } else { 3 }),
            }),
        };
        if let Some(value) = index_value {
            statements.push(private_array_assign_v29(
                SemanticPlaceV1::new(index_local, vec![], index_type).unwrap(),
                value,
            ));
        }
        statements.push(private_array_assign_v29(
            array_element.clone(),
            SemanticRvalueKindV1::Use(private_array_literal_v29(41)),
        ));
        statements.push(private_array_assign_v29(
            SemanticPlaceV1::new(read_local, vec![], U32).unwrap(),
            SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(array_element)),
        ));
        let mut blocks = helper.blocks().to_vec();
        statements.extend_from_slice(blocks[0].statements());
        blocks[0] = SemanticBasicBlockV1::new(
            blocks[0].identity(),
            blocks[0].source(),
            statements,
            blocks[0].terminator().clone(),
        )
        .unwrap();
        functions[3] = SemanticFunctionDeclV1::new(
            helper.identity(),
            helper.role(),
            helper.item_definition_identity(),
            helper.monomorphization_identity(),
            helper.generic_type_arguments_identity(),
            helper.const_generic_arguments_identity(),
            helper.source(),
            helper.abi().clone(),
            locals,
            helper.entry(),
            blocks,
        )
        .unwrap();
        let admitted = InertSemanticMirRequestV1::new_with_callables(
            semantic.target(),
            types.to_vec(),
            semantic.allocations().to_vec(),
            semantic.statics().to_vec(),
            semantic.vtables().to_vec(),
            functions,
            semantic.callables().to_vec(),
            semantic.roots().to_vec(),
        )
        .unwrap()
        .admit_exact_v29(SemanticMirLimitsV1::default())
        .unwrap();
        ProductionSemanticSsaOwnerV1::try_new(
            ProductionSemanticMirOwnerV1::try_new(
                admitted,
                ProductionSemanticMirLimitsV1::default(),
            )
            .unwrap(),
            ProductionSemanticSsaLimitsV1::default(),
        )
        .unwrap()
    }

    #[test]
    fn genuine_private_array_sources_cover_reachable_optional_attachments() {
        for variant in [
            PrivateArraySourceVariantV29::ConstantIndex,
            PrivateArraySourceVariantV29::LocalLiteral,
            PrivateArraySourceVariantV29::LocalCastLiteral,
        ] {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(SCHEDULE_LIMIT);
            let mut budget = ArgumentBudgetV1::new(&mut work, SCHEDULE_LIMIT);
            budget.reserve_storage(SCHEDULE_FLOOR).unwrap();
            let candidate = scalar_candidate_from_source_with_context_v29(
                || private_array_source_owner_v29(variant),
                ScopedTileOrderV29::Blocked,
                64,
                &mut budget,
                variant,
            );
            candidate.replay_with_budget(&mut budget)
                .unwrap_or_else(|error| panic!("{variant:?}: candidate source replay: {error:?}"));
            let witnesses = assert_sidecar_census(&candidate);
            // Two initializers of two scalar components, plus a write and a read
            // for each independently instantiated helper.
            assert_eq!(witnesses.private_literal, 4, "{variant:?}");
            assert_eq!(witnesses.offset_present, 8, "{variant:?}");
            assert_eq!(witnesses.offset_absent, 0, "{variant:?}");
            assert_eq!(witnesses.slot_count_present, 2, "{variant:?}");
            assert_eq!(witnesses.slot_count_absent, 2, "{variant:?}");
            let expected = match variant {
                PrivateArraySourceVariantV29::ConstantIndex => (4, 0, 0),
                PrivateArraySourceVariantV29::LocalLiteral => (0, 4, 0),
                PrivateArraySourceVariantV29::LocalCastLiteral => (0, 0, 4),
                PrivateArraySourceVariantV29::DynamicLocal
                | PrivateArraySourceVariantV29::UnboundedLocal => unreachable!(),
            };
            assert_eq!(
                (
                    witnesses.private_constant,
                    witnesses.private_local_direct,
                    witnesses.private_local_indirect,
                ),
                expected,
                "{variant:?}"
            );
            let retained = candidate.adopted_storage();
            drop(candidate);
            budget.release_storage(retained).unwrap();
            assert_eq!(budget.storage(), SCHEDULE_FLOOR);
            budget.release_storage(SCHEDULE_FLOOR).unwrap();
        }
    }

    #[test]
    fn genuine_bounded_dynamic_array_index_reaches_verified_candidate_and_replay() {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(SCHEDULE_LIMIT);
        let mut budget = ArgumentBudgetV1::new(&mut work, SCHEDULE_LIMIT);
        budget.reserve_storage(SCHEDULE_FLOOR).unwrap();
        let variant = PrivateArraySourceVariantV29::DynamicLocal;
        let candidate = scalar_candidate_from_source_with_context_v29(
            || private_array_source_owner_v29(variant),
            ScopedTileOrderV29::Blocked,
            64,
            &mut budget,
            variant,
        );
        candidate.replay_with_budget(&mut budget)
            .unwrap_or_else(|error| panic!("{variant:?}: candidate source replay: {error:?}"));
        let witnesses = assert_sidecar_census(&candidate);
        assert_eq!(witnesses.private_literal, 4);
        assert_eq!(witnesses.private_constant, 0);
        assert_eq!(witnesses.private_local_direct, 0);
        assert_eq!(witnesses.private_local_indirect, 4);
        assert_eq!(witnesses.offset_present, 4);
        assert_eq!(witnesses.offset_absent, 4);
        assert_eq!(witnesses.slot_count_present, 2);
        assert_eq!(witnesses.slot_count_absent, 2);
        let retained = candidate.adopted_storage();
        drop(candidate);
        budget.release_storage(retained).unwrap();
        assert_eq!(budget.storage(), SCHEDULE_FLOOR);
        budget.release_storage(SCHEDULE_FLOOR).unwrap();
    }

    #[test]
    fn genuine_unbounded_dynamic_array_index_has_no_candidate_authority() {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(SCHEDULE_LIMIT);
        let mut budget = ArgumentBudgetV1::new(&mut work, SCHEDULE_LIMIT);
        budget.reserve_storage(SCHEDULE_FLOOR).unwrap();
        let admitted = std::cell::Cell::new(0);
        let result = scalar_pending_from_source_v29(
            || {
                let owner = private_array_source_owner_v29(PrivateArraySourceVariantV29::UnboundedLocal);
                admitted.set(admitted.get() + 1);
                owner
            },
            64,
            &mut budget,
        );
        assert_eq!(admitted.get(), 2, "both original MIR/SSA owners must be admitted");
        match result {
            Err(ProductionPendingScopedSourceErrorV29::Source(
                ProductionSemanticKirErrorV1::Unsupported {
                    detail,
                    ..
                },
            )) => assert_eq!(
                detail,
                "scoped symbolic index lacks an exact current bound and success-edge proof"
            ),
            Err(error) => panic!("unexpected source boundary: {error:?}"),
            Ok(pending) => {
                let retained = pending.adopted_storage();
                drop(pending);
                budget.release_storage(retained).unwrap();
                panic!("out-of-range dynamic array offset acquired scoped candidate authority");
            }
        }
        assert_eq!(budget.storage(), SCHEDULE_FLOOR);
        budget.release_storage(SCHEDULE_FLOOR).unwrap();
    }
}
