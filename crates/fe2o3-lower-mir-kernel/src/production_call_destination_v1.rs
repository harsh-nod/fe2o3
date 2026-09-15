enum PreparedSemanticCallDestinationV1 {
    Unprojected,
    Memory {
        pointer: ValueId,
        value_type: Type,
        access: MemoryAccess,
    },
}

impl SemanticFunctionLoweringV1<'_> {
    fn prepare_call_destination_v1(
        &mut self,
        block: SemanticBlockIdV1,
        destination: &SemanticPlaceV1,
        operations: &mut Vec<Operation>,
    ) -> Result<PreparedSemanticCallDestinationV1, ProductionSemanticKirErrorV1> {
        if destination.projections().is_empty() {
            return Ok(PreparedSemanticCallDestinationV1::Unprojected);
        }
        if !destination
            .projections()
            .iter()
            .any(|projection| projection.kind() == SemanticProjectionKindV1::Dereference)
        {
            return Err(unsupported(
                0,
                Some(block.index()),
                None,
                "projected local assignment is not a dereferenced store",
            ));
        }
        let (pointer, pointer_type) = self
            .resolve_place(block, None, destination, operations)?
            .value()
            .map_err(|detail| unsupported(0, Some(block.index()), None, detail))?;
        let Type::Pointer(pointer_type) = pointer_type else {
            return Err(unsupported(
                0,
                Some(block.index()),
                None,
                "dereferenced store destination is not a lowered pointer",
            ));
        };
        let access =
            memory_access_for_type(self.types, destination.ty(), pointer_type.address_space)?;
        Ok(PreparedSemanticCallDestinationV1::Memory {
            pointer,
            value_type: *pointer_type.pointee,
            access,
        })
    }

    fn finish_call_destination_v1(
        &mut self,
        block: SemanticBlockIdV1,
        destination: &SemanticPlaceV1,
        prepared: PreparedSemanticCallDestinationV1,
        binding: SemanticValueBindingV1,
        predicate: Option<ValueId>,
        operations: &mut Vec<Operation>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        match prepared {
            PreparedSemanticCallDestinationV1::Unprojected => {
                if predicate.is_some()
                    && self
                        .retained_local_slots
                        .contains_key(&destination.local().index())
                {
                    self.store_retained_local_with_predicate_v1(
                        block,
                        None,
                        destination.local(),
                        &binding,
                        (SemanticVolatilityV1::NonVolatile, predicate),
                        operations,
                    )?;
                    // The only false-guard continuation is the existing trap block.
                    return self.bind_destination(block, None, destination, binding);
                }
                self.assign_place(
                    block,
                    None,
                    destination,
                    binding,
                    SemanticVolatilityV1::NonVolatile,
                    operations,
                )
            }
            PreparedSemanticCallDestinationV1::Memory {
                pointer,
                value_type,
                access,
            } => {
                let (value, actual_type) = binding
                    .value()
                    .map_err(|detail| unsupported(0, Some(block.index()), None, detail))?;
                if actual_type != value_type {
                    return Err(unsupported(
                        self.semantic_function.index(),
                        Some(block.index()),
                        None,
                        "call result differs from its prepared destination type",
                    ));
                }
                self.push_memory_store_v1(operations, pointer, value, access, predicate)
            }
        }
    }

    fn push_memory_store_v1(
        &mut self,
        operations: &mut Vec<Operation>,
        pointer: ValueId,
        value: ValueId,
        access: MemoryAccess,
        predicate: Option<ValueId>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        self.push_operation(operations, || {
            let kind = match predicate {
                Some(predicate) => OperationKind::GuardedStore {
                    pointer,
                    predicate,
                    value,
                    access,
                },
                None => OperationKind::Store {
                    pointer,
                    value,
                    access,
                },
            };
            Operation::new(Vec::new(), kind)
        })
    }
}
