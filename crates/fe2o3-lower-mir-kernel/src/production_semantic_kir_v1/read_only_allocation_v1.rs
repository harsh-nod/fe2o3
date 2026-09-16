fn read_only_allocation_bound_slice_v1(
    base: ValueId,
    definitions: &BTreeMap<ValueId, GuardedAddressDefinitionV1<'_>>,
    budget: &mut GuardedAddressProofBudgetV1,
) -> Option<ValueId> {
    budget.charge().ok()?;
    let operation = operation_definition(definitions, base)?;
    if let OperationKind::SliceData { slice } = operation.kind {
        return Some(slice);
    }
    let OperationKind::Cast {
        kind: CastKind::RestrictPointerAccess,
        value,
        to,
    } = &operation.kind
    else {
        return None;
    };
    let [restricted] = operation.results.as_slice() else {
        return None;
    };
    let Type::Pointer(read_only) = to else {
        return None;
    };
    if restricted.id != base
        || restricted.ty != *to
        || read_only.address_space != AddressSpace::Global
        || read_only.access != AccessMode::ReadOnly
    {
        return None;
    }
    budget.charge().ok()?;
    let source = operation_definition(definitions, *value)?;
    let OperationKind::SliceData { slice } = source.kind else {
        return None;
    };
    let [original] = source.results.as_slice() else {
        return None;
    };
    let Type::Pointer(read_write) = &original.ty else {
        return None;
    };
    if original.id != *value
        || read_write.address_space != AddressSpace::Global
        || read_write.access != AccessMode::ReadWrite
        || read_write.pointee != read_only.pointee
    {
        return None;
    }
    Some(slice)
}
fn read_only_allocation_fields_v1(
    types: &[SemanticTypeDeclV1],
    view: SemanticTypeIdV1,
) -> Option<(SemanticTypeIdV1, SemanticTypeIdV1)> {
    let declaration = types.get(view.index() as usize)?;
    let SemanticTypeShapeV1::Aggregate(fields) = declaration.shape() else {
        return None;
    };
    let [pointer_type, length] = fields.fields() else {
        return None;
    };
    let pointer_declaration = types.get(pointer_type.index() as usize)?;
    let SemanticTypeShapeV1::Pointer(pointer) = pointer_declaration.shape() else {
        return None;
    };
    let length_declaration = types.get(length.index() as usize)?;
    if pointer.kind() != SemanticPointerKindV1::Raw
        || pointer.mutability() != SemanticMutabilityV1::Immutable
        || pointer.address_space() != 0
        || pointer.pointer_width_bits() != 64
        || pointer.metadata() != SemanticPointerMetadataV1::None
        || !matches!(
            length_declaration.shape(),
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                signed: false,
                bits: 64
            })
        )
        || !matches!(
            lower_scalar_type(types, pointer.pointee()).ok(),
            Some(Type::Scalar(ScalarType::U16 | ScalarType::F32))
        )
    {
        return None;
    }
    let SemanticTypeLayoutDetailsV1::Aggregate(layout) = declaration.layout().details() else {
        return None;
    };
    let SemanticBackendReprV1::Scalar(first) = pointer_declaration.layout().backend_repr() else {
        return None;
    };
    let SemanticBackendReprV1::Scalar(second) = length_declaration.layout().backend_repr() else {
        return None;
    };
    (declaration.layout().size_bytes() == Some(16)
        && declaration.layout().alignment_bytes() == 8
        && layout.field_offsets() == [0, 8]
        && *declaration.layout().backend_repr()
            == SemanticBackendReprV1::ScalarPair {
                first: *first,
                second: *second,
            })
    .then_some((pointer.pointee(), *length))
}

struct ReadOnlyAllocationBindingV1 {
    pointer: (ValueId, Type),
    length: (ValueId, Type),
}

impl<'a> SemanticFunctionLoweringV1<'a> {
    fn lower_disjoint_slice_into_read_only_v1(
        &mut self,
        block: SemanticBlockIdV1,
        call: &SemanticDirectCallV1,
        operations: &mut Vec<Operation>,
        slice_type: SemanticTypeIdV1,
        view_type: SemanticTypeIdV1,
        element: SemanticTypeIdV1,
    ) -> Result<SemanticValueBindingV1, ProductionSemanticKirErrorV1> {
        self.require_call_argument_count(block, call, 1)?;
        let invalid = |reason| unsupported(0, Some(block.index()), None, reason);
        let Some((actual_element, _)) = read_only_allocation_fields_v1(self.types, view_type)
        else {
            return Err(invalid(
                "read-only allocation constructor has an invalid exact view layout",
            ));
        };
        let source = match &call.arguments()[0] {
            SemanticOperandV1::Move(source) => source,
            SemanticOperandV1::Copy(source) if source.projections().is_empty() => {
                let local = self.function.locals().get(source.local().index() as usize);
                let Some(local) = local.filter(|local| local.ty() == slice_type) else {
                    return Err(invalid(
                        "read-only allocation Copy source is not its original argument",
                    ));
                };
                let SemanticLocalRoleV1::Argument(argument) = local.role() else {
                    return Err(invalid(
                        "read-only allocation Copy source is not its original argument",
                    ));
                };
                if authenticated_disjoint_slice_parameter(
                    self.types, self.callables, self.function, argument, slice_type,
                ).is_none_or(|ty| !matches!(ty, Type::Slice(slice) if slice.address_space == AddressSpace::Global && slice.access == AccessMode::ReadWrite)) {
                    return Err(invalid("read-only allocation Copy source lacks exact exclusive argument custody"));
                }
                source
            }
            _ => {
                return Err(invalid(
                    "read-only allocation constructor requires Move or an exact original-argument Copy",
                ));
            }
        };
        if actual_element != element || source.ty() != slice_type {
            return Err(invalid(
                "read-only allocation constructor has mismatched slice or element identity",
            ));
        }
        // Optimized MIR can spell this genuine by-value argument as Copy.
        // Consume it only here; the source request and owner replay retain the
        // original spelling, and ranked validation still audits the whole root.
        let consumed = SemanticOperandV1::Move(source.clone());
        let (slice, ty) = self
            .lower_operand(block, None, &consumed, operations)?
            .value()
            .map_err(|_| {
                invalid("read-only allocation constructor requires a single lowered slice value")
            })?;
        let element_type = lower_scalar_type(self.types, element)?;
        if ty
            != Type::slice(
                element_type.clone(),
                AddressSpace::Global,
                AccessMode::ReadWrite,
            )
        {
            return Err(invalid(
                "read-only allocation constructor requires an exact Global RW slice type",
            ));
        }
        let base = self.emit_id(
            operations,
            Type::pointer(
                element_type.clone(),
                AddressSpace::Global,
                AccessMode::ReadWrite,
            ),
            OperationKind::SliceData { slice },
        )?;
        let pointer_type = Type::pointer(element_type, AddressSpace::Global, AccessMode::ReadOnly);
        let pointer = self.emit_id(
            operations,
            pointer_type.clone(),
            OperationKind::Cast {
                kind: CastKind::RestrictPointerAccess,
                value: base,
                to: pointer_type.clone(),
            },
        )?;
        let length = self.emit_id(
            operations,
            Type::INDEX,
            OperationKind::SliceLength { slice },
        )?;
        // This restricts one local pointer only. Whole-kernel no-write custody
        // belongs to ranked source validation; the root ABI remains RW/exclusive.
        Ok(SemanticValueBindingV1::Aggregate(vec![
            SemanticValueBindingV1::Value {
                id: pointer,
                ty: pointer_type,
            },
            SemanticValueBindingV1::Value {
                id: length,
                ty: Type::INDEX,
            },
        ]))
    }

    fn lower_read_only_allocation_receiver_v1(
        &mut self,
        block: SemanticBlockIdV1,
        operand: &SemanticOperandV1,
        operations: &mut Vec<Operation>,
        view_type: SemanticTypeIdV1,
    ) -> Result<ReadOnlyAllocationBindingV1, ProductionSemanticKirErrorV1> {
        let invalid = || {
            unsupported(
                0,
                Some(block.index()),
                None,
                "read-only allocation receiver has no exact pointer/extent representation",
            )
        };
        let (element, length_type) =
            read_only_allocation_fields_v1(self.types, view_type).ok_or_else(invalid)?;
        let binding = self.lower_operand(block, None, operand, operations)?;
        let fields = binding.values().map_err(|_| invalid())?;
        let [pointer, length] = fields.as_slice() else {
            return Err(invalid());
        };
        if pointer.1
            != Type::pointer(
                lower_scalar_type(self.types, element)?,
                AddressSpace::Global,
                AccessMode::ReadOnly,
            )
            || (length.1 != lower_scalar_type(self.types, length_type)?
                && !index_and_u64_are_transport_equivalent(
                    &length.1,
                    &lower_scalar_type(self.types, length_type)?,
                ))
        {
            return Err(invalid());
        }
        Ok(ReadOnlyAllocationBindingV1 {
            pointer: pointer.clone(),
            length: length.clone(),
        })
    }

    fn lower_read_only_allocation_len_v1(
        &mut self,
        block: SemanticBlockIdV1,
        call: &SemanticDirectCallV1,
        operations: &mut Vec<Operation>,
        view_type: SemanticTypeIdV1,
    ) -> Result<SemanticValueBindingV1, ProductionSemanticKirErrorV1> {
        self.require_call_argument_count(block, call, 1)?;
        let ReadOnlyAllocationBindingV1 {
            length: (id, ty), ..
        } = self.lower_read_only_allocation_receiver_v1(
            block,
            &call.arguments()[0],
            operations,
            view_type,
        )?;
        Ok(SemanticValueBindingV1::Value { id, ty })
    }

    fn lower_read_only_allocation_load_or_v1(
        &mut self,
        block: SemanticBlockIdV1,
        call: &SemanticDirectCallV1,
        operations: &mut Vec<Operation>,
        view_type: SemanticTypeIdV1,
        element: SemanticTypeIdV1,
    ) -> Result<SemanticValueBindingV1, ProductionSemanticKirErrorV1> {
        self.require_call_argument_count(block, call, 3)?;
        let invalid = || {
            unsupported(
                0,
                Some(block.index()),
                None,
                "read-only allocation load element/index/fallback contract changed",
            )
        };
        if read_only_allocation_fields_v1(self.types, view_type)
            .is_none_or(|(actual, _)| actual != element)
        {
            return Err(invalid());
        }
        let ReadOnlyAllocationBindingV1 {
            pointer: (base, pointer_type),
            length,
        } = self.lower_read_only_allocation_receiver_v1(
            block,
            &call.arguments()[0],
            operations,
            view_type,
        )?;
        let index = self.lower_operand(block, None, &call.arguments()[1], operations)?;
        let (index, index_type) = index.value().map_err(|_| invalid())?;
        if index_type != length.1 && !index_and_u64_are_transport_equivalent(&index_type, &length.1)
        {
            return Err(invalid());
        }
        let index = self.coerce_typed_index_component(block, operations, &(index, index_type))?;
        let length = self.coerce_typed_index_component(block, operations, &length)?;
        let fallback = self.lower_operand(block, None, &call.arguments()[2], operations)?;
        let (fallback, fallback_type) = fallback.value().map_err(|_| invalid())?;
        let element_type = lower_scalar_type(self.types, element)?;
        if fallback_type != element_type {
            return Err(invalid());
        }
        let predicate = self.emit_compare(operations, ComparePredicate::LessThan, index, length)?;
        let zero = self.emit_index_constant(operations, 0)?;
        let safe_index = self.emit_select_index(operations, predicate, index, zero)?;
        let pointer = self.emit_id(
            operations,
            pointer_type,
            OperationKind::GetElementPointer {
                base,
                offset: safe_index,
            },
        )?;
        let alignment = strided_read_scalar_alignment_v1(&element_type).ok_or_else(invalid)?;
        let id = self.emit_id(
            operations,
            element_type.clone(),
            OperationKind::GuardedLoad {
                pointer,
                predicate,
                fallback,
                access: MemoryAccess::new(AddressSpace::Global, alignment),
            },
        )?;
        Ok(SemanticValueBindingV1::Value {
            id,
            ty: element_type,
        })
    }
}
