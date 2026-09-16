impl SemanticFunctionLoweringV1<'_> {
    fn lower_return(
        &mut self,
        block: SemanticBlockIdV1,
        operations: &mut Vec<Operation>,
    ) -> Result<Terminator, ProductionSemanticKirErrorV1> {
        let return_local = self
            .function
            .locals()
            .iter()
            .position(|local| local.role() == SemanticLocalRoleV1::Return)
            .ok_or_else(|| {
                unsupported(
                    self.semantic_function.index(),
                    Some(block.index()),
                    None,
                    "function return local is missing",
                )
            })?;
        if self
            .retained_local_slots
            .contains_key(&(return_local as u32))
            && !matches!(
                self.types[self.function.locals()[return_local].ty().index() as usize].shape(),
                SemanticTypeShapeV1::Scalar(_) | SemanticTypeShapeV1::ValidityScalar(_)
            )
        {
            return Err(unsupported(
                self.semantic_function.index(),
                Some(block.index()),
                None,
                "aggregate helper return requires a whole SSA local",
            ));
        }
        let inputs = if self.result_types.is_empty() {
            Vec::new()
        } else {
            self.locals
                .get(return_local)
                .and_then(Option::as_ref)
                .ok_or(ProductionSemanticKirErrorV1::MissingLocalDefinition {
                    function: self.semantic_function.index(),
                    block: block.index(),
                    statement: None,
                    local: return_local as u32,
                })?
                .values()
                .map_err(|detail| {
                    unsupported(
                        self.semantic_function.index(),
                        Some(block.index()),
                        None,
                        detail,
                    )
                })?
        };
        if inputs.len() != self.result_types.len() {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
        let first = self.call_returns.components.rows.len();
        let mut returned = argument_vec_v1(inputs.len())?;
        for (slot, (input, actual)) in inputs.into_iter().enumerate() {
            let expected = self.result_types[slot].clone();
            let (value, conversion) =
                if actual == Type::INDEX && expected == Type::Scalar(ScalarType::U64) {
                    let ordinal = call_operation_ordinal_v1(operations, block)?;
                    let value = self
                        .emit(
                            operations,
                            expected.clone(),
                            OperationKind::Cast {
                                kind: CastKind::Bitcast,
                                value: input,
                                to: expected,
                            },
                        )?
                        .value()
                        .map_err(|detail| {
                            unsupported(
                                self.semantic_function.index(),
                                Some(block.index()),
                                None,
                                detail,
                            )
                        })?
                        .0;
                    (value, Some(ordinal))
                } else if actual == expected {
                    (input, None)
                } else {
                    return Err(unsupported(
                        self.semantic_function.index(),
                        Some(block.index()),
                        None,
                        "helper return local component type changed",
                    ));
                };
            self.call_returns
                .components
                .push(CallResultComponentV1::Return { input, conversion })?;
            returned.push(value);
        }
        let components = self.call_returns.component_span(first)?;
        self.record_call_return_v1(block, SemanticKirCallReturnKindV1::Return { components })?;
        Ok(Terminator::Return { values: returned })
    }

    fn lower_defined_call(
        &mut self,
        block: SemanticBlockIdV1,
        call: &SemanticDirectCallV1,
        callee: SemanticFunctionIdV1,
        operations: &mut Vec<Operation>,
    ) -> Result<Terminator, ProductionSemanticKirErrorV1> {
        if !matches!(call.unwind(), SemanticUnwindActionV1::Unreachable) {
            return Err(unsupported(
                self.semantic_function.index(),
                Some(block.index()),
                None,
                "defined scalar call does not have an unreachable unwind edge",
            ));
        }
        let callee_id = self
            .defined_function_ids
            .get(&callee)
            .cloned()
            .ok_or_else(|| {
                unsupported(
                    self.semantic_function.index(),
                    Some(block.index()),
                    None,
                    "defined call target is outside the lowered helper closure",
                )
            })?;
        let signature = self
            .defined_function_signatures
            .get(&callee)
            .cloned()
            .ok_or_else(|| {
                unsupported(
                    self.semantic_function.index(),
                    Some(block.index()),
                    None,
                    "defined call target has no exact KIR signature",
                )
            })?;
        if call.arguments().len() != signature.parameter_semantic_types.len()
            || signature.call_arguments.len() != signature.parameter_types.len()
        {
            return Err(unsupported(
                self.semantic_function.index(),
                Some(block.index()),
                None,
                "defined call argument or result arity changed",
            ));
        }
        let destination = call.destination().ok_or_else(|| {
            unsupported(
                self.semantic_function.index(),
                Some(block.index()),
                None,
                "returning defined call has no continuation destination",
            )
        })?;
        if destination.place().ty() != signature.result_semantic_type {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
        let aggregate = !matches!(
            self.types[signature.result_semantic_type.index() as usize].shape(),
            SemanticTypeShapeV1::Scalar(_) | SemanticTypeShapeV1::ValidityScalar(_)
        );
        if aggregate
            && (!destination.place().projections().is_empty()
                || self
                    .retained_local_slots
                    .contains_key(&destination.place().local().index()))
        {
            return Err(unsupported(
                self.semantic_function.index(),
                Some(block.index()),
                None,
                "aggregate helper result requires a whole SSA destination",
            ));
        }
        let prepared_destination = match call.destination() {
            Some(destination) if !destination.place().projections().is_empty() => {
                self.prepare_call_destination_v1(block, destination.place(), operations)?
            }
            _ => PreparedSemanticCallDestinationV1::Unprojected,
        };
        let destination_witness = match &prepared_destination {
            PreparedSemanticCallDestinationV1::Memory {
                pointer, access, ..
            } => SemanticKirCallDestinationV1::Projected {
                pointer: *pointer,
                access: *access,
            },
            PreparedSemanticCallDestinationV1::Unprojected => {
                match call.destination().and_then(|destination| {
                    self.retained_local_slots
                        .get(&destination.place().local().index())
                }) {
                    Some(slot) => SemanticKirCallDestinationV1::Retained {
                        pointer: slot.pointer,
                        access: MemoryAccess::new(AddressSpace::Private, slot.alignment),
                    },
                    None => SemanticKirCallDestinationV1::Local,
                }
            }
        };
        let arguments_first = call_operation_ordinal_v1(operations, block)?;
        // Moving each source operand once precedes outer-tuple expansion.
        let mut source_bindings = Vec::new();
        source_bindings
            .try_reserve_exact(call.arguments().len())
            .map_err(|_| ProductionSemanticKirErrorV1::AllocationFailure {
                resource: ProductionSemanticKirResourceV1::AnalysisStorage,
            })?;
        for (argument, expected) in call
            .arguments()
            .iter()
            .zip(&signature.parameter_semantic_types)
        {
            if semantic_operand_type(argument) != *expected {
                return Err(unsupported(
                    self.semantic_function.index(),
                    Some(block.index()),
                    None,
                    "defined call source argument type changed",
                ));
            }
            source_bindings.push(self.lower_operand(block, None, argument, operations)?);
        }
        let mut arguments = Vec::new();
        arguments
            .try_reserve_exact(signature.parameter_types.len())
            .map_err(|_| ProductionSemanticKirErrorV1::AllocationFailure {
                resource: ProductionSemanticKirResourceV1::AnalysisStorage,
            })?;
        let mut flattened = None;
        for (parameter, (projection, expected)) in signature
            .call_arguments
            .iter()
            .zip(signature.parameter_types)
            .enumerate()
        {
            let source = source_bindings
                .get(projection.source_argument as usize)
                .ok_or_else(|| {
                    unsupported(
                        self.semantic_function.index(),
                        Some(block.index()),
                        None,
                        "defined call projection has no source argument",
                    )
                })?;
            let binding = match (source, projection.tuple_field) {
                (SemanticValueBindingV1::Aggregate(fields), Some(field)) => {
                    fields.get(field as usize).ok_or_else(|| {
                        unsupported(
                            self.semantic_function.index(),
                            Some(block.index()),
                            None,
                            "defined call tuple field is missing",
                        )
                    })?
                }
                (_, None) => source,
                (_, Some(_)) => {
                    return Err(unsupported(
                        self.semantic_function.index(),
                        Some(block.index()),
                        None,
                        "defined call RustCall argument is not a tuple binding",
                    ));
                }
            };
            let failure = |detail| {
                unsupported(
                    self.semantic_function.index(),
                    Some(block.index()),
                    None,
                    detail,
                )
            };
            let (value, actual) = match projection.component {
                None => binding.value().map_err(failure)?,
                Some(component) => {
                    let key = (projection.source_argument, projection.tuple_field);
                    if flattened
                        .as_ref()
                        .is_none_or(|(previous, _)| *previous != key)
                    {
                        flattened = Some((key, binding.values().map_err(failure)?));
                    }
                    flattened
                        .as_ref()
                        .unwrap()
                        .1
                        .get(component)
                        .cloned()
                        .ok_or_else(|| failure("defined call aggregate component is missing"))?
                }
            };
            if actual != expected {
                return Err(ProductionSemanticKirErrorV1::DefinedCallArgumentTypeMismatch {
                    function: self.semantic_function.index(),
                    callee: callee.index(),
                    block: block.index(),
                    parameter,
                    source_argument: projection.source_argument,
                    tuple_field: projection.tuple_field,
                    component: projection.component,
                    expected,
                    actual,
                });
            }
            arguments.push(value);
        }
        let call_operation = call_operation_ordinal_v1(operations, block)?;
        let results = self.emit_results(
            operations,
            signature.result_types,
            OperationKind::Call {
                callee: callee_id,
                arguments,
            },
        )?;
        let binding =
            binding_from_value_defs(self.types, signature.result_semantic_type, &results)?;
        let watch = matches!(destination_witness, SemanticKirCallDestinationV1::Local)
            .then_some((destination.place().local(), results.as_slice()));
        self.finish_call_destination_v1(
            block,
            destination.place(),
            prepared_destination,
            binding,
            None,
            operations,
        )?;
        let destination_end = call_operation_ordinal_v1(operations, block)?;
        let (arguments, transport) = self.edge_arguments_with_result_v1(
            block,
            0,
            destination.edge().target(),
            operations,
            watch,
        )?;
        let terminator = Terminator::Branch {
            target: BlockId(destination.edge().target().index()),
            arguments,
        };
        self.record_call_return_v1(
            block,
            SemanticKirCallReturnKindV1::Call {
                arguments_first,
                call_operation,
                destination_end,
                destination: destination_witness,
                transport,
            },
        )?;
        Ok(terminator)
    }
}
