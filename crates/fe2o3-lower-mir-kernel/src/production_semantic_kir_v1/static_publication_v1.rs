include!("static_publication_roster_v1.rs");

impl ProductionSemanticKirOwnerV1 {
    pub(crate) fn retained_static_publication_inputs_v1(
        &self,
        kernel_id: &str,
    ) -> Option<(
        &ProductionRankedKernelLoweringInputV1,
        &[ProductionRankedAccessSourceV1],
    )> {
        let mut matching = self
            .generic_checks
            .iter()
            .filter(|checks| checks.function_name == kernel_id);
        let checks = matching.next()?;
        if matching.next().is_some() {
            return None;
        }
        Some((&checks.lowering, &checks.access_sources))
    }
}

#[derive(Clone, Copy)]
struct StaticPublicationTypesV1 {
    payload: SemanticTypeIdV1,
    flags: SemanticTypeIdV1,
    result: SemanticTypeIdV1,
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum StaticPublicationRoleV1 {
    Publish,
    TryRead,
}

fn static_publication_payload_fields_v1(
    types: &[SemanticTypeDeclV1],
    payload: SemanticTypeIdV1,
) -> Option<(SemanticTypeIdV1, SemanticTypeIdV1)> {
    let declaration = types.get(payload.index() as usize)?;
    let SemanticTypeShapeV1::Aggregate(fields) = declaration.shape() else {
        return None;
    };
    let [pointer, length, marker] = fields.fields() else {
        return None;
    };
    let pointer_decl = types.get(pointer.index() as usize)?;
    let SemanticTypeShapeV1::Pointer(pointer) = pointer_decl.shape() else {
        return None;
    };
    let length_decl = types.get(length.index() as usize)?;
    let SemanticTypeLayoutDetailsV1::Aggregate(layout) = declaration.layout().details() else {
        return None;
    };
    let SemanticBackendReprV1::Scalar(first) = pointer_decl.layout().backend_repr() else {
        return None;
    };
    let SemanticBackendReprV1::Scalar(second) = length_decl.layout().backend_repr() else {
        return None;
    };
    (pointer.kind() == SemanticPointerKindV1::Raw
        && pointer.mutability() == SemanticMutabilityV1::Mutable
        && pointer.address_space() == 0
        && pointer.pointer_width_bits() == 64
        && pointer.metadata() == SemanticPointerMetadataV1::None
        && lower_scalar_type(types, pointer.pointee()).ok()? == Type::Scalar(ScalarType::F32)
        && lower_scalar_type(types, *length).ok()? == Type::Scalar(ScalarType::U64)
        && types.get(marker.index() as usize)?.layout().size_bytes() == Some(0)
        && declaration.layout().size_bytes() == Some(16)
        && declaration.layout().alignment_bytes() == 8
        && layout.field_offsets() == [0, 8, 16]
        && *declaration.layout().backend_repr()
            == SemanticBackendReprV1::ScalarPair {
                first: *first,
                second: *second,
            })
    .then_some((pointer.pointee(), *length))
}

fn static_publication_result_layout_v1(
    types: &[SemanticTypeDeclV1],
    result: SemanticTypeIdV1,
) -> bool {
    let Some(declaration) = types.get(result.index() as usize) else {
        return false;
    };
    let SemanticTypeShapeV1::Aggregate(fields) = declaration.shape() else {
        return false;
    };
    let [status, value] = fields.fields() else {
        return false;
    };
    let SemanticTypeLayoutDetailsV1::Aggregate(layout) = declaration.layout().details() else {
        return false;
    };
    declaration.layout().size_bytes() == Some(8)
        && declaration.layout().alignment_bytes() == 4
        && layout.field_offsets() == [0, 4]
        && lower_scalar_type(types, *status).ok() == Some(Type::Scalar(ScalarType::U32))
        && lower_scalar_type(types, *value).ok() == Some(Type::Scalar(ScalarType::F32))
}

fn static_publication_atomic_v1(pointer: ValueId, value: Option<ValueId>) -> OperationKind {
    OperationKind::Atomic(Atomic {
        kind: if value.is_some() {
            AtomicKind::Store
        } else {
            AtomicKind::Load
        },
        pointer,
        value,
        compare: None,
        access: MemoryAccess::new(AddressSpace::Global, 4),
        scope: SynchronizationScope::System,
        ordering: if value.is_some() {
            MemoryOrdering::Release
        } else {
            MemoryOrdering::Acquire
        },
        failure_ordering: None,
    })
}

impl<'a> SemanticFunctionLoweringV1<'a> {
    fn lower_static_publication_payload_v1(
        &mut self,
        block: SemanticBlockIdV1,
        operand: &SemanticOperandV1,
        operations: &mut Vec<Operation>,
        payload: SemanticTypeIdV1,
    ) -> Result<ValueId, ProductionSemanticKirErrorV1> {
        let invalid = || {
            unsupported(
                0,
                Some(block.index()),
                None,
                "static publication requires an exact consumed exclusive f32 slice",
            )
        };
        let source = match operand {
            SemanticOperandV1::Move(source) if source.projections().is_empty() => source,
            SemanticOperandV1::Copy(source) if source.projections().is_empty() => {
                let local = self
                    .function
                    .locals()
                    .get(source.local().index() as usize)
                    .filter(|local| local.ty() == payload)
                    .ok_or_else(invalid)?;
                let SemanticLocalRoleV1::Argument(argument) = local.role() else {
                    return Err(invalid());
                };
                if authenticated_disjoint_slice_parameter(
                    self.types,
                    self.callables,
                    self.function,
                    argument,
                    payload,
                ) != Some(Type::slice(
                    Type::Scalar(ScalarType::F32),
                    AddressSpace::Global,
                    AccessMode::ReadWrite,
                )) {
                    return Err(invalid());
                }
                source
            }
            _ => return Err(invalid()),
        };
        if source.ty() != payload {
            return Err(invalid());
        }
        // Consume optimized original-argument Copy only at this trusted terminal.
        // Source replay and the ranked whole-root custody audit remain mandatory.
        let binding = self.lower_operand(
            block,
            None,
            &SemanticOperandV1::Move(source.clone()),
            operations,
        )?;
        let (value, ty) = binding.value().map_err(|_| invalid())?;
        if ty
            != Type::slice(
                Type::Scalar(ScalarType::F32),
                AddressSpace::Global,
                AccessMode::ReadWrite,
            )
        {
            return Err(invalid());
        }
        Ok(value)
    }

    fn lower_static_publication_v1(
        &mut self,
        block: SemanticBlockIdV1,
        call: &SemanticDirectCallV1,
        operations: &mut Vec<Operation>,
        types: StaticPublicationTypesV1,
        role: StaticPublicationRoleV1,
    ) -> Result<SemanticValueBindingV1, ProductionSemanticKirErrorV1> {
        let publish = role == StaticPublicationRoleV1::Publish;
        self.require_call_argument_count(block, call, if publish { 4 } else { 3 })?;
        let invalid = || {
            unsupported(
                0,
                Some(block.index()),
                None,
                "static publication terminal type or argument contract changed",
            )
        };
        let (_, length_type) =
            static_publication_payload_fields_v1(self.types, types.payload).ok_or_else(invalid)?;
        let expected_flags =
            lower_shared_atomic_slice_parameter_v1(self.types, types.flags).ok_or_else(invalid)?;
        if !static_publication_result_layout_v1(self.types, types.result) {
            return Err(invalid());
        }
        let payload = self.lower_static_publication_payload_v1(
            block,
            &call.arguments()[0],
            operations,
            types.payload,
        )?;
        if call.arguments()[1].ty() != types.flags || call.arguments()[2].ty() != length_type {
            return Err(invalid());
        }
        let flags = self.lower_operand(block, None, &call.arguments()[1], operations)?;
        let (flags, flags_type) = flags.value().map_err(|_| invalid())?;
        if flags_type != expected_flags {
            return Err(invalid());
        }
        let cell = self.lower_operand(block, None, &call.arguments()[2], operations)?;
        let cell = cell.value().map_err(|_| invalid())?;
        let cell = self.coerce_typed_index_component(block, operations, &cell)?;
        let payload_pointer_type = Type::pointer(
            Type::Scalar(ScalarType::F32),
            AddressSpace::Global,
            AccessMode::ReadWrite,
        );
        let payload_base = self.emit_id(
            operations,
            payload_pointer_type.clone(),
            OperationKind::SliceData { slice: payload },
        )?;
        let flag_pointer_type = Type::pointer(
            Type::Scalar(ScalarType::U32),
            AddressSpace::Global,
            AccessMode::ReadWrite,
        );
        let flag_base = self.emit_id(
            operations,
            flag_pointer_type.clone(),
            OperationKind::SliceData { slice: flags },
        )?;
        // The facade's real source bounds dominate the unconditional atomics.
        let flag_pointer = self.emit_id(
            operations,
            flag_pointer_type,
            OperationKind::GetElementPointer {
                base: flag_base,
                offset: cell,
            },
        )?;
        let ready = self.emit_id(
            operations,
            Type::Scalar(ScalarType::U32),
            OperationKind::Constant(Constant::U32(2)),
        )?;
        let zero_value = self.emit_id(
            operations,
            Type::Scalar(ScalarType::F32),
            OperationKind::Constant(Constant::F32Bits(0)),
        )?;
        let (status, value) = if publish {
            let value = self.lower_operand(block, None, &call.arguments()[3], operations)?;
            let (value, value_type) = value.value().map_err(|_| invalid())?;
            if value_type != Type::Scalar(ScalarType::F32) {
                return Err(invalid());
            }
            let pointer = self.emit_id(
                operations,
                payload_pointer_type,
                OperationKind::GetElementPointer {
                    base: payload_base,
                    offset: cell,
                },
            )?;
            operations.push(Operation::new(
                vec![],
                OperationKind::Store {
                    pointer,
                    value,
                    access: MemoryAccess::new(AddressSpace::Global, 4),
                },
            ));
            operations.push(Operation::new(
                vec![],
                static_publication_atomic_v1(flag_pointer, Some(ready)),
            ));
            let status = self.emit_id(
                operations,
                Type::Scalar(ScalarType::U32),
                OperationKind::Constant(Constant::U32(1)),
            )?;
            (status, zero_value)
        } else {
            let request = self.emit_id(
                operations,
                Type::Scalar(ScalarType::U32),
                OperationKind::Constant(Constant::U32(1)),
            )?;
            operations.push(Operation::new(
                vec![],
                static_publication_atomic_v1(flag_pointer, Some(request)),
            ));
            let observed = self.emit_id(
                operations,
                Type::Scalar(ScalarType::U32),
                static_publication_atomic_v1(flag_pointer, None),
            )?;
            let matches =
                self.emit_compare(operations, ComparePredicate::Equal, observed, ready)?;
            let length = self.emit_id(
                operations,
                Type::INDEX,
                OperationKind::SliceLength { slice: payload },
            )?;
            let in_bounds =
                self.emit_compare(operations, ComparePredicate::LessThan, cell, length)?;
            let predicate = self.emit_bool_and(operations, matches, in_bounds)?;
            let zero_index = self.emit_index_constant(operations, 0)?;
            let safe_index = self.emit_select_index(operations, predicate, cell, zero_index)?;
            let pointer = self.emit_id(
                operations,
                payload_pointer_type,
                OperationKind::GetElementPointer {
                    base: payload_base,
                    offset: safe_index,
                },
            )?;
            let value = self.emit_id(
                operations,
                Type::Scalar(ScalarType::F32),
                OperationKind::GuardedLoad {
                    pointer,
                    predicate,
                    fallback: zero_value,
                    access: MemoryAccess::new(AddressSpace::Global, 4),
                },
            )?;
            let not_ready = self.emit_id(
                operations,
                Type::Scalar(ScalarType::U32),
                OperationKind::Constant(Constant::U32(0)),
            )?;
            let status = self.emit_id(
                operations,
                Type::Scalar(ScalarType::U32),
                OperationKind::Select {
                    condition: predicate,
                    true_value: ready,
                    false_value: not_ready,
                },
            )?;
            (status, value)
        };
        Ok(SemanticValueBindingV1::Aggregate(vec![
            SemanticValueBindingV1::Value {
                id: status,
                ty: Type::Scalar(ScalarType::U32),
            },
            SemanticValueBindingV1::Value {
                id: value,
                ty: Type::Scalar(ScalarType::F32),
            },
        ]))
    }
}
