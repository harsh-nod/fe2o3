// Non-inlined lds call bodies keep unrelated intrinsic temporaries off the dispatcher stack.
impl SemanticFunctionLoweringV1<'_> {
    #[inline(never)]
    fn lower_intrinsic_dynamic_lds_exact_v1(
        &mut self,
        block: SemanticBlockIdV1,
        call: &SemanticDirectCallV1,
        dynamic_lds: &SemanticTypeIdV1,
        element_storage: &SemanticTypeIdV1,
        elements: &u64,
        operations: &mut Vec<Operation>,
    ) -> Result<SemanticValueBindingV1, ProductionSemanticKirErrorV1> {
        Ok({
            self.require_call_argument_count(block, call, 1)?;
            // Post-borrow-check rustc MIR may reclassify this final use of
            // a non-Copy authority reference as `Copy`. Taking its local
            // binding below still consumes the compiler-issued authority.
            let (SemanticOperandV1::Copy(scope_place) | SemanticOperandV1::Move(scope_place)) =
                &call.arguments()[0]
            else {
                return Err(unsupported(
                    0,
                    Some(block.index()),
                    None,
                    "exact LDS scope authority must be transferred exactly once",
                ));
            };
            if !scope_place.projections().is_empty() {
                return Err(unsupported(
                    0,
                    Some(block.index()),
                    None,
                    "exact LDS scope authority has a projected carrier",
                ));
            }
            let scope_local = self.require_local(block, None, scope_place.local().index())?;
            let scope = self.locals[scope_local].take().ok_or(
                ProductionSemanticKirErrorV1::MissingLocalDefinition {
                    function: 0,
                    block: block.index(),
                    statement: None,
                    local: scope_place.local().index(),
                },
            )?;
            if !matches!(scope, SemanticValueBindingV1::WorkgroupLdsScope) {
                return Err(unsupported(
                    0,
                    Some(block.index()),
                    None,
                    "exact LDS allocation lacks compiler-authenticated scope authority",
                ));
            }
            let element = lower_dynamic_lds_element_type_v1(self.types, *element_storage)?;
            let storage = self
                .types
                .get(element_storage.index() as usize)
                .ok_or_else(|| {
                    unsupported(
                        0,
                        Some(block.index()),
                        None,
                        "exact LDS storage type is missing",
                    )
                })?;
            let element_size = storage.layout().size_bytes().ok_or_else(|| {
                unsupported(
                    0,
                    Some(block.index()),
                    None,
                    "exact LDS storage is dynamically sized",
                )
            })?;
            let byte_extent = elements.checked_mul(element_size).ok_or_else(|| {
                unsupported(
                    0,
                    Some(block.index()),
                    None,
                    "exact LDS byte extent overflows",
                )
            })?;
            let extent = u32::try_from(*elements).map_err(|_| {
                unsupported(
                    0,
                    Some(block.index()),
                    None,
                    "exact LDS element extent exceeds Kernel IR",
                )
            })?;
            let alignment = u32::try_from(storage.layout().alignment_bytes()).map_err(|_| {
                unsupported(
                    0,
                    Some(block.index()),
                    None,
                    "exact LDS alignment exceeds Kernel IR",
                )
            })?;
            let pointer_type = Type::pointer(
                element.clone(),
                AddressSpace::Workgroup,
                AccessMode::ReadWrite,
            );
            let pointer = self.emit(
                operations,
                pointer_type,
                OperationKind::WorkgroupMemory(WorkgroupMemory {
                    element,
                    extent: WorkgroupMemoryExtent::Static(extent),
                    alignment,
                }),
            )?;
            let len = self.emit(
                operations,
                Type::INDEX,
                OperationKind::Constant(Constant::Index(*elements)),
            )?;
            let byte_len = self.emit(
                operations,
                Type::INDEX,
                OperationKind::Constant(Constant::Index(byte_extent)),
            )?;
            let (base, base_ty) = pointer
                .value()
                .map_err(|detail| unsupported(0, Some(block.index()), None, detail))?;
            let (len, _) = len
                .value()
                .map_err(|detail| unsupported(0, Some(block.index()), None, detail))?;
            let (byte_len, _) = byte_len
                .value()
                .map_err(|detail| unsupported(0, Some(block.index()), None, detail))?;
            SemanticValueBindingV1::DynamicLds {
                base,
                base_ty,
                len,
                byte_len,
                dynamic_lds: *dynamic_lds,
                element_storage: *element_storage,
                elements: extent,
                byte_extent,
                alignment,
                producer_function: self.semantic_function,
                producer_block: block,
            }
        })
    }

    #[inline(never)]
    fn lower_intrinsic_dynamic_lds_raw_parts_v1(
        &mut self,
        block: SemanticBlockIdV1,
        call: &SemanticDirectCallV1,
        raw_parts: &SemanticTypeIdV1,
        element_storage: &SemanticTypeIdV1,
        element: &SemanticTypeIdV1,
    ) -> Result<SemanticValueBindingV1, ProductionSemanticKirErrorV1> {
        Ok({
            self.require_call_argument_count(block, call, 1)?;
            let SemanticOperandV1::Move(dynamic_lds_place) = &call.arguments()[0] else {
                return Err(unsupported(
                    0,
                    Some(block.index()),
                    None,
                    "collective LDS conversion must consume dynamic LDS exactly once",
                ));
            };
            if !dynamic_lds_place.projections().is_empty() {
                return Err(unsupported(
                    0,
                    Some(block.index()),
                    None,
                    "collective LDS conversion has a projected dynamic-LDS carrier",
                ));
            }
            let local = self.require_local(block, None, dynamic_lds_place.local().index())?;
            let dynamic_lds_binding = self.locals[local].take().ok_or(
                ProductionSemanticKirErrorV1::MissingLocalDefinition {
                    function: self.semantic_function.index(),
                    block: block.index(),
                    statement: None,
                    local: dynamic_lds_place.local().index(),
                },
            )?;
            let SemanticValueBindingV1::DynamicLds {
                base: pointer,
                base_ty: pointer_ty,
                len,
                byte_len,
                dynamic_lds,
                element_storage: issued_element_storage,
                elements,
                byte_extent,
                alignment,
                producer_function,
                producer_block,
            } = dynamic_lds_binding
            else {
                return Err(unsupported(
                    0,
                    Some(block.index()),
                    None,
                    "collective LDS conversion input is not compiler-issued dynamic LDS",
                ));
            };
            let storage_element = lower_dynamic_lds_element_type_v1(self.types, *element_storage)?;
            let semantic_element = lower_scalar_type(self.types, *element)?;
            let storage = self
                .types
                .get(element_storage.index() as usize)
                .ok_or_else(|| {
                    unsupported(
                        self.semantic_function.index(),
                        Some(block.index()),
                        None,
                        "collective LDS conversion storage type is missing",
                    )
                })?;
            let expected_byte_extent = u64::from(elements)
                .checked_mul(storage.layout().size_bytes().ok_or_else(|| {
                    unsupported(
                        self.semantic_function.index(),
                        Some(block.index()),
                        None,
                        "collective LDS conversion storage is dynamically sized",
                    )
                })?)
                .ok_or_else(|| {
                    unsupported(
                        self.semantic_function.index(),
                        Some(block.index()),
                        None,
                        "collective LDS conversion byte extent overflows",
                    )
                })?;
            let Type::Pointer(pointer_contract) = &pointer_ty else {
                return Err(unsupported(
                    0,
                    Some(block.index()),
                    None,
                    "collective LDS conversion input is not a lowered pointer",
                ));
            };
            if pointer_contract.address_space != AddressSpace::Workgroup
                || pointer_contract.access != AccessMode::ReadWrite
                || *pointer_contract.pointee != storage_element
                || storage_element != semantic_element
                || dynamic_lds != call.arguments()[0].ty()
                || issued_element_storage != *element_storage
                || producer_function != self.semantic_function
                || producer_block == block
                || !self
                    .enum_payload_dominance
                    .block_dominates(producer_block, block)
                || byte_extent != expected_byte_extent
                || u64::from(alignment) != storage.layout().alignment_bytes()
                || self.emitted_workgroup_memory_extents.get(&pointer).copied() != Some(elements)
                || self.emitted_unsigned_constants.get(&len).copied() != Some(u64::from(elements))
                || self.emitted_unsigned_constants.get(&byte_len).copied() != Some(byte_extent)
            {
                return Err(unsupported(
                    0,
                    Some(block.index()),
                    None,
                    "collective LDS conversion pointer, element, or length changed",
                ));
            }
            let values = [
                ValueDef::new(pointer, pointer_ty),
                ValueDef::new(len, Type::INDEX),
            ];
            binding_from_value_defs_with_validation(self.types, *raw_parts, &values, false)?
        })
    }

    #[inline(never)]
    #[allow(clippy::too_many_arguments)]
    fn lower_intrinsic_workgroup_pipeline_create_v1(
        &mut self,
        block: SemanticBlockIdV1,
        call: &SemanticDirectCallV1,
        pipeline: &SemanticTypeIdV1,
        buffers: &u32,
        elements: &u64,
        prefetch_distance: &u32,
        operations: &mut Vec<Operation>,
    ) -> Result<SemanticValueBindingV1, ProductionSemanticKirErrorV1> {
        Ok({
            self.require_call_argument_count(block, call, 1)?;
            let scope = self.lower_operand(block, None, &call.arguments()[0], operations)?;
            if !matches!(scope, SemanticValueBindingV1::WorkgroupLdsScope) {
                return Err(unsupported(
                    0,
                    Some(block.index()),
                    None,
                    "workgroup pipeline creation lacks compiler-authenticated LDS scope",
                ));
            }
            if !(2..=8).contains(buffers)
                || *elements == 0
                || *prefetch_distance == 0
                || prefetch_distance >= buffers
            {
                return Err(unsupported(
                    0,
                    Some(block.index()),
                    None,
                    "workgroup pipeline geometry is outside the executable contract",
                ));
            }
            let contract = self
                .workgroup_pipeline_contracts
                .get(pipeline)
                .cloned()
                .ok_or_else(|| {
                    unsupported(
                        0,
                        Some(block.index()),
                        None,
                        "workgroup pipeline has no consistent typed payload contract",
                    )
                })?;
            let extent = u64::from(*buffers)
                .checked_mul(*elements)
                .and_then(|extent| u32::try_from(extent).ok())
                .ok_or_else(|| {
                    unsupported(
                        0,
                        Some(block.index()),
                        None,
                        "workgroup pipeline LDS extent exceeds Kernel IR",
                    )
                })?;
            let pointer_type = Type::pointer(
                contract.packed_type.clone(),
                AddressSpace::Workgroup,
                AccessMode::ReadWrite,
            );
            let storage = self.emit_id(
                operations,
                pointer_type,
                OperationKind::WorkgroupMemory(WorkgroupMemory {
                    element: contract.packed_type.clone(),
                    extent: WorkgroupMemoryExtent::Static(extent),
                    alignment: contract.alignment,
                }),
            )?;
            SemanticValueBindingV1::WorkgroupPipeline {
                storage,
                pipeline: *pipeline,
                element: contract.element,
                payload_binding: contract.payload_binding,
                component_types: contract.component_types,
                packed_type: contract.packed_type,
                buffers: *buffers,
                elements: *elements,
                prefetch_distance: *prefetch_distance,
                alignment: contract.alignment,
            }
        })
    }

    #[inline(never)]
    fn lower_intrinsic_workgroup_pipeline_event_v1(
        &mut self,
        block: SemanticBlockIdV1,
        call: &SemanticDirectCallV1,
        pipeline: &SemanticTypeIdV1,
        event: &SemanticWorkgroupPipelineEventV1,
        operations: &mut Vec<Operation>,
    ) -> Result<SemanticValueBindingV1, ProductionSemanticKirErrorV1> {
        Ok({
            self.require_call_argument_count(block, call, 2)?;
            let receiver = self.lower_operand(block, None, &call.arguments()[0], operations)?;
            let SemanticValueBindingV1::WorkgroupPipeline {
                pipeline: actual_pipeline,
                buffers,
                elements,
                prefetch_distance,
                ..
            } = receiver
            else {
                return Err(unsupported(
                    0,
                    Some(block.index()),
                    None,
                    "workgroup pipeline event lacks its compiler-owned storage capability",
                ));
            };
            if actual_pipeline != *pipeline
                || !(2..=8).contains(&buffers)
                || elements == 0
                || prefetch_distance == 0
                || prefetch_distance >= buffers
            {
                return Err(unsupported(
                    0,
                    Some(block.index()),
                    None,
                    "workgroup pipeline event changed its authenticated contract",
                ));
            }
            let epoch = self.lower_operand(block, None, &call.arguments()[1], operations)?;
            let _ = self.coerce_index(block, operations, epoch)?;
            if *event == SemanticWorkgroupPipelineEventV1::Wait {
                self.emit_workgroup_pipeline_barrier(operations)?;
            }
            SemanticValueBindingV1::Unit
        })
    }

    #[inline(never)]
    fn lower_intrinsic_workgroup_pipeline_write_v1(
        &mut self,
        block: SemanticBlockIdV1,
        call: &SemanticDirectCallV1,
        pipeline: &SemanticTypeIdV1,
        element: &SemanticTypeIdV1,
        operations: &mut Vec<Operation>,
    ) -> Result<SemanticValueBindingV1, ProductionSemanticKirErrorV1> {
        Ok({
            self.require_call_argument_count(block, call, 4)?;
            let receiver = self.lower_operand(block, None, &call.arguments()[0], operations)?;
            let SemanticValueBindingV1::WorkgroupPipeline {
                storage,
                pipeline: actual_pipeline,
                element: actual_element,
                payload_binding,
                component_types,
                packed_type,
                buffers,
                elements,
                prefetch_distance: _,
                alignment,
            } = receiver
            else {
                return Err(unsupported(
                    0,
                    Some(block.index()),
                    None,
                    "workgroup pipeline write lacks its compiler-owned storage capability",
                ));
            };
            let contract = self
                .workgroup_pipeline_contracts
                .get(pipeline)
                .cloned()
                .ok_or_else(|| {
                    unsupported(
                        0,
                        Some(block.index()),
                        None,
                        "workgroup pipeline write has no typed payload contract",
                    )
                })?;
            if actual_pipeline != *pipeline
                || actual_element != *element
                || contract.element != *element
                || payload_binding != contract.payload_binding
                || component_types.as_ref() != contract.component_types.as_ref()
                || packed_type != contract.packed_type
                || alignment != contract.alignment
            {
                return Err(unsupported(
                    0,
                    Some(block.index()),
                    None,
                    "workgroup pipeline write changed its authenticated storage contract",
                ));
            }
            let slot = self.lower_workgroup_pipeline_slot(
                block,
                &call.arguments()[1],
                &call.arguments()[2],
                buffers,
                elements,
                operations,
            )?;
            let payload = self.lower_operand(block, None, &call.arguments()[3], operations)?;
            let packed =
                self.pack_workgroup_pipeline_payload(block, payload, &contract, operations)?;
            let pointer = self.emit_id(
                operations,
                Type::pointer(
                    contract.packed_type.clone(),
                    AddressSpace::Workgroup,
                    AccessMode::ReadWrite,
                ),
                OperationKind::GetElementPointer {
                    base: storage,
                    offset: slot,
                },
            )?;
            self.push_operation(operations, || {
                Operation::new(
                    Vec::new(),
                    OperationKind::Store {
                        pointer,
                        value: packed,
                        access: MemoryAccess::new(AddressSpace::Workgroup, alignment),
                    },
                )
            })?;
            SemanticValueBindingV1::Unit
        })
    }

    #[inline(never)]
    fn lower_intrinsic_workgroup_pipeline_read_v1(
        &mut self,
        block: SemanticBlockIdV1,
        call: &SemanticDirectCallV1,
        pipeline: &SemanticTypeIdV1,
        element: &SemanticTypeIdV1,
        operations: &mut Vec<Operation>,
    ) -> Result<SemanticValueBindingV1, ProductionSemanticKirErrorV1> {
        Ok({
            self.require_call_argument_count(block, call, 3)?;
            let receiver = self.lower_operand(block, None, &call.arguments()[0], operations)?;
            let SemanticValueBindingV1::WorkgroupPipeline {
                storage,
                pipeline: actual_pipeline,
                element: actual_element,
                payload_binding,
                component_types,
                packed_type,
                buffers,
                elements,
                prefetch_distance: _,
                alignment,
            } = receiver
            else {
                return Err(unsupported(
                    0,
                    Some(block.index()),
                    None,
                    "workgroup pipeline read lacks its compiler-owned storage capability",
                ));
            };
            let contract = self
                .workgroup_pipeline_contracts
                .get(pipeline)
                .cloned()
                .ok_or_else(|| {
                    unsupported(
                        0,
                        Some(block.index()),
                        None,
                        "workgroup pipeline read has no typed payload contract",
                    )
                })?;
            if actual_pipeline != *pipeline
                || actual_element != *element
                || contract.element != *element
                || payload_binding != contract.payload_binding
                || component_types.as_ref() != contract.component_types.as_ref()
                || packed_type != contract.packed_type
                || alignment != contract.alignment
            {
                return Err(unsupported(
                    0,
                    Some(block.index()),
                    None,
                    "workgroup pipeline read changed its authenticated storage contract",
                ));
            }
            let slot = self.lower_workgroup_pipeline_slot(
                block,
                &call.arguments()[1],
                &call.arguments()[2],
                buffers,
                elements,
                operations,
            )?;
            let pointer = self.emit_id(
                operations,
                Type::pointer(
                    contract.packed_type.clone(),
                    AddressSpace::Workgroup,
                    AccessMode::ReadWrite,
                ),
                OperationKind::GetElementPointer {
                    base: storage,
                    offset: slot,
                },
            )?;
            let packed = self.emit_id(
                operations,
                contract.packed_type.clone(),
                OperationKind::Load {
                    pointer,
                    access: MemoryAccess::new(AddressSpace::Workgroup, alignment),
                },
            )?;
            self.unpack_workgroup_pipeline_payload(block, packed, &contract, operations)?
        })
    }
}
