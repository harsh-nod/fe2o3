// A retained IndexUse observes memory. Its payload is deliberately distinct
// from BaseUse and from a promoted SSA definition.
fn source_index_result_header_v29<T>() -> Result<usize, ArgumentResourceV1> {
    argument_sum_v1(&[
        std::mem::size_of::<Vec<T>>(),
        std::mem::size_of::<Result<Vec<T>, ProductionSemanticKirErrorV1>>(),
    ])
}

fn check_scoped_index_read_v29(
    function: &SemanticFunctionDeclV1,
    occurrences: &ProductionSemanticSsaFunctionOccurrencesV1<'_>,
    read: ScopedMemoryIndexReadV29,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<(), ProductionSemanticKirErrorV1> {
    budget.charge_work(14)?;
    let source = occurrences
        .owner()
        .source_semantic()
        .functions()
        .get(occurrences.function().index() as usize)
        .ok_or_else(scoped_memory_error_v29)?;
    let place = source_reference_selector_place_v29(function, read.site, read.role)
        .ok_or_else(scoped_memory_error_v29)?;
    let event = occurrences
        .events()
        .get(read.event)
        .ok_or_else(scoped_memory_error_v29)?;
    let local = function
        .locals()
        .get(read.local.index() as usize)
        .ok_or_else(scoped_memory_error_v29)?;
    if !std::ptr::eq(source, function)
        || !event.is_reachable()
        || event.is_promoted()
        || event.resolved().is_some()
        || event.site() != read.site
        || event.operand() != read.role
        || event.role() != ExecutionEventV29::ProjectionIndexUse(read.projection)
        || event.event().variable().get() != read.local.index()
        || local.ty() != read.ty
        || place
            .projections()
            .get(read.projection as usize)
            .map(|row| row.kind())
            != Some(SemanticProjectionKindV1::Index(read.local))
    {
        return Err(scoped_memory_error_v29());
    }
    let types = occurrences.owner().source_semantic().types();
    if !matches!(
        types
            .get(read.ty.index() as usize)
            .map(SemanticTypeDeclV1::shape),
        Some(SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
            signed: false,
            bits: 8 | 16 | 32 | 64
        }))
    ) {
        return Err(scoped_memory_error_v29());
    }
    Ok(())
}

fn source_retained_index_generation_v29(
    plan: &SourceReferencePlanV29<'_, '_>,
    instance: ProductionCallInstanceIdV1,
    read: ScopedMemoryIndexReadV29,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<u32, ProductionSemanticKirErrorV1> {
    budget.source_reference_owner_v29(plan)?;
    source_reference_owned_prepay_v29::<u32>(plan, budget)?;
    source_reference_owned_prepay_v29::<Option<(usize, SourceReferenceSelectorV29)>>(plan, budget)?;
    let original = plan
        .instances
        .instance(instance)
        .ok_or_else(scoped_memory_error_v29)?;
    let occurrences = plan
        .instances
        .occurrences(instance)
        .ok_or_else(scoped_memory_error_v29)?;
    check_scoped_index_read_v29(original.declaration(), &occurrences, read, budget)?;
    let source = source_reference_selector_place_v29(original.declaration(), read.site, read.role)
        .ok_or_else(scoped_memory_error_v29)?;
    let (_, selector) = plan
        .selector_at(
            instance,
            read.site,
            source,
            read.projection as usize,
            budget,
        )?
        .ok_or_else(scoped_memory_error_v29)?;
    budget.charge_work(4)?;
    if selector.local != read.local
        || selector.value != (SourceReferenceSelectorValueV29::Retained { event: read.event })
    {
        return Err(scoped_memory_error_v29());
    }
    selector
        .retained_generation
        .ok_or_else(scoped_memory_error_v29)
}

fn check_scoped_index_payload_v29(
    function: &SemanticFunctionDeclV1,
    occurrences: &ProductionSemanticSsaFunctionOccurrencesV1<'_>,
    anchor: &ScopedMemoryAnchorV29,
    operation: &Operation,
    result: ValueId,
    read: ScopedMemoryIndexReadV29,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<(), ProductionSemanticKirErrorV1> {
    check_scoped_index_read_v29(function, occurrences, read, budget)?;
    budget.charge_work(5)?;
    let [actual] = operation.results.as_slice() else {
        return Err(scoped_memory_error_v29());
    };
    if !matches!(
        operation.kind,
        OperationKind::Load { .. }
            | OperationKind::Storage(ScopedObjectOperationV29::ReadValue { .. })
    ) || actual.id != result
        || anchor.source != Some(ScopedMemoryFrameV29::operand(read.site, Some(read.role)))
        || !scoped_payload_type_matches_v29(
            occurrences.owner().source_semantic().types(),
            read.ty,
            &actual.ty,
            budget,
        )?
    {
        return Err(scoped_memory_error_v29());
    }
    Ok(())
}

fn scoped_original_index_payload_v29(
    anchors: &ScopedMemoryAnchorsV29,
    anchor: &ScopedMemoryAnchorV29,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<Option<(ValueId, ScopedMemoryIndexReadV29)>, ProductionSemanticKirErrorV1> {
    source_reference_emission_prepay_v29::<Option<(ValueId, ScopedMemoryIndexReadV29)>>(budget)?;
    budget.charge_work(2)?;
    match anchor.kind {
        ScopedMemoryAnchorKindV29::Access {
            payload: Some(ScopedMemoryPayloadV29::IndexLoad { result, read }),
            ..
        } => Ok(Some((result, read))),
        ScopedMemoryAnchorKindV29::Object(_) => {
            source_reference_emission_prepay_v29::<&ScopedObjectPayloadV29>(budget)?;
            let payload = anchors.object_payload(anchor, budget)?;
            match (payload.operation, payload.role, payload.result) {
                (
                    ScopedObjectOperationV29::ReadValue { .. },
                    ScopedObjectRoleV29::ReadValue {
                        source,
                        read: ScopedObjectReadOriginV29::ProjectionIndex(read),
                    },
                    Some(result),
                ) if source.source == ScopedObjectSourceV29::ProjectionIndex(read) => {
                    Ok(Some((result, read)))
                }
                _ => Ok(None),
            }
        }
        _ => Ok(None),
    }
}

impl SemanticFunctionLoweringV1<'_, '_> {
    fn with_retained_index_payload_v29<T>(
        &mut self,
        block: SemanticBlockIdV1,
        statement: Option<u32>,
        source: &SemanticPlaceV1,
        projection: usize,
        body: impl FnOnce(&mut Self) -> Result<T, ProductionSemanticKirErrorV1>,
    ) -> Result<T, ProductionSemanticKirErrorV1> {
        let header = argument_sum_v1(&[
            std::mem::size_of::<ScopedMemoryIndexReadV29>(),
            std::mem::size_of::<Option<(ScopedMemoryReadV29, bool)>>(),
            argument_product_v1(2, std::mem::size_of::<Option<ScopedMemoryIndexReadV29>>())?,
            std::mem::size_of::<
                Result<Option<ScopedMemoryIndexReadV29>, ProductionSemanticKirErrorV1>,
            >(),
        ])?;
        self.with_scoped_payload_header_v29(header, |this| {
            this.with_retained_index_payload_inner_v29(block, statement, source, projection, body)
        })
    }

    fn with_retained_index_payload_inner_v29<T>(
        &mut self,
        block: SemanticBlockIdV1,
        statement: Option<u32>,
        source: &SemanticPlaceV1,
        projection: usize,
        body: impl FnOnce(&mut Self) -> Result<T, ProductionSemanticKirErrorV1>,
    ) -> Result<T, ProductionSemanticKirErrorV1> {
        let read = self.with_emission_budget_v1(|this, budget| {
            let Some(cursor) = this.execution.as_ref() else {
                return Ok(None);
            };
            let Some(references) = cursor.references else {
                return Ok(None);
            };
            let instance = cursor.instance;
            let site = execution_site_v29(block, statement);
            let Some((_, row)) = references
                .plan
                .selector_at(instance, site, source, projection, budget)?
            else {
                return Ok(None);
            };
            let SourceReferenceSelectorValueV29::Retained { event } = row.value else {
                return Ok(None);
            };
            row.check(references.plan.instances, budget)?;
            cursor.check_ledger(budget)?;
            let original = cursor
                .occurrences
                .events()
                .get(event)
                .ok_or_else(scoped_memory_error_v29)?;
            let read = ScopedMemoryIndexReadV29 {
                site,
                role: original.operand(),
                projection: u32::try_from(projection)
                    .map_err(|_| ArgumentResourceV1::Arithmetic)?,
                local: row.local,
                ty: this
                    .function
                    .locals()
                    .get(row.local.index() as usize)
                    .ok_or_else(scoped_memory_error_v29)?
                    .ty(),
                event,
            };
            check_scoped_index_read_v29(this.function, &cursor.occurrences, read, budget)?;
            let recorder = this
                .scoped_memory
                .as_ref()
                .ok_or_else(scoped_memory_error_v29)?;
            if recorder.frame != Some(ScopedMemoryFrameV29::operand(site, Some(read.role)))
                || recorder.index_payload.is_some()
            {
                return Err(scoped_memory_error_v29());
            }
            Ok(Some(read))
        })?;
        let Some(read) = read else {
            return body(self);
        };
        {
            let recorder = self
                .scoped_memory
                .as_mut()
                .ok_or_else(scoped_memory_error_v29)?;
            let old_read = recorder.read_payload.take();
            let old_index = recorder.index_payload.replace(read);
            let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| body(self)));
            let restored = if let Some(recorder) = self.scoped_memory.as_mut() {
                recorder.read_payload = old_read;
                recorder.index_payload = old_index;
                true
            } else {
                false
            };
            match outcome {
                Err(payload) => std::panic::resume_unwind(payload),
                Ok(Err(error)) => Err(error),
                Ok(Ok(value)) if restored => Ok(value),
                Ok(Ok(_)) => Err(scoped_memory_error_v29()),
            }
        }
    }
}
// These fields survive only as replayed source/physical locators. Neither the
// row nor its presence proves the loaded value, a bound, or initializedness.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PendingSourceIndexV29 {
    instance: ProductionCallInstanceIdV1,
    event: usize,
    canonical: usize,
    load_anchor: Option<usize>,
    index_slot: Option<usize>,
    array_slot: usize,
    original: ValueId,
    scalar: ScalarType,
    length: u64,
    base: ValueId,
    offset: ValueId,
    pointer: ValueId,
    block: BlockId,
    operation: usize,
}

struct SourceIndexLocationV29 {
    source: PendingSourceIndexV29,
    load: Option<fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PendingSourceIndexGuardV29 {
    instance: ProductionCallInstanceIdV1,
    assertion: SemanticBlockIdV1,
    condition_event: usize,
    comparison_event: usize,
    load_anchor: usize,
    slot: usize,
    value: ValueId,
    scalar: ScalarType,
    length: u64,
    condition: ValueId,
    block: BlockId,
    success: BlockId,
    failure: BlockId,
}

struct SourceIndexGuardLocationV29 {
    source: PendingSourceIndexGuardV29,
    load: fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1,
}

fn source_index_constant_v29(
    types: &[SemanticTypeDeclV1],
    operand: &SemanticOperandV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<Option<u64>, ProductionSemanticKirErrorV1> {
    budget.charge_work(4)?;
    let SemanticOperandV1::Constant(constant) = operand else {
        return Ok(None);
    };
    let Some(SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
        signed: false,
        bits,
    })) = types
        .get(constant.ty().index() as usize)
        .map(SemanticTypeDeclV1::shape)
    else {
        return Ok(None);
    };
    let SemanticConstantValueV1::Scalar(value) = constant.value() else {
        return Ok(None);
    };
    if !matches!(*bits, 8 | 16 | 32 | 64) || u16::from(value.size_bytes()) * 8 != *bits {
        return Ok(None);
    }
    Ok(u64::try_from(value.bits()).ok())
}

// Re-read the original condition definition. Retained comparison operands have
// no SSA definition; their exact source read occurrence is joined separately.
fn source_index_guard_original_v29<'a>(
    function: &'a SemanticFunctionDeclV1,
    occurrences: &ProductionSemanticSsaFunctionOccurrencesV1<'_>,
    types: &[SemanticTypeDeclV1],
    assertion: SemanticBlockIdV1,
    condition_event: usize,
    comparison_event: usize,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(ExecutionSiteV29, &'a SemanticPlaceV1, u64, SsaValueV1), ProductionSemanticKirErrorV1>
{
    budget.charge_work(5)?;
    let SemanticTerminatorKindV1::Assert {
        expected: true,
        message: SemanticAssertMessageV1::BoundsCheck { length, .. },
        unwind,
        ..
    } = function
        .blocks()
        .get(assertion.index() as usize)
        .ok_or_else(source_raw_physical_error_v29)?
        .terminator()
        .kind()
    else {
        return Err(source_raw_physical_error_v29());
    };
    if matches!(unwind, SemanticUnwindActionV1::Cleanup(_)) {
        return Err(source_raw_physical_error_v29());
    }
    let length = source_index_constant_v29(types, length, budget)?
        .ok_or_else(source_raw_physical_error_v29)?;
    let site = ExecutionSiteV29::Terminator {
        block: SsaBlockIdV1::new(assertion.index()),
    };
    let condition = source_descriptor_use_v29(
        occurrences,
        condition_event,
        site,
        ExecutionOperandV29::AssertCondition,
        budget,
    )?;
    let (comparison, value) = source_descriptor_assignment_v29(
        function,
        occurrences,
        comparison_event,
        condition,
        budget,
    )?;
    let SemanticRvalueKindV1::Binary {
        operation: SemanticBinaryOpV1::LessThan,
        right,
        ..
    } = value
    else {
        return Err(source_raw_physical_error_v29());
    };
    if source_index_constant_v29(types, right, budget)? != Some(length) {
        return Err(source_raw_physical_error_v29());
    }
    let place = source_descriptor_whole_operand_v29(
        function,
        comparison,
        ExecutionOperandV29::RvalueOperand(0),
    )
    .ok_or_else(source_raw_physical_error_v29)?;
    // The failure diagnostic describes this exact original object too; its
    // read remains failure-only and never supplies successful-path memory.
    let diagnostic =
        source_descriptor_whole_operand_v29(function, site, ExecutionOperandV29::AssertMessage(1))
            .ok_or_else(source_raw_physical_error_v29)?;
    if diagnostic.local() != place.local() || diagnostic.ty() != place.ty() {
        return Err(source_raw_physical_error_v29());
    }
    Ok((comparison, place, length, condition))
}

fn source_index_guards_v29(
    instances: &ExecutionInstancesV29<'_>,
    source_index: &SourceAddressSourceIndexV29<'_>,
    slots: &OwnedScopedSourceSlotsV29,
    graph: &SourceAddressMemoryV29<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<Vec<PendingSourceIndexGuardV29>, ProductionSemanticKirErrorV1> {
    budget.reserve_storage(source_index_result_header_v29::<PendingSourceIndexGuardV29>()?)?;
    budget.reserve_storage(std::mem::size_of::<
        Result<&ScopedObjectPayloadV29, ProductionSemanticKirErrorV1>,
    >())?;
    let mut output = Vec::new();
    for sidecar in &source_index.pending.sidecars.rows {
        budget.charge_work(1)?;
        let instance = sidecar
            .source_call_instance
            .ok_or_else(source_raw_physical_error_v29)?;
        let original = instances
            .instance(instance)
            .ok_or_else(source_raw_physical_error_v29)?;
        let function = original.declaration();
        let occurrences = instances
            .occurrences(instance)
            .ok_or_else(source_raw_physical_error_v29)?;
        let capture = sidecar
            .instance_assert_origins
            .as_ref()
            .ok_or_else(source_raw_physical_error_v29)?;
        capture.check_identity(instances, instance, budget)?;
        let inventory = SourceDescriptorInventoryV29::build(&occurrences, budget)?;
        let anchors = sidecar
            .scoped_memory_anchors
            .as_ref()
            .ok_or_else(source_raw_physical_error_v29)?;
        let archive = sidecar
            .execution_observation
            .as_ref()
            .ok_or_else(source_raw_physical_error_v29)?;
        // One metered original statement index, not repeated anchor scans for
        // each guard. Keep original typed reads in the same census so a guard
        // cannot silently lose its still-unsupported typed comparison input.
        budget.reserve_storage(std::mem::size_of::<BTreeMap<(u32, u32), usize>>())?;
        let mut loads = BTreeMap::new();
        for (ordinal, anchor) in anchors.rows.iter().enumerate() {
            budget.charge_work(2)?;
            let read = match anchor.kind {
                ScopedMemoryAnchorKindV29::Access {
                    payload: Some(ScopedMemoryPayloadV29::Load { read, .. }),
                    ..
                } => read,
                ScopedMemoryAnchorKindV29::Object(_) => {
                    match anchors.object_payload(anchor, budget)?.role {
                        ScopedObjectRoleV29::ReadValue {
                            read: ScopedObjectReadOriginV29::Original(read),
                            ..
                        } => read,
                        _ => continue,
                    }
                }
                _ => continue,
            };
            if read.role != ExecutionOperandV29::RvalueOperand(0)
                || read.prefix != 0
                || !matches!(read.occurrence, ScopedMemoryOccurrenceV29::Retained { .. })
            {
                continue;
            }
            let (block, Some(statement)) = scoped_memory_site_key_v29(read.site) else {
                continue;
            };
            reserve_execution_cfg_map_entry_v29::<(u32, u32), usize>(loads.len(), budget)?;
            if loads.insert((block, statement), ordinal).is_some() {
                return Err(source_raw_physical_error_v29());
            }
        }
        for recorded in &capture.records {
            budget.charge_work(6)?;
            let assertion = recorded.site.semantic_block;
            let Some(block) = function.blocks().get(assertion.index() as usize) else {
                return Err(source_raw_physical_error_v29());
            };
            let SemanticTerminatorKindV1::Assert {
                expected: true,
                message: SemanticAssertMessageV1::BoundsCheck { length, .. },
                unwind,
                target,
                ..
            } = block.terminator().kind()
            else {
                continue;
            };
            if matches!(unwind, SemanticUnwindActionV1::Cleanup(_))
                || source_index_constant_v29(
                    instances.owner().source_semantic().types(),
                    length,
                    budget,
                )?
                .is_none()
            {
                continue;
            }
            let site = ExecutionSiteV29::Terminator {
                block: SsaBlockIdV1::new(assertion.index()),
            };
            let Some(condition_event) = inventory.use_at(
                &occurrences,
                site,
                ExecutionOperandV29::AssertCondition,
                budget,
            )?
            else {
                continue;
            };
            let condition = source_descriptor_use_v29(
                &occurrences,
                condition_event,
                site,
                ExecutionOperandV29::AssertCondition,
                budget,
            )?;
            let Some(comparison_event) = inventory.definition(condition, budget)? else {
                continue;
            };
            let (comparison, value) = source_descriptor_assignment_v29(
                function,
                &occurrences,
                comparison_event,
                condition,
                budget,
            )?;
            if !matches!(
                value,
                SemanticRvalueKindV1::Binary {
                    operation: SemanticBinaryOpV1::LessThan,
                    ..
                }
            ) {
                continue;
            }
            let (comparison_block, Some(comparison_statement)) =
                scoped_memory_site_key_v29(comparison)
            else {
                continue;
            };
            charge_execution_cfg_lookup_v29(loads.len(), budget)?;
            let Some(&load_anchor) = loads.get(&(comparison_block, comparison_statement)) else {
                continue;
            };
            if matches!(
                anchors.rows[load_anchor].kind,
                ScopedMemoryAnchorKindV29::Object(_)
            ) {
                return Err(scoped_object_pending_v29());
            }
            let (comparison, place, length, definition) = source_index_guard_original_v29(
                function,
                &occurrences,
                instances.owner().source_semantic().types(),
                assertion,
                condition_event,
                comparison_event,
                budget,
            )?;
            let ScopedMemoryAnchorKindV29::Access {
                payload:
                    Some(ScopedMemoryPayloadV29::Load {
                        result: value,
                        read,
                    }),
                ..
            } = anchors.rows[load_anchor].kind
            else {
                return Err(source_raw_physical_error_v29());
            };
            if read.site != comparison || read.ty != place.ty() {
                return Err(source_raw_physical_error_v29());
            }
            check_scoped_payload_occurrence_v29(
                &occurrences,
                comparison,
                ExecutionOperandV29::RvalueOperand(0),
                place,
                read.occurrence,
                budget,
            )?;
            let slot = source_address_original_slot_v29(
                instances,
                slots,
                instance,
                place.local(),
                place.ty(),
                budget,
            )?;
            let PrivateRetainedElementFactsV1::Scalar(scalar) =
                slots.slots[slot].scalar_array()?.element.element
            else {
                return Err(source_raw_physical_error_v29());
            };
            let PendingAssertOutcomeV1::Emitted { condition, failure } = recorded.outcome else {
                continue;
            };
            let SemanticValueBindingV1::Value { id, ty } =
                archive.lookup_original_v29(instances, instance, definition, budget)?
            else {
                return Err(source_raw_physical_error_v29());
            };
            if *id != condition
                || *ty != Type::BOOL
                || !recorded.expected
                || recorded.site.semantic_function != original.function()
                || recorded.block != capture.placement.block(assertion.index())?
                || recorded.physical_success != capture.placement.block(target.target().index())?
                || recorded.semantic_success != target.target()
            {
                return Err(source_raw_physical_error_v29());
            }
            let actual = graph.blocks[graph.block(recorded.block, budget)?].1;
            if !matches!(actual.terminator.as_ref(), Some(Terminator::ConditionalBranch {
                condition: actual, then_target, else_target, ..
            }) if *actual == condition && *then_target == recorded.physical_success && *else_target == failure
                && *then_target != *else_target)
            {
                return Err(source_raw_physical_error_v29());
            }
            let span = source_index.statement(
                SourceReferenceSiteV29 {
                    instance,
                    block: SemanticBlockIdV1::from_index(comparison_block),
                    statement: Some(comparison_statement as usize),
                },
                budget,
            )?;
            let mut found = false;
            for ordinal in span.first_operation_ordinal
                ..span
                    .first_operation_ordinal
                    .checked_add(span.operation_count)
                    .ok_or(ArgumentResourceV1::Arithmetic)?
            {
                budget.charge_work(1)?;
                let operation = source_address_original_operation_v29(
                    source_index.pending,
                    graph,
                    instance,
                    span.kernel_ir_block,
                    ordinal as usize,
                    budget,
                )?;
                budget.charge_work(operation.results.len())?;
                if operation
                    .results
                    .iter()
                    .any(|result| result.id == condition)
                {
                    if found
                        || !matches!(operation.kind, OperationKind::Compare {
                        predicate: ComparePredicate::LessThan, lhs, ..
                    } if lhs == value)
                    {
                        return Err(source_raw_physical_error_v29());
                    }
                    found = true;
                }
            }
            if !found {
                return Err(source_raw_physical_error_v29());
            }
            emission_push_v1(
                &mut output,
                PendingSourceIndexGuardV29 {
                    instance,
                    assertion,
                    condition_event,
                    comparison_event,
                    load_anchor,
                    slot,
                    value,
                    scalar,
                    length,
                    condition,
                    block: recorded.block,
                    success: recorded.physical_success,
                    failure,
                },
                budget,
            )?;
        }
        drop((inventory, loads));
    }
    Ok(output)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SourceIndexFailureV29 {
    instance: ProductionCallInstanceIdV1,
    anchor: usize,
    slot: usize,
    block: BlockId,
    gap: usize,
    move_after: bool,
}

include!("production_source_failure_history_v29.rs");

// A locator over the existing identity-sorted roster, not source authority.
fn source_failure_private_identity_v29<'a, T>(
    rows: &'a [T],
    local: u32,
    identity: impl Fn(&T) -> ScopedAllocationIdentityV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<Option<&'a T>, ProductionSemanticKirErrorV1> {
    for first in [
        ScopedAllocationIdentityV29::LegacyLocal(local),
        ScopedAllocationIdentityV29::OriginalObject {
            local,
            generation: 0,
        },
        ScopedAllocationIdentityV29::EntryValue { local, argument: 0 },
    ] {
        budget.charge_work(argument_sum_v1(&[
            call_splice_search_work_v1(rows.len()),
            1,
        ])?)?;
        let at = rows.partition_point(|row| identity(row) < first);
        let Some(row) = rows.get(at) else {
            continue;
        };
        let found = match identity(row) {
            ScopedAllocationIdentityV29::LegacyLocal(local)
            | ScopedAllocationIdentityV29::OriginalObject { local, .. }
            | ScopedAllocationIdentityV29::EntryValue { local, .. } => Some(local),
            _ => None,
        };
        if found == Some(local) {
            return Ok(Some(row));
        }
    }
    Ok(None)
}

#[cfg(test)]
#[test]
fn promoted_failure_identity_locator_covers_nonzero_generations_arguments_and_boundaries() {
    use ScopedAllocationIdentityV29 as I;
    let mut rows = [
        I::LegacyLocal(1),
        I::LegacyLocal(3),
        I::OriginalObject {
            local: 5,
            generation: 9,
        },
        I::EntryValue {
            local: 7,
            argument: 4,
        },
        I::OriginalObject {
            local: u32::MAX,
            generation: u32::MAX,
        },
    ];
    rows.sort_unstable();
    let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(10000);
    let mut budget = ArgumentBudgetV1::new(&mut work, 0);
    for (local, present) in [
        (0, false),
        (1, true),
        (2, false),
        (3, true),
        (4, false),
        (5, true),
        (6, false),
        (7, true),
        (8, false),
        (u32::MAX, true),
    ] {
        assert_eq!(
            source_failure_private_identity_v29(&rows, local, |row| *row, &mut budget)
                .unwrap()
                .is_some(),
            present
        );
    }
    assert_eq!(budget.storage(), 0);
}

fn source_failure_is_promoted_scalar_v29(
    instances: &ExecutionInstancesV29<'_>,
    source_index: &SourceAddressSourceIndexV29<'_>,
    slots: &OwnedScopedSourceSlotsV29,
    instance: ProductionCallInstanceIdV1,
    row: &ScopedMemoryAnchorV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<bool, ProductionSemanticKirErrorV1> {
    budget.charge_work(4)?;
    let original = instances
        .instance(instance)
        .ok_or_else(source_raw_physical_error_v29)?
        .declaration();
    let occurrences = instances
        .occurrences(instance)
        .ok_or_else(source_raw_physical_error_v29)?;
    let place = checked_scoped_failure_read_v29(original, &occurrences, row, budget)?;
    let ScopedMemoryAnchorKindV29::FailureRead { event, .. } = row.kind else {
        return Err(source_raw_physical_error_v29());
    };
    let frame = row.source.ok_or_else(source_raw_physical_error_v29)?;
    let Some(ScopedMemoryRoleV29::Operand(role)) = frame.role else {
        return Err(source_raw_physical_error_v29());
    };
    let event = occurrences
        .events()
        .get(event)
        .ok_or_else(source_raw_physical_error_v29)?;
    if !event.is_promoted()
        || event.resolved().is_none()
        || !matches!(
            scoped_source_operand_v29(original, frame.site, role),
            Some(SemanticOperandV1::Copy(_) | SemanticOperandV1::Move(_))
        )
        || !matches!(
            instances
                .owner()
                .source_semantic()
                .types()
                .get(place.ty().index() as usize)
                .map(|ty| ty.shape()),
            Some(SemanticTypeShapeV1::Scalar(_) | SemanticTypeShapeV1::ValidityScalar(_))
        )
    {
        return Ok(false);
    }
    if !place.projections().is_empty()
        && !source_failure_scalar_path_v43(
            original,
            instances.owner().source_semantic().types(),
            place,
            budget,
        )?
    {
        return Ok(false);
    }
    // A promoted diagnostic has no original private cell. Check the exact
    // instance interval, not the presence of unrelated helper allocations.
    budget.charge_work(call_splice_search_work_v1(slots.instances.len()))?;
    let owned = slots
        .instances
        .binary_search_by_key(&instance.index(), |owned| owned.instance.index())
        .ok()
        .and_then(|ordinal| slots.instances.get(ordinal))
        .ok_or_else(source_raw_physical_error_v29)?;
    if owned.function
        != instances
            .instance(instance)
            .ok_or_else(source_raw_physical_error_v29)?
            .function()
    {
        return Err(source_raw_physical_error_v29());
    }
    let rows = slots
        .slots
        .get(owned.slots.clone())
        .ok_or_else(source_raw_physical_error_v29)?;
    if let Some(slot) = source_failure_private_identity_v29(
        rows,
        place.local().index(),
        |slot| slot.origin.identity,
        budget,
    )? {
        if slot.instance != instance {
            return Err(source_raw_physical_error_v29());
        }
        return Ok(false);
    }
    source_index.frame_gap(instance, frame, row.block, row.position, budget)?;
    Ok(true)
}

fn source_index_failures_v29(
    instances: &ExecutionInstancesV29<'_>,
    plan: &SourceReferencePlanV29<'_, '_>,
    source_index: &SourceAddressSourceIndexV29<'_>,
    slots: &OwnedScopedSourceSlotsV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<Vec<SourceIndexFailureV29>, ProductionSemanticKirErrorV1> {
    plan.check_owner(instances, budget)?;
    budget.reserve_storage(source_index_result_header_v29::<SourceIndexFailureV29>()?)?;
    let mut output = Vec::new();
    for sidecar in &source_index.pending.sidecars.rows {
        budget.charge_work(1)?;
        let instance = sidecar
            .source_call_instance
            .ok_or_else(source_raw_physical_error_v29)?;
        let original = instances
            .instance(instance)
            .ok_or_else(source_raw_physical_error_v29)?
            .declaration();
        let occurrences = instances
            .occurrences(instance)
            .ok_or_else(source_raw_physical_error_v29)?;
        let anchors = sidecar
            .scoped_memory_anchors
            .as_ref()
            .ok_or_else(source_raw_physical_error_v29)?;
        for (anchor, row) in anchors.rows.iter().enumerate() {
            budget.charge_work(2)?;
            if !matches!(row.kind, ScopedMemoryAnchorKindV29::FailureRead { .. }) {
                continue;
            }
            if source_failure_is_promoted_scalar_v29(
                instances,
                source_index,
                slots,
                instance,
                row,
                budget,
            )? {
                continue;
            }
            let place = checked_scoped_failure_read_v29(original, &occurrences, row, budget)?;
            let frame = row.source.ok_or_else(source_raw_physical_error_v29)?;
            let (block, statement) = scoped_memory_site_key_v29(frame.site);
            if statement.is_some() || !place.projections().is_empty() {
                return Err(source_reference_error_v29(
                    "failure history requires an exact whole scalar diagnostic",
                ));
            }
            let site = SourceReferenceSiteV29 {
                instance,
                block: SemanticBlockIdV1::from_index(block),
                statement: None,
            };
            let slot = source_failure_scalar_slot_v29(instances, plan, slots, site, place, budget)?;
            let move_after = source_failure_operand_moved_v29(original, row, budget)?;
            source_index.frame_gap(instance, frame, row.block, row.position, budget)?;
            let mut mapping = ScopedEmittedPointsV29 {
                coordinates: &source_index.pending.coordinates,
                relocation: &source_index.pending.slot_relocation,
                budget,
            };
            let (block, gap) = mapping
                .emitted_point(
                    instance,
                    row.block,
                    u32::try_from(row.position).map_err(|_| ArgumentResourceV1::Arithmetic)?,
                    true,
                )
                .map_err(source_address_point_error_v29)?
                .ok_or_else(source_raw_physical_error_v29)?;
            emission_push_v1(
                &mut output,
                SourceIndexFailureV29 {
                    instance,
                    anchor,
                    slot,
                    block,
                    gap: gap as usize,
                    move_after,
                },
                budget,
            )?;
        }
    }
    call_splice_sort_work_v1(output.len(), budget).map_err(source_address_call_error_v29)?;
    output.sort_unstable_by_key(|row| (row.block, row.gap, row.instance.index(), row.anchor));
    Ok(output)
}

fn source_index_recipes_v29(
    instances: &ExecutionInstancesV29<'_>,
    references: &SourceReferenceEmissionV29<'_, '_>,
    source_index: &SourceAddressSourceIndexV29<'_>,
    slots: &OwnedScopedSourceSlotsV29,
    graph: &SourceAddressMemoryV29<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<Vec<PendingSourceIndexV29>, ProductionSemanticKirErrorV1> {
    references.plan.check_owner(instances, budget)?;
    references.check(budget)?;
    budget.reserve_storage(source_index_result_header_v29::<PendingSourceIndexV29>()?)?;
    let mut output = emission_vec_v1(references.plan.selectors.len(), budget)?;
    for (index, source) in references.plan.selectors.iter().enumerate() {
        let place = source.check(instances, budget)?;
        let used = references
            .selectors
            .get(index)
            .and_then(|row| row.get())
            .ok_or_else(source_raw_physical_error_v29)?;
        let SourceReferenceSelectorProducerV29::Address {
            base,
            offset,
            pointer,
            block,
            operation,
        } = used.producer
        else {
            if matches!(
                source.value,
                SourceReferenceSelectorValueV29::Retained { .. }
            ) {
                return Err(source_raw_physical_error_v29());
            }
            continue;
        };
        if source.projection != 0 || place.projections().len() != 1 {
            return Err(source_reference_error_v29(
                "retained source index needs its exact direct fixed-array backing",
            ));
        }
        let array_slot = source_address_original_slot_v29(
            instances,
            slots,
            source.instance,
            place.local(),
            source.array,
            budget,
        )?;
        let declaration = instances
            .instance(source.instance)
            .ok_or_else(source_raw_physical_error_v29)?
            .declaration();
        let sidecar = source_index.sidecar(source.instance, budget)?;
        let index_slot = match (source.value, used.memory) {
            (SourceReferenceSelectorValueV29::Promoted(_), None) => None,
            (SourceReferenceSelectorValueV29::Retained { event }, Some(anchor)) => {
                let anchors = sidecar
                    .scoped_memory_anchors
                    .as_ref()
                    .ok_or_else(source_raw_physical_error_v29)?;
                let row = anchors
                    .rows
                    .get(anchor)
                    .ok_or_else(source_raw_physical_error_v29)?;
                let (result, read) = scoped_original_index_payload_v29(anchors, row, budget)?
                    .ok_or_else(source_raw_physical_error_v29)?;
                let occurrences = instances
                    .occurrences(source.instance)
                    .ok_or_else(source_raw_physical_error_v29)?;
                let actual = source_address_original_operation_v29(
                    source_index.pending,
                    graph,
                    source.instance,
                    row.block,
                    row.position,
                    budget,
                )?;
                check_scoped_index_payload_v29(
                    declaration,
                    &occurrences,
                    row,
                    actual,
                    result,
                    read,
                    budget,
                )?;
                if result != used.original || read.event != event || read.local != source.local {
                    return Err(source_raw_physical_error_v29());
                }
                let slot = if let Some((endpoint, _)) =
                    source_address_object_payload_v29(anchors, row, budget)?
                {
                    source_address_object_index_v29(
                        instances,
                        references.plan,
                        slots,
                        source.instance,
                        read,
                        endpoint,
                        budget,
                    )?
                } else {
                    source_address_original_slot_v29(
                        instances,
                        slots,
                        source.instance,
                        read.local,
                        read.ty,
                        budget,
                    )?
                };
                Some(slot)
            }
            _ => return Err(source_raw_physical_error_v29()),
        };
        let actual = source_address_original_operation_v29(
            source_index.pending,
            graph,
            source.instance,
            block,
            operation,
            budget,
        )?;
        let slot = slots
            .slots
            .get(array_slot)
            .ok_or_else(source_raw_physical_error_v29)?;
        let scalar = slot.scalar_array()?;
        if !matches!(actual.kind, OperationKind::GetElementPointer { base: actual, offset: index }
            if actual == base && index == offset)
            || !matches!(actual.results.as_slice(), [result] if result.id == pointer)
            || slot.origin.pointer != base
            || scalar.length != used.length
        {
            return Err(source_raw_physical_error_v29());
        }
        graph.check_gep_type(actual, budget)?;
        let mut mapping = ScopedEmittedPointsV29 {
            coordinates: &source_index.pending.coordinates,
            relocation: &source_index.pending.slot_relocation,
            budget,
        };
        let (block, operation) = mapping
            .emitted_point(
                source.instance,
                block,
                u32::try_from(operation).map_err(|_| ArgumentResourceV1::Arithmetic)?,
                false,
            )
            .map_err(source_address_point_error_v29)?
            .ok_or_else(source_raw_physical_error_v29)?;
        output.push(PendingSourceIndexV29 {
            instance: source.instance,
            event: source.occurrence,
            canonical: source.canonical,
            load_anchor: used.memory,
            index_slot,
            array_slot,
            original: used.original,
            scalar: used.scalar,
            length: used.length,
            base,
            offset,
            pointer,
            block,
            operation: operation as usize,
        });
    }
    call_splice_sort_work_v1(output.len(), budget).map_err(source_address_call_error_v29)?;
    output.sort_unstable_by_key(|row| row.pointer);
    for pair in output.windows(2) {
        budget.charge_work(1)?;
        if pair[0].pointer >= pair[1].pointer {
            return Err(source_raw_physical_error_v29());
        }
    }
    Ok(output)
}
