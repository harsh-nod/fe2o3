// Retained arrays remain private memory. The element type is a modeled KIR
// scalar or thin pointer, never a synthetic aggregate KIR type.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SemanticRetainedArrayLayoutV1 {
    element: SemanticTypeIdV1,
    length: u64,
}

fn retained_array_slot_plan_v1(
    types: &[SemanticTypeDeclV1],
    ty: SemanticTypeIdV1,
    max_elements: usize,
) -> Result<SemanticRetainedLocalSlotPlanV1, ProductionSemanticKirErrorV1> {
    let declaration = types
        .get(ty.index() as usize)
        .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
    let SemanticTypeShapeV1::Array { element, length } = declaration.shape() else {
        return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
    };
    let Some((kernel_type, alignment)) = retained_local_slot_type_v1(types, *element) else {
        return Err(unsupported(
            0,
            None,
            None,
            "retained array element is not an exact storable scalar or metadata-free pointer",
        ));
    };
    let layout = declaration.layout();
    let SemanticFieldsShapeV1::Array {
        stride_bytes,
        count,
    } = layout.fields()
    else {
        return Err(unsupported(
            0,
            None,
            None,
            "retained array lacks exact rustc field-stride evidence",
        ));
    };
    let element_size = types[element.index() as usize].layout().size_bytes();
    let size = stride_bytes
        .checked_mul(*length)
        .ok_or_else(|| unsupported(0, None, None, "retained array byte extent overflows"))?;
    if *length == 0
        || *count != *length
        || element_size != Some(*stride_bytes)
        || layout.size_bytes() != Some(size)
        || layout.alignment_bytes() != u64::from(alignment)
        || layout.is_uninhabited()
        || size > i64::MAX as u64
    {
        return Err(unsupported(
            0,
            None,
            None,
            "retained array has no exact nonempty fixed element layout",
        ));
    }
    let elements = usize::try_from(*length).map_err(|_| {
        unsupported(
            0,
            None,
            None,
            "retained array element count does not fit this host",
        )
    })?;
    enforce_limit(
        ProductionSemanticKirResourceV1::Operations,
        elements,
        max_elements,
    )?;
    Ok(SemanticRetainedLocalSlotPlanV1 {
        semantic_type: ty,
        kernel_type,
        alignment,
        array: Some(SemanticRetainedArrayLayoutV1 {
            element: *element,
            length: *length,
        }),
    })
}

fn retained_array_expansion_v1(
    length: u64,
    emitted_operations: usize,
    block_operations: usize,
    max_operations: usize,
) -> Result<usize, ProductionSemanticKirErrorV1> {
    let length = usize::try_from(length).map_err(|_| {
        unsupported(
            0,
            None,
            None,
            "retained array element count does not fit this host",
        )
    })?;
    // One Index constant, one GEP, and one memory operation per element.
    // Check the expansion before allocating the aggregate result buffer.
    let count = length
        .checked_mul(3)
        .ok_or_else(|| unsupported(0, None, None, "retained array operation count overflows"))?;
    let total = emitted_operations.checked_add(count).ok_or(
        ProductionSemanticKirErrorV1::ResourceLimit {
            resource: ProductionSemanticKirResourceV1::Operations,
            actual: usize::MAX,
            limit: max_operations,
        },
    )?;
    let block =
        block_operations
            .checked_add(count)
            .ok_or(ProductionSemanticKirErrorV1::ResourceLimit {
                resource: ProductionSemanticKirResourceV1::Operations,
                actual: usize::MAX,
                limit: MAX_BLOCK_OPERATIONS_V1,
            })?;
    enforce_limit(
        ProductionSemanticKirResourceV1::Operations,
        total,
        max_operations,
    )?;
    enforce_limit(
        ProductionSemanticKirResourceV1::Operations,
        block,
        MAX_BLOCK_OPERATIONS_V1,
    )?;
    Ok(length)
}

impl SemanticFunctionLoweringV1<'_> {
    fn retained_array_slot_v1(
        &self,
        local: SemanticLocalIdV1,
    ) -> Option<&SemanticRetainedLocalSlotV1> {
        self.retained_local_slots
            .get(&local.index())
            .filter(|slot| slot.array.is_some())
    }

    fn require_retained_array_initialized_v1(
        &self,
        block: SemanticBlockIdV1,
        statement: Option<u32>,
        local: SemanticLocalIdV1,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        if !self.retained_local_initialized.contains(&local.index()) {
            return Err(unsupported(
                self.semantic_function.index(),
                Some(block.index()),
                statement,
                "retained array read requires whole-array initialization",
            ));
        }
        Ok(())
    }

    fn require_retained_array_expansion_v1(
        &self,
        length: u64,
        operations: &[Operation],
    ) -> Result<usize, ProductionSemanticKirErrorV1> {
        retained_array_expansion_v1(
            length,
            self.emitted_operations,
            operations.len(),
            self.max_operations,
        )
    }

    fn emit_retained_array_pointer_v1(
        &mut self,
        slot: &SemanticRetainedLocalSlotV1,
        offset: ValueId,
        operations: &mut Vec<Operation>,
    ) -> Result<ValueId, ProductionSemanticKirErrorV1> {
        self.emit_id(
            operations,
            Type::pointer(
                slot.kernel_type.clone(),
                AddressSpace::Private,
                AccessMode::ReadWrite,
            ),
            OperationKind::GetElementPointer {
                base: slot.pointer,
                offset,
            },
        )
    }

    fn retained_array_element_pointer_v1(
        &mut self,
        block: SemanticBlockIdV1,
        statement: Option<u32>,
        place: &SemanticPlaceV1,
        operations: &mut Vec<Operation>,
    ) -> Result<(ValueId, SemanticRetainedLocalSlotV1), ProductionSemanticKirErrorV1> {
        let slot = self
            .retained_array_slot_v1(place.local())
            .cloned()
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        let array = slot
            .array
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        let [projection] = place.projections() else {
            return Err(unsupported(
                self.semantic_function.index(),
                Some(block.index()),
                statement,
                "retained array place requires one element-index projection",
            ));
        };
        if place.ty() != array.element || projection.result_type() != array.element {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
        let mut private_original_index = None;
        let offset = match projection.kind() {
            SemanticProjectionKindV1::ConstantIndex {
                offset,
                minimum_length,
                from_end,
            } => {
                if self.private_arrays.frame.is_some() {
                    self.private_arrays.work.charge_private_array_work(1)?;
                    private_original_index = Some(PrivateArrayIndexV1::ConstantIndex {
                        offset,
                        min_length: minimum_length,
                        from_end,
                    });
                }
                let index = if from_end {
                    array.length.checked_sub(offset)
                } else {
                    Some(offset)
                };
                let index = index
                    .filter(|index| *index < array.length && minimum_length <= array.length)
                    .ok_or_else(|| {
                        unsupported(
                            self.semantic_function.index(),
                            Some(block.index()),
                            statement,
                            "retained array constant index is out of range",
                        )
                    })?;
                self.emit_index_constant(operations, index)?
            }
            SemanticProjectionKindV1::Index(local) => {
                let index_type = self
                    .function
                    .locals()
                    .get(local.index() as usize)
                    .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?
                    .ty();
                let index_place = SemanticPlaceV1::new(local, Vec::new(), index_type)
                    .map_err(|_| ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
                let binding = self.resolve_place(block, statement, &index_place, operations)?;
                let (mut index, ty) = binding.value().map_err(|detail| {
                    unsupported(
                        self.semantic_function.index(),
                        Some(block.index()),
                        statement,
                        detail,
                    )
                })?;
                let scalar = ty
                    .as_scalar()
                    .filter(|scalar| {
                        matches!(
                            scalar,
                            ScalarType::U8
                                | ScalarType::U16
                                | ScalarType::U32
                                | ScalarType::U64
                                | ScalarType::Index
                        )
                    })
                    .ok_or_else(|| {
                        unsupported(
                            self.semantic_function.index(),
                            Some(block.index()),
                            statement,
                            "retained array index is not a modeled unsigned integer",
                        )
                    })?;
                if self.private_arrays.frame.is_some() {
                    let direct_definition = self
                        .private_arrays
                        .direct_definition(index, scalar)?
                        .map(|row| row.location);
                    self.private_arrays.work.charge_private_array_work(1)?;
                    private_original_index = Some(PrivateArrayIndexV1::Local {
                        local: local.index(),
                        semantic_type: index_type,
                        original: index,
                        physical_type: scalar,
                        direct_definition,
                    });
                }
                if let Some(constant) = self.emitted_unsigned_constants.get(&index).copied() {
                    if constant >= array.length {
                        return Err(unsupported(
                            self.semantic_function.index(),
                            Some(block.index()),
                            statement,
                            "retained array constant index is out of range",
                        ));
                    }
                    self.emit_index_constant(operations, constant)?
                } else {
                    // This transports the source index exactly. Existing source Assert
                    // control flow and downstream memory checks retain the bounds duty.
                    let path = plan_integer_cast_v1(scalar, ScalarType::Index)
                        .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
                    for (kind, target) in path.into_iter().flatten() {
                        let to = Type::Scalar(target);
                        index = self.emit_id(
                            operations,
                            to.clone(),
                            OperationKind::Cast {
                                kind,
                                value: index,
                                to,
                            },
                        )?;
                    }
                    index
                }
            }
            _ => {
                return Err(unsupported(
                    self.semantic_function.index(),
                    Some(block.index()),
                    statement,
                    "retained array place requires one element-index projection",
                ));
            }
        };
        let gep_operation = operations.len();
        let pointer = self.emit_retained_array_pointer_v1(&slot, offset, operations)?;
        if let Some(original_index) = private_original_index {
            let offset_location = self
                .private_arrays
                .direct_definition(offset, ScalarType::Index)?
                .map(|row| row.location);
            self.private_arrays.work.charge_private_array_work(1)?;
            if self.private_arrays.pending.is_some() {
                return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
            }
            self.private_arrays.pending = Some(PrivateArrayPendingAddressV1 {
                original_index,
                offset_location,
                gep_location: self.private_arrays.location(gep_operation)?,
                offset,
                gep: pointer,
            });
        }
        Ok((pointer, slot))
    }

    fn load_retained_array_place_v1(
        &mut self,
        block: SemanticBlockIdV1,
        statement: Option<u32>,
        place: &SemanticPlaceV1,
        volatility: SemanticVolatilityV1,
        operations: &mut Vec<Operation>,
    ) -> Result<SemanticValueBindingV1, ProductionSemanticKirErrorV1> {
        self.require_retained_array_initialized_v1(block, statement, place.local())?;
        if !place.projections().is_empty() {
            let (pointer, slot) =
                self.retained_array_element_pointer_v1(block, statement, place, operations)?;
            let mut access = MemoryAccess::new(AddressSpace::Private, slot.alignment);
            access.volatile = volatility == SemanticVolatilityV1::Volatile;
            self.private_arrays
                .prepare_effect(self.emitted_operations)?;
            let operation = operations.len();
            let value = self.emit(
                operations,
                slot.kernel_type,
                OperationKind::Load { pointer, access },
            )?;
            self.private_arrays.commit_effect(
                self.correspondence_owner,
                self.semantic_function,
                place,
                PrivateArrayAccessV1::Read,
                operation,
                self.emitted_operations,
            )?;
            return Ok(value);
        }
        if volatility == SemanticVolatilityV1::Volatile {
            return Err(unsupported(
                self.semantic_function.index(),
                Some(block.index()),
                statement,
                "retained whole-array volatile access requires an aggregate access contract",
            ));
        }
        let slot = self
            .retained_array_slot_v1(place.local())
            .cloned()
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        if place.ty() != slot.semantic_type {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
        let array = slot
            .array
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        let length = self.require_retained_array_expansion_v1(array.length, operations)?;
        let mut fields = Vec::new();
        fields.try_reserve_exact(length).map_err(|_| {
            ProductionSemanticKirErrorV1::AllocationFailure {
                resource: ProductionSemanticKirResourceV1::Operations,
            }
        })?;
        for index in 0..array.length {
            let offset = self.emit_index_constant(operations, index)?;
            let pointer = self.emit_retained_array_pointer_v1(&slot, offset, operations)?;
            let mut access = MemoryAccess::new(AddressSpace::Private, slot.alignment);
            access.volatile = volatility == SemanticVolatilityV1::Volatile;
            fields.push(self.emit(
                operations,
                slot.kernel_type.clone(),
                OperationKind::Load { pointer, access },
            )?);
        }
        Ok(SemanticValueBindingV1::Aggregate(fields))
    }

    fn store_retained_array_place_v1(
        &mut self,
        block: SemanticBlockIdV1,
        statement: Option<u32>,
        place: &SemanticPlaceV1,
        value: &SemanticValueBindingV1,
        volatility: SemanticVolatilityV1,
        operations: &mut Vec<Operation>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        if !place.projections().is_empty() {
            let (value, ty) = value.value().map_err(|detail| {
                unsupported(
                    self.semantic_function.index(),
                    Some(block.index()),
                    statement,
                    detail,
                )
            })?;
            let (pointer, slot) =
                self.retained_array_element_pointer_v1(block, statement, place, operations)?;
            if ty != slot.kernel_type {
                return Err(unsupported(
                    self.semantic_function.index(),
                    Some(block.index()),
                    statement,
                    "retained array element value differs from its exact storage type",
                ));
            }
            let mut access = MemoryAccess::new(AddressSpace::Private, slot.alignment);
            access.volatile = volatility == SemanticVolatilityV1::Volatile;
            self.private_arrays
                .prepare_effect(self.emitted_operations)?;
            let operation = operations.len();
            self.push_memory_store_v1(operations, pointer, value, access, None)?;
            self.private_arrays.commit_effect(
                self.correspondence_owner,
                self.semantic_function,
                place,
                PrivateArrayAccessV1::Write,
                operation,
                self.emitted_operations,
            )?;
            // An element write cannot establish initialization of the entire slot.
            return Ok(());
        }
        if volatility == SemanticVolatilityV1::Volatile {
            return Err(unsupported(
                self.semantic_function.index(),
                Some(block.index()),
                statement,
                "retained whole-array volatile access requires an aggregate access contract",
            ));
        }
        let slot = self
            .retained_array_slot_v1(place.local())
            .cloned()
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        let array = slot
            .array
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        let length = self.require_retained_array_expansion_v1(array.length, operations)?;
        let SemanticValueBindingV1::Aggregate(fields) = value else {
            return Err(unsupported(
                self.semantic_function.index(),
                Some(block.index()),
                statement,
                "retained array assignment requires an exact aggregate value",
            ));
        };
        if place.ty() != slot.semantic_type
            || fields.len() != length
            || fields
                .iter()
                .any(|field| field.value().map_or(true, |(_, ty)| ty != slot.kernel_type))
        {
            return Err(unsupported(
                self.semantic_function.index(),
                Some(block.index()),
                statement,
                "retained array assignment differs from its exact element layout",
            ));
        }
        for (index, field) in fields.iter().enumerate() {
            let (value, _) = field.value().map_err(|detail| {
                unsupported(
                    self.semantic_function.index(),
                    Some(block.index()),
                    statement,
                    detail,
                )
            })?;
            let offset = self.emit_index_constant(operations, index as u64)?;
            let gep_operation = operations.len();
            let pointer = self.emit_retained_array_pointer_v1(&slot, offset, operations)?;
            let initializer = self.private_arrays.prepare_initializer_address(
                place,
                index,
                value,
                offset,
                pointer,
                gep_operation,
            )?;
            let mut access = MemoryAccess::new(AddressSpace::Private, slot.alignment);
            access.volatile = volatility == SemanticVolatilityV1::Volatile;
            if initializer {
                self.private_arrays
                    .prepare_effect(self.emitted_operations)?;
            }
            let operation = operations.len();
            self.push_operation(operations, || {
                Operation::new(
                    Vec::new(),
                    OperationKind::Store {
                        pointer,
                        value,
                        access,
                    },
                )
            })?;
            if initializer {
                self.private_arrays.commit_effect(
                    self.correspondence_owner,
                    self.semantic_function,
                    place,
                    PrivateArrayAccessV1::Write,
                    operation,
                    self.emitted_operations,
                )?;
            }
        }
        self.retained_local_initialized
            .insert(place.local().index());
        Ok(())
    }
}

#[cfg(test)]
#[path = "production_retained_arrays_v1_tests.rs"]
mod retained_array_layout_tests_v1;
