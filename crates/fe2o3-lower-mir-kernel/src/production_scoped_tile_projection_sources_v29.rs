// Retained sidecar fields join only original source evidence, never output plans.
impl TileAttachmentWalkV29<'_, '_, '_, '_> {
    fn sidecar_instance_v29(
        &mut self,
        ordinal: usize,
    ) -> TileAttachmentResultV29<ProductionCallInstanceIdV1> {
        self.budget.charge_work(3)?;
        let sidecar = self
            .root
            .sidecars
            .rows
            .get(ordinal)
            .ok_or(ScopedTileFailureKindV29::ReplayMismatch)?;
        let instance = sidecar
            .source_call_instance
            .ok_or(ScopedTileFailureKindV29::ReplayMismatch)?;
        let source = self
            .root
            .coordinates
            .sources
            .rows
            .get(instance.index())
            .ok_or(ScopedTileFailureKindV29::ReplayMismatch)?;
        if sidecar.source_call_instance != Some(source.instance)
            || self.root.active_instances.sidecar_ordinal(
                instance.index(),
                self.root.coordinates.sources.rows.len(),
                &self.root.sidecars.rows,
                self.budget,
            )? != Some(ordinal)
        {
            return Err(ScopedTileFailureKindV29::ReplayMismatch);
        }
        Ok(source.instance)
    }

    fn raw_span_v29(
        &mut self,
        key: TileAttachmentKeyV29,
        instance: ProductionCallInstanceIdV1,
        source: InstanceSpanSourceV1,
    ) -> TileAttachmentResultV29<()> {
        self.budget
            .charge_work(self.root.coordinates.spans.rows.len())?;
        let mut rows = self
            .root
            .coordinates
            .spans
            .rows
            .iter()
            .enumerate()
            .filter(|(_, row)| row.instance == instance && row.source == source);
        let (ordinal, _) = rows
            .next()
            .ok_or(ScopedTileFailureKindV29::ReplayMismatch)?;
        if rows.next().is_some() {
            return Err(ScopedTileFailureKindV29::ReplayMismatch);
        }
        self.mapped_span(key, ordinal)
    }

    fn raw_block_v29(
        &mut self,
        sidecar: &PendingInstanceSidecarsV29,
        source: SemanticBlockIdV1,
    ) -> TileAttachmentResultV29<BlockId> {
        self.budget.charge_work(sidecar.blocks.len())?;
        let mut rows = sidecar
            .blocks
            .iter()
            .filter(|row| row.semantic_block == source);
        let row = rows
            .next()
            .ok_or(ScopedTileFailureKindV29::ReplayMismatch)?;
        if rows.next().is_some() {
            return Err(ScopedTileFailureKindV29::ReplayMismatch);
        }
        Ok(row.kernel_ir_block)
    }

    fn walk_raw_sidecars_v29(&mut self) -> TileAttachmentResultV29<()> {
        use TileAttachmentFamilyV29 as Family;
        use TileAttachmentFieldV29 as Field;
        let root = self.root;
        for (instance_ordinal, sidecar) in root.sidecars.rows.iter().enumerate() {
            let instance = self.sidecar_instance_v29(instance_ordinal)?;
            let PendingInstanceSidecarsV29 {
                next_value: _,
                execution_observation: _,
                direct_call_inputs,
                source_call_instance: _,
                invocation_entry,
                scoped_slot_origins: _,
                scoped_initialization: _,
                scoped_memory_anchors: _,
                instance_assert_origins: _,
                lifecycle_events: _,
                private_arrays: _,
                operation_capabilities: _,
                diagnostic_declarations: _,
                float_declarations: _,
                blocks,
                statement_operation_spans,
                terminator_operation_spans,
                generated_terminator_values,
                call_returns,
                synthetic_operation_spans,
                parameter_bindings,
                parameter_component_bindings,
                ignored_parameter_bindings,
                emitted_operations: _,
            } = sidecar;
            self.budget
                .charge_work(size_of::<PendingInstanceSidecarsV29>())?;
            let mut row = 0;
            if let Some(invocation) = invocation_entry {
                let InvocationEntryRelationV1 {
                    subject: _,
                    layout,
                    span,
                    arguments,
                    components,
                    inputs_retained: _,
                    inputs,
                } = invocation;
                let preheader = layout
                    .preheader
                    .ok_or(ScopedTileFailureKindV29::ReplayMismatch)?;
                self.raw_span_v29(
                    self.key(
                        Family::RawSidecar,
                        instance.index(),
                        row,
                        Field::RawInvocationSpan,
                    ),
                    instance,
                    InstanceSpanSourceV1::InvocationEntry(*span),
                )?;
                self.block(
                    self.key(
                        Family::RawSidecar,
                        instance.index(),
                        row,
                        Field::RawInvocationPreheader,
                    ),
                    preheader,
                )?;
                self.block(
                    self.key(
                        Family::RawSidecar,
                        instance.index(),
                        row,
                        Field::RawInvocationEntry,
                    ),
                    layout.source_entry,
                )?;
                self.terminator(
                    self.key(
                        Family::RawSidecar,
                        instance.index(),
                        row,
                        Field::RawInvocationTerminator,
                    ),
                    preheader,
                )?;
                self.edge(
                    self.key(
                        Family::RawSidecar,
                        instance.index(),
                        row,
                        Field::RawInvocationEdge,
                    ),
                    preheader,
                    0,
                )?;
                row = argument_sum_v1(&[row, 1])?;
                for argument in arguments {
                    let InvocationArgumentRowV1 {
                        original: _,
                        first_component: _,
                        component_count: _,
                    } = *argument;
                    self.emit(
                        self.key(
                            Family::RawSidecar,
                            instance.index(),
                            row,
                            Field::RawInvocationArgument,
                        ),
                        TileAttachmentLocationV29::NoOutput,
                    )?;
                    row = argument_sum_v1(&[row, 1])?;
                }
                for input in inputs {
                    let InvocationInputRowV1 {
                        local: _,
                        ty: _,
                        source_argument: _,
                        tuple_field: _,
                        first_parameter: _,
                        parameter_count: _,
                        reference_call_transport: _,
                    } = *input;
                    self.emit(
                        self.key(
                            Family::RawSidecar,
                            instance.index(),
                            row,
                            Field::RawInvocationInputMap,
                        ),
                        TileAttachmentLocationV29::NoOutput,
                    )?;
                    row = argument_sum_v1(&[row, 1])?;
                }
                for component in components {
                    let InvocationComponentRowV1 {
                        original,
                        parameter,
                        transported,
                        conversion,
                    } = *component;
                    self.definition(
                        self.key(
                            Family::RawSidecar,
                            instance.index(),
                            row,
                            Field::RawInvocationInput,
                        ),
                        original,
                    )?;
                    self.definition(
                        self.key(
                            Family::RawSidecar,
                            instance.index(),
                            row,
                            Field::RawInvocationParameter,
                        ),
                        parameter,
                    )?;
                    self.definition(
                        self.key(
                            Family::RawSidecar,
                            instance.index(),
                            row,
                            Field::RawInvocationOutput,
                        ),
                        transported,
                    )?;
                    let key = self.key(
                        Family::RawSidecar,
                        instance.index(),
                        row,
                        Field::RawInvocationConversion,
                    );
                    match conversion {
                        Some(ordinal) => {
                            self.emitted_operation(key, instance, preheader, ordinal)?
                        }
                        None => self.emit(key, TileAttachmentLocationV29::NoOutput)?,
                    }
                    row = argument_sum_v1(&[row, 1])?;
                }
            }
            for input in direct_call_inputs.iter().flatten() {
                let InvocationInputRowV1 {
                    local: _,
                    ty: _,
                    source_argument: _,
                    tuple_field: _,
                    first_parameter: _,
                    parameter_count: _,
                    reference_call_transport: _,
                } = *input;
                self.emit(
                    self.key(
                        Family::RawSidecar,
                        instance.index(),
                        row,
                        Field::RawInvocationInputMap,
                    ),
                    TileAttachmentLocationV29::NoOutput,
                )?;
                row = argument_sum_v1(&[row, 1])?;
            }
            for entry in blocks {
                let SemanticKirBlockCorrespondenceV1 {
                    correspondence_owner: _,
                    semantic_function: _,
                    semantic_block: _,
                    kernel_ir_block,
                    source_statement_count: _,
                } = *entry;
                self.block(
                    self.key(Family::RawSidecar, instance.index(), row, Field::RawBlock),
                    kernel_ir_block,
                )?;
                row = argument_sum_v1(&[row, 1])?;
            }
            for &entry in statement_operation_spans {
                let SemanticKirStatementOperationSpanV1 {
                    correspondence_owner: _,
                    semantic_function: _,
                    semantic_block: _,
                    statement_ordinal: _,
                    kernel_ir_block: _,
                    first_operation_ordinal: _,
                    operation_count: _,
                } = entry;
                self.raw_span_v29(
                    self.key(
                        Family::RawSidecar,
                        instance.index(),
                        row,
                        Field::RawStatementSpan,
                    ),
                    instance,
                    InstanceSpanSourceV1::Statement(entry),
                )?;
                row = argument_sum_v1(&[row, 1])?;
            }
            for &entry in terminator_operation_spans {
                let SemanticKirTerminatorOperationSpanV1 {
                    correspondence_owner: _,
                    semantic_function: _,
                    semantic_block: _,
                    kernel_ir_block: _,
                    first_operation_ordinal: _,
                    operation_count: _,
                } = entry;
                self.raw_span_v29(
                    self.key(
                        Family::RawSidecar,
                        instance.index(),
                        row,
                        Field::RawTerminatorSpan,
                    ),
                    instance,
                    InstanceSpanSourceV1::Terminator(entry),
                )?;
                row = argument_sum_v1(&[row, 1])?;
            }
            for entry in generated_terminator_values {
                let SemanticKirGeneratedTerminatorValuesV1 {
                    correspondence_owner: _,
                    semantic_function: _,
                    semantic_block: _,
                    destination_local: _,
                    input,
                    output,
                } = *entry;
                self.definition(
                    self.key(
                        Family::RawSidecar,
                        instance.index(),
                        row,
                        Field::RawGeneratedInput,
                    ),
                    input,
                )?;
                self.definition(
                    self.key(
                        Family::RawSidecar,
                        instance.index(),
                        row,
                        Field::RawGeneratedOutput,
                    ),
                    output,
                )?;
                row = argument_sum_v1(&[row, 1])?;
            }
            let CallReturnBufferV1 {
                sites,
                components: _,
            } = call_returns;
            for &entry in &sites.rows {
                self.raw_call_v29(instance, row, sidecar, entry)?;
                row = argument_sum_v1(&[row, 1])?;
            }
            for &entry in synthetic_operation_spans {
                let SemanticKirSyntheticOperationSpanV1 {
                    correspondence_owner: _,
                    semantic_function: _,
                    rule: _,
                    kernel_ir_block: _,
                    first_operation_ordinal: _,
                    operation_count: _,
                } = entry;
                self.raw_span_v29(
                    self.key(
                        Family::RawSidecar,
                        instance.index(),
                        row,
                        Field::RawSyntheticSpan,
                    ),
                    instance,
                    InstanceSpanSourceV1::Synthetic(entry),
                )?;
                row = argument_sum_v1(&[row, 1])?;
            }
            for entry in parameter_bindings {
                let SemanticKirParameterBindingV1 {
                    correspondence_owner: _,
                    semantic_function: _,
                    semantic_local: _,
                    kernel_ir_value,
                } = *entry;
                self.definition(
                    self.key(
                        Family::RawSidecar,
                        instance.index(),
                        row,
                        Field::RawParameter,
                    ),
                    kernel_ir_value,
                )?;
                row = argument_sum_v1(&[row, 1])?;
            }
            for entry in parameter_component_bindings {
                let SemanticKirParameterComponentBindingV1 {
                    correspondence_owner: _,
                    semantic_function: _,
                    semantic_local: _,
                    semantic_component_type: _,
                    projection: _,
                    kernel_ir_value,
                } = entry;
                self.definition(
                    self.key(
                        Family::RawSidecar,
                        instance.index(),
                        row,
                        Field::RawParameterComponent,
                    ),
                    *kernel_ir_value,
                )?;
                row = argument_sum_v1(&[row, 1])?;
            }
            for entry in ignored_parameter_bindings {
                let SemanticKirIgnoredParameterBindingV1 {
                    correspondence_owner: _,
                    semantic_function: _,
                    semantic_local: _,
                    semantic_type: _,
                } = *entry;
                self.emit(
                    self.key(
                        Family::RawSidecar,
                        instance.index(),
                        row,
                        Field::RawIgnoredParameter,
                    ),
                    TileAttachmentLocationV29::NoOutput,
                )?;
                row = argument_sum_v1(&[row, 1])?;
            }
        }
        Ok(())
    }

    fn array_location_v29(
        &mut self,
        key: TileAttachmentKeyV29,
        instance: ProductionCallInstanceIdV1,
        location: PrivateArrayPhysicalLocationV1,
    ) -> TileAttachmentResultV29<()> {
        let PrivateArrayPhysicalLocationV1 {
            block_ordinal: _,
            block,
            operation,
        } = location;
        self.emitted_operation(key, instance, block, tile_attachment_u32_v29(operation)?)
    }

    fn optional_array_location_v29(
        &mut self,
        key: TileAttachmentKeyV29,
        instance: ProductionCallInstanceIdV1,
        location: Option<PrivateArrayPhysicalLocationV1>,
    ) -> TileAttachmentResultV29<()> {
        match location {
            Some(location) => self.array_location_v29(key, instance, location),
            None => self.emit(key, TileAttachmentLocationV29::NoOutput),
        }
    }

    fn walk_source_slots_v29(&mut self) -> TileAttachmentResultV29<()> {
        use TileAttachmentFamilyV29 as Family;
        use TileAttachmentFieldV29 as Field;
        let root = self.root;
        let OwnedScopedSourceSlotsV29 {
            source: _,
            ledger: _,
            instances,
            slots,
            pending_memory: _,
            retained_storage: _,
        } = &root.source_slots;
        if instances.len() != root.sidecars.rows.len() {
            return Err(ScopedTileFailureKindV29::ReplayMismatch);
        }
        for (instance_ordinal, slot_instance) in instances.iter().enumerate() {
            let ScopedSourceSlotInstanceV29 {
                instance,
                function: _,
                incoming: _,
                placement: _,
                slots: range,
            } = slot_instance;
            if *instance != self.sidecar_instance_v29(instance_ordinal)? {
                return Err(ScopedTileFailureKindV29::ReplayMismatch);
            }
            let mut row = 0;
            if let Some(origins) = &root.sidecars.rows[instance_ordinal].scoped_slot_origins {
                self.budget.charge_work(origins.len())?;
                for origin in origins {
                    let ScopedSlotOriginV29 {
                        identity: _,
                        source: _,
                        semantic_type: _,
                        pointer,
                    } = *origin;
                    self.definition(
                        self.key(
                            Family::SourceSlot,
                            instance.index(),
                            row,
                            Field::SlotRawPointer,
                        ),
                        pointer,
                    )?;
                    row = argument_sum_v1(&[row, 1])?;
                }
            }
            let slots = slots
                .get(range.clone())
                .ok_or(ScopedTileFailureKindV29::ReplayMismatch)?;
            self.budget.charge_work(slots.len())?;
            for slot in slots {
                let ScopedSourceSlotV29 {
                    instance: slot_owner,
                    origin,
                    representation,
                    allocation,
                } = *slot;
                let count = representation.count();
                let ScopedSlotOriginV29 {
                    identity: _,
                    source: _,
                    semantic_type: _,
                    pointer,
                } = origin;
                if slot_owner != *instance {
                    return Err(ScopedTileFailureKindV29::ReplayMismatch);
                }
                self.definition(
                    self.key(
                        Family::SourceSlot,
                        instance.index(),
                        row,
                        Field::SlotPointer,
                    ),
                    pointer,
                )?;
                let key = self.key(Family::SourceSlot, instance.index(), row, Field::SlotCount);
                match count {
                    Some((value, _)) => self.definition(key, value)?,
                    None => self.emit(key, TileAttachmentLocationV29::NoOutput)?,
                }
                self.optional_array_location_v29(
                    self.key(
                        Family::SourceSlot,
                        instance.index(),
                        row,
                        Field::SlotCountLocation,
                    ),
                    *instance,
                    count.map(|(_, p)| p),
                )?;
                self.array_location_v29(
                    self.key(
                        Family::SourceSlot,
                        instance.index(),
                        row,
                        Field::SlotAllocation,
                    ),
                    *instance,
                    allocation,
                )?;
                row = argument_sum_v1(&[row, 1])?;
            }
        }
        Ok(())
    }

    fn walk_memory_anchors_v29(&mut self) -> TileAttachmentResultV29<()> {
        use TileAttachmentFamilyV29 as Family;
        use TileAttachmentFieldV29 as Field;
        let root = self.root;
        for (instance_ordinal, sidecar) in root.sidecars.rows.iter().enumerate() {
            let instance = self.sidecar_instance_v29(instance_ordinal)?;
            let Some(ScopedMemoryAnchorsV29 {
                subject: _,
                placement: _,
                rows,
                objects,
                object_components: _,
                zero_objects,
            }) = &sidecar.scoped_memory_anchors
            else {
                continue;
            };
            self.budget.charge_work(rows.len())?;
            for (row, anchor) in rows.iter().enumerate() {
                let ScopedMemoryAnchorV29 {
                    block,
                    position,
                    source,
                    kind,
                } = *anchor;
                if let Some(ScopedMemoryFrameV29 { site: _, role }) = source {
                    match role {
                        Some(ScopedMemoryRoleV29::Operand(_))
                        | Some(ScopedMemoryRoleV29::CallResult)
                        | None => {}
                    }
                }
                let key = self.key(
                    Family::MemoryAnchor,
                    instance.index(),
                    row,
                    Field::MemoryPosition,
                );
                match kind {
                    ScopedMemoryAnchorKindV29::Object(_)
                    | ScopedMemoryAnchorKindV29::Access {
                        pointer: _,
                        payload: _,
                    } => {
                        self.emitted_operation(
                            key,
                            instance,
                            block,
                            tile_attachment_u32_v29(position)?,
                        )?;
                    }
                    ScopedMemoryAnchorKindV29::Kill {
                        event: _,
                        local: _,
                        cause: _,
                    }
                    | ScopedMemoryAnchorKindV29::FailureRead { event: _, local: _ } => {
                        self.emitted_gap(key, instance, block, tile_attachment_u32_v29(position)?)?;
                    }
                }
                let key = self.key(
                    Family::MemoryAnchor,
                    instance.index(),
                    row,
                    Field::MemoryPointer,
                );
                match kind {
                    ScopedMemoryAnchorKindV29::Access {
                        pointer,
                        payload: _,
                    } => self.definition(key, pointer)?,
                    ScopedMemoryAnchorKindV29::Object(_) => {
                        self.emit(key, TileAttachmentLocationV29::NoOutput)?
                    }
                    ScopedMemoryAnchorKindV29::Kill {
                        event: _,
                        local: _,
                        cause: _,
                    }
                    | ScopedMemoryAnchorKindV29::FailureRead { event: _, local: _ } => {
                        self.emit(key, TileAttachmentLocationV29::NoOutput)?;
                    }
                }
                for field in [
                    Field::MemoryLoadResult,
                    Field::MemoryStoreValue,
                    Field::MemoryStoreUse,
                ] {
                    let key = self.key(Family::MemoryAnchor, instance.index(), row, field);
                    match (field, kind) {
                        (
                            Field::MemoryLoadResult,
                            ScopedMemoryAnchorKindV29::Access {
                                payload:
                                    Some(
                                        ScopedMemoryPayloadV29::Load { result, read: _ }
                                        | ScopedMemoryPayloadV29::IndexLoad { result, read: _ },
                                    ),
                                ..
                            },
                        ) => self.definition(key, result)?,
                        (
                            Field::MemoryStoreValue,
                            ScopedMemoryAnchorKindV29::Access {
                                payload: Some(ScopedMemoryPayloadV29::Store { value, source: _ }),
                                ..
                            },
                        ) => self.definition(key, value)?,
                        (
                            Field::MemoryStoreUse,
                            ScopedMemoryAnchorKindV29::Access {
                                pointer,
                                payload: Some(ScopedMemoryPayloadV29::Store { value, source: _ }),
                            },
                        ) => self.emitted_store_payload_use_v18(
                            key,
                            instance,
                            block,
                            tile_attachment_u32_v29(position)?,
                            pointer,
                            value,
                        )?,
                        _ => self.emit(key, TileAttachmentLocationV29::NoOutput)?,
                    }
                }
                let object = match kind {
                    ScopedMemoryAnchorKindV29::Object(index) => Some(
                        objects
                            .get(index)
                            .ok_or(ScopedTileFailureKindV29::ReplayMismatch)?,
                    ),
                    ScopedMemoryAnchorKindV29::Access { .. }
                    | ScopedMemoryAnchorKindV29::Kill { .. }
                    | ScopedMemoryAnchorKindV29::FailureRead { .. } => None,
                };
                if let Some(object) = object {
                    self.check_emitted_object_v29(
                        instance,
                        block,
                        tile_attachment_u32_v29(position)?,
                        object,
                    )?;
                    // Keep both definitions and exact uses. A copy is not a
                    // single-pointer scalar access, even when its endpoints alias.
                    for (component, value) in object.operands().into_iter().enumerate() {
                        self.budget.charge_work(2)?;
                        let mut definition = self.key(
                            Family::MemoryAnchor,
                            instance.index(),
                            row,
                            Field::ObjectOperand,
                        );
                        definition.component = component;
                        let mut usage = definition;
                        usage.field = Field::ObjectOperandUse;
                        match value {
                            Some(value) => {
                                self.definition(definition, value)?;
                                self.emitted_use(
                                    usage,
                                    instance,
                                    block,
                                    tile_attachment_u32_v29(position)?,
                                    tile_attachment_u32_v29(component)?,
                                )?;
                            }
                            None => {
                                self.emit(definition, TileAttachmentLocationV29::NoOutput)?;
                                self.emit(usage, TileAttachmentLocationV29::NoOutput)?;
                            }
                        }
                    }
                    let key = self.key(
                        Family::MemoryAnchor,
                        instance.index(),
                        row,
                        Field::ObjectResult,
                    );
                    match object.result {
                        Some(value) => self.definition(key, value)?,
                        None => self.emit(key, TileAttachmentLocationV29::NoOutput)?,
                    }
                }
            }
            // Logical zero-component effects have positions, but no definitions
            // or uses. Keep their gap correspondence in the same complete walk.
            for (ordinal, zero) in zero_objects.iter().enumerate() {
                self.budget.charge_work(1)?;
                let key = self.key(
                    Family::MemoryAnchor,
                    instance.index(),
                    argument_sum_v1(&[rows.len(), ordinal])?,
                    Field::MemoryPosition,
                );
                self.emitted_gap(
                    key,
                    instance,
                    zero.block,
                    tile_attachment_u32_v29(zero.gap)?,
                )?;
            }
        }
        Ok(())
    }

    fn walk_private_arrays_v29(&mut self) -> TileAttachmentResultV29<()> {
        use TileAttachmentFamilyV29 as Family;
        use TileAttachmentFieldV29 as Field;
        let root = self.root;
        for (instance_ordinal, sidecar) in root.sidecars.rows.iter().enumerate() {
            let instance = self.sidecar_instance_v29(instance_ordinal)?;
            let PrivateArrayFunctionRowsV1 {
                active: _,
                placement: _,
                slots,
                effects,
                payload: _,
            } = &sidecar.private_arrays;
            self.budget
                .charge_work(argument_sum_v1(&[slots.len(), effects.len()])?)?;
            let mut row = 0;
            for slot in slots {
                let PrivateArraySlotV1 {
                    owner: _,
                    function: _,
                    local: _,
                    semantic_type: _,
                    count_location,
                    alloca_location,
                    pointer,
                    count,
                    length: _,
                    element_type: _,
                    element_facts: _,
                } = *slot;
                for (field, location) in [
                    (Field::ArrayCountLocation, count_location),
                    (Field::ArrayAllocation, alloca_location),
                ] {
                    self.array_location_v29(
                        self.key(Family::PrivateArray, instance.index(), row, field),
                        instance,
                        location,
                    )?;
                }
                for (field, value) in [(Field::ArrayPointer, pointer), (Field::ArrayCount, count)] {
                    self.definition(
                        self.key(Family::PrivateArray, instance.index(), row, field),
                        value,
                    )?;
                }
                row = argument_sum_v1(&[row, 1])?;
            }
            for effect in effects {
                let PrivateArrayEffectV1 {
                    owner: _,
                    function: _,
                    semantic_block,
                    semantic_statement: _,
                    role: _,
                    local: _,
                    semantic_type: _,
                    source_first_operation,
                    source_end_operation,
                    original_index,
                    offset_location,
                    gep_location,
                    memory_location,
                    offset,
                    gep,
                    access: _,
                } = *effect;
                let block =
                    self.raw_block_v29(sidecar, SemanticBlockIdV1::from_index(semantic_block))?;
                self.emitted_range(
                    self.key(
                        Family::PrivateArray,
                        instance.index(),
                        row,
                        Field::ArraySourceRange,
                    ),
                    instance,
                    block,
                    tile_attachment_u32_v29(source_first_operation)?,
                    tile_attachment_u32_v29(source_end_operation)?,
                )?;
                let (original, direct, literal, literal_definition) = match original_index {
                    PrivateArrayIndexV1::ConstantIndex {
                        offset: _,
                        min_length: _,
                        from_end: _,
                    } => (None, None, None, None),
                    PrivateArrayIndexV1::Local {
                        local: _,
                        semantic_type: _,
                        original,
                        physical_type: _,
                        direct_definition,
                    } => (Some(original), direct_definition, None, None),
                    PrivateArrayIndexV1::InitializerElement {
                        component: _,
                        value: PrivateArrayInitializerValueV1::LiteralScalar { value, definition },
                    } => (None, None, Some(value), Some(definition)),
                };
                let key = self.key(
                    Family::PrivateArray,
                    instance.index(),
                    row,
                    Field::ArrayOriginalIndex,
                );
                match original {
                    Some(value) => self.definition(key, value)?,
                    None => self.emit(key, TileAttachmentLocationV29::NoOutput)?,
                }
                self.optional_array_location_v29(
                    self.key(
                        Family::PrivateArray,
                        instance.index(),
                        row,
                        Field::ArrayDirectDefinition,
                    ),
                    instance,
                    direct,
                )?;
                let key = self.key(
                    Family::PrivateArray,
                    instance.index(),
                    row,
                    Field::ArrayLiteralValue,
                );
                match literal {
                    Some(value) => self.definition(key, value)?,
                    None => self.emit(key, TileAttachmentLocationV29::NoOutput)?,
                }
                self.optional_array_location_v29(
                    self.key(
                        Family::PrivateArray,
                        instance.index(),
                        row,
                        Field::ArrayLiteralDefinition,
                    ),
                    instance,
                    literal_definition,
                )?;
                self.optional_array_location_v29(
                    self.key(
                        Family::PrivateArray,
                        instance.index(),
                        row,
                        Field::ArrayOffsetLocation,
                    ),
                    instance,
                    offset_location,
                )?;
                self.array_location_v29(
                    self.key(
                        Family::PrivateArray,
                        instance.index(),
                        row,
                        Field::ArrayGepLocation,
                    ),
                    instance,
                    gep_location,
                )?;
                self.array_location_v29(
                    self.key(
                        Family::PrivateArray,
                        instance.index(),
                        row,
                        Field::ArrayMemoryLocation,
                    ),
                    instance,
                    memory_location,
                )?;
                self.definition(
                    self.key(
                        Family::PrivateArray,
                        instance.index(),
                        row,
                        Field::ArrayOffset,
                    ),
                    offset,
                )?;
                self.definition(
                    self.key(Family::PrivateArray, instance.index(), row, Field::ArrayGep),
                    gep,
                )?;
                row = argument_sum_v1(&[row, 1])?;
            }
        }
        Ok(())
    }

    fn walk_lifecycle_attachments_v29(&mut self) -> TileAttachmentResultV29<()> {
        use TileAttachmentFamilyV29 as Family;
        use TileAttachmentFieldV29 as Field;
        let root = self.root;
        let mut event_count = 0;
        for (instance_ordinal, sidecar) in root.sidecars.rows.iter().enumerate() {
            let instance = self.sidecar_instance_v29(instance_ordinal)?;
            let Some(PendingLifecycleEventsV29 {
                ledger: _,
                source: _,
                instance: event_instance,
                function: _,
                placement: _,
                provider: _,
                expected_rows: _,
                rows,
                retained_storage: _,
            }) = &sidecar.lifecycle_events
            else {
                continue;
            };
            if *event_instance != instance {
                return Err(ScopedTileFailureKindV29::ReplayMismatch);
            }
            for (row, event) in rows.iter().enumerate() {
                let DeferredLifecycleEventV29 {
                    source,
                    block: _,
                    original_block,
                    original_gap,
                    kind,
                } = *event;
                match source {
                    DeferredLifecycleSourceV29::Issuance { root: _ }
                    | DeferredLifecycleSourceV29::Derive { event: _ }
                    | DeferredLifecycleSourceV29::Return { event: _ }
                    | DeferredLifecycleSourceV29::Intrinsic { callee: _ } => {}
                }
                match kind {
                    DeferredLifecycleKindV29::Issue { result: _ }
                    | DeferredLifecycleKindV29::Derive {
                        context: _,
                        result: _,
                    }
                    | DeferredLifecycleKindV29::End { workgroup: _ } => {}
                    DeferredLifecycleKindV29::Tile(DeferredTileEventV29 {
                        input,
                        producer: _,
                        result_type: _,
                        first_result: _,
                        lanes: _,
                        elements: _,
                    }) => match input {
                        DeferredTileInputV29::Load {
                            workgroup: _,
                            input: _,
                            base: _,
                        }
                        | DeferredTileInputV29::Fragment { tile: _ }
                        | DeferredTileInputV29::Parts { fragment: _ } => {}
                    },
                }
                self.budget.charge_work(root.insertions.len())?;
                let mut insertions = root
                    .insertions
                    .iter()
                    .filter(|insertion| insertion.instance == instance && insertion.event == row);
                let insertion = insertions
                    .next()
                    .ok_or(ScopedTileFailureKindV29::ReplayMismatch)?;
                if insertions.next().is_some() {
                    return Err(ScopedTileFailureKindV29::ReplayMismatch);
                }
                let LifecycleInsertionV29 {
                    instance: _,
                    event: _,
                    source_span: _,
                    before,
                    after,
                } = *insertion;
                if before.count != 0 || after.count != 1 {
                    return Err(ScopedTileFailureKindV29::ReplayMismatch);
                }
                self.emitted_gap(
                    self.key(
                        Family::Lifecycle,
                        instance.index(),
                        row,
                        Field::LifecycleOriginalGap,
                    ),
                    instance,
                    original_block,
                    original_gap,
                )?;
                self.gap_c(
                    self.key(
                        Family::Lifecycle,
                        instance.index(),
                        row,
                        Field::LifecycleBeforeGap,
                    ),
                    before.block,
                    before.first,
                )?;
                self.operation_v(
                    self.key(
                        Family::Lifecycle,
                        instance.index(),
                        row,
                        Field::LifecycleOperation,
                    ),
                    after.block,
                    after.first,
                )?;
                let (_, block) = tile_attachment_block_v29(
                    self.graph,
                    self.index,
                    root.function_ordinal,
                    after.block,
                    self.budget,
                )?;
                let operation = block
                    .operations
                    .get(after.first as usize)
                    .ok_or(ScopedTileFailureKindV29::ReplayMismatch)?;
                let OperationKind::Execution(execution) = &operation.kind else {
                    return Err(ScopedTileFailureKindV29::ReplayMismatch);
                };
                let key = self.key(
                    Family::Lifecycle,
                    instance.index(),
                    row,
                    Field::LifecycleOperand,
                );
                let mut component = 0;
                execution.try_visit_operands_v1(|value| {
                    self.definition(tile_attachment_component_v29(key, component), value)?;
                    component = argument_sum_v1(&[component, 1])?;
                    Ok::<_, ScopedTileFailureKindV29>(())
                })?;
                if component == 0 {
                    self.emit(key, TileAttachmentLocationV29::NoOutput)?;
                }
                let key = self.key(
                    Family::Lifecycle,
                    instance.index(),
                    row,
                    Field::LifecycleResult,
                );
                if operation.results.is_empty() {
                    self.emit(key, TileAttachmentLocationV29::NoOutput)?;
                }
                for (component, result) in operation.results.iter().enumerate() {
                    self.definition(tile_attachment_component_v29(key, component), result.id)?;
                }
                event_count = argument_sum_v1(&[event_count, 1])?;
            }
        }
        if event_count != root.insertions.len() {
            return Err(ScopedTileFailureKindV29::ReplayMismatch);
        }
        Ok(())
    }

    fn walk_assertion_attachments_v29(&mut self) -> TileAttachmentResultV29<()> {
        use TileAttachmentFamilyV29 as Family;
        use TileAttachmentFieldV29 as Field;
        let root = self.root;
        let assertions = &self.pending.inner.assertions;
        let mut matched = 0;
        for (instance_ordinal, sidecar) in root.sidecars.rows.iter().enumerate() {
            let instance = self.sidecar_instance_v29(instance_ordinal)?;
            let Some(InstanceAssertCaptureV1 {
                ledger: _,
                source: _,
                instance: capture_instance,
                function: _,
                placement: _,
                records,
                arguments,
                storage: _,
                failed: _,
            }) = &sidecar.instance_assert_origins
            else {
                continue;
            };
            if *capture_instance != instance {
                return Err(ScopedTileFailureKindV29::ReplayMismatch);
            }
            let mut argument_end = 0;
            for (row, capture) in records.iter().enumerate() {
                let PendingAssertOriginV1 {
                    site,
                    emitted_function: _,
                    block,
                    first_operation,
                    operation_count,
                    expected: _,
                    semantic_success: _,
                    physical_success,
                    argument_start,
                    argument_count,
                    outcome,
                } = capture;
                self.budget.charge_work(assertions.len())?;
                let mut bindings = assertions.iter().filter(|binding| {
                    binding.instance == instance
                        && binding.site == *site
                        && binding.binding.block().function.0 as usize == root.function_ordinal
                });
                let ReplayedInstanceAssertV1 {
                    instance: _,
                    site: _,
                    binding,
                } = *bindings
                    .next()
                    .ok_or(ScopedTileFailureKindV29::ReplayMismatch)?;
                if bindings.next().is_some()
                    || binding.block().function.0 as usize != root.function_ordinal
                    || *argument_start != argument_end
                {
                    return Err(ScopedTileFailureKindV29::ReplayMismatch);
                }
                argument_end = argument_sum_v1(&[*argument_start, *argument_count])?;
                let captured = arguments
                    .get(*argument_start..argument_end)
                    .ok_or(ScopedTileFailureKindV29::ReplayMismatch)?;
                let end = first_operation
                    .checked_add(*operation_count)
                    .ok_or(ArgumentResourceV1::Arithmetic)?;
                self.emitted_range(
                    self.key(
                        Family::Assertion,
                        instance.index(),
                        row,
                        Field::AssertSourceRange,
                    ),
                    instance,
                    *block,
                    *first_operation,
                    end,
                )?;
                self.block(
                    self.key(
                        Family::Assertion,
                        instance.index(),
                        row,
                        Field::AssertSourceBlock,
                    ),
                    *block,
                )?;
                self.block(
                    self.key(
                        Family::Assertion,
                        instance.index(),
                        row,
                        Field::AssertSuccessBlock,
                    ),
                    *physical_success,
                )?;
                let key = self.key(
                    Family::Assertion,
                    instance.index(),
                    row,
                    Field::AssertFailureBlock,
                );
                match *outcome {
                    PendingAssertOutcomeV1::Emitted {
                        condition: _,
                        failure,
                    } => {
                        let failure = checked_terminal_assertion_v18(
                            root.terminal_failures.as_ref(),
                            instance,
                            capture,
                            self.budget,
                        )?
                        .map_or(failure, |row| row.actual_failure);
                        self.block(key, failure)?;
                    }
                    PendingAssertOutcomeV1::ElidedByExistingRule => {
                        self.emit(key, TileAttachmentLocationV29::NoOutput)?
                    }
                }
                let key = self.key(
                    Family::Assertion,
                    instance.index(),
                    row,
                    Field::AssertCapturedCondition,
                );
                match *outcome {
                    PendingAssertOutcomeV1::Emitted {
                        condition,
                        failure: _,
                    } => self.definition(key, condition)?,
                    PendingAssertOutcomeV1::ElidedByExistingRule => {
                        self.emit(key, TileAttachmentLocationV29::NoOutput)?
                    }
                }
                let key = self.key(
                    Family::Assertion,
                    instance.index(),
                    row,
                    Field::AssertCapturedArgument,
                );
                if captured.is_empty() {
                    self.emit(key, TileAttachmentLocationV29::NoOutput)?;
                }
                for (component, &value) in captured.iter().enumerate() {
                    self.definition(tile_attachment_component_v29(key, component), value)?;
                }
                let SemanticKirAssertConditionBindingV1 {
                    expected: _,
                    semantic_success: _,
                    outcome,
                } = binding;
                let use_key = self.key(
                    Family::Assertion,
                    instance.index(),
                    row,
                    Field::AssertConditionUse,
                );
                let definition_key = self.key(
                    Family::Assertion,
                    instance.index(),
                    row,
                    Field::AssertConditionDefinition,
                );
                let success_key = self.key(
                    Family::Assertion,
                    instance.index(),
                    row,
                    Field::AssertSuccessEdge,
                );
                let failure_key = self.key(
                    Family::Assertion,
                    instance.index(),
                    row,
                    Field::AssertFailureEdge,
                );
                let success = match outcome {
                    SemanticKirAssertConditionOutcomeV1::Emitted {
                        condition_use,
                        definition,
                        success_edge,
                        failure_edge,
                    } => {
                        self.canonical_use(use_key, condition_use)?;
                        self.canonical_definition(definition_key, definition)?;
                        self.canonical_edge(success_key, success_edge)?;
                        self.canonical_edge(failure_key, failure_edge)?;
                        success_edge
                    }
                    SemanticKirAssertConditionOutcomeV1::ElidedByExistingRule { success_edge } => {
                        self.emit(use_key, TileAttachmentLocationV29::NoOutput)?;
                        self.emit(definition_key, TileAttachmentLocationV29::NoOutput)?;
                        self.canonical_edge(success_key, success_edge)?;
                        self.emit(failure_key, TileAttachmentLocationV29::NoOutput)?;
                        success_edge
                    }
                };
                let key = self.key(
                    Family::Assertion,
                    instance.index(),
                    row,
                    Field::AssertSuccessArgument,
                );
                if captured.is_empty() {
                    self.emit(key, TileAttachmentLocationV29::NoOutput)?;
                }
                for component in 0..captured.len() {
                    self.canonical_edge_argument(
                        tile_attachment_component_v29(key, component),
                        AttachmentEdgeArgumentV29 {
                            edge: success,
                            argument: tile_attachment_u32_v29(component)?,
                        },
                    )?;
                }
                matched = argument_sum_v1(&[matched, 1])?;
            }
            if argument_end != arguments.len() {
                return Err(ScopedTileFailureKindV29::ReplayMismatch);
            }
        }
        self.budget.charge_work(assertions.len())?;
        let expected = assertions
            .iter()
            .filter(|row| row.binding.block().function.0 as usize == root.function_ordinal)
            .count();
        if matched != expected {
            return Err(ScopedTileFailureKindV29::ReplayMismatch);
        }
        Ok(())
    }

    fn walk_sidecars_v29(&mut self) -> TileAttachmentResultV29<()> {
        self.walk_raw_sidecars_v29()?;
        self.walk_source_slots_v29()?;
        self.walk_memory_anchors_v29()?;
        self.walk_private_arrays_v29()?;
        self.walk_lifecycle_attachments_v29()?;
        self.walk_assertion_attachments_v29()?;
        self.walk_terminal_failure_attachments_v18()
    }
}

include!("production_terminal_failure_attachments_v18.rs");
