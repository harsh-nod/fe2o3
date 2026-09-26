use production_call_instances_v1::ProductionCallControlV1;

// This is a prepared emission position, never a nominal result or an edge proof.
struct ExecutionCallRequestV1 {
    source: ExecutionCallSourceV29,
    occurrence: ProductionCallOccurrenceV1,
    original_call: usize,
    child: ProductionCallInstanceIdV1,
    callee: SemanticFunctionIdV1,
    normal_return: bool,
    target: FunctionId,
    arguments: Vec<ValueId>,
    credit: usize,
}

impl ExecutionCallRequestV1 {
    fn check_position_v1(
        &self,
        lowering: &SemanticFunctionLoweringV1<'_, '_>,
        budget: &mut dyn SemanticEmissionBudgetV1,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        budget.charge_work(8)?;
        let cursor = lowering
            .execution
            .as_ref()
            .ok_or_else(execution_call_error_v29)?;
        cursor.check_ledger(budget)?;
        if cursor.source != self.source
            || cursor.instance != self.occurrence.caller
            || cursor.block != Some(SsaBlockIdV1::new(self.occurrence.block.index()))
            || !std::ptr::eq(cursor.function, lowering.function)
        {
            return Err(execution_call_error_v29());
        }
        let original = cursor
            .function
            .blocks()
            .get(self.occurrence.block.index() as usize)
            .ok_or_else(execution_call_error_v29)?;
        let SemanticTerminatorKindV1::Call(call) = original.terminator().kind() else {
            return Err(execution_call_error_v29());
        };
        if std::ptr::from_ref(call) as usize != self.original_call
            || !matches!(lowering.callables.get(call.callee().index() as usize),
                Some(SemanticCallableDeclV1::Defined { function }) if *function == self.callee)
        {
            return Err(execution_call_error_v29());
        }
        let control = cursor.source_call_control_v29(self.occurrence.block, call, budget)?;
        if control == ProductionCallControlV1::Unreachable
            || (control == ProductionCallControlV1::MayReturn) != self.normal_return
        {
            return Err(execution_call_error_v29());
        }
        Ok(())
    }
}

struct AwaitingDefinedCallV1 {
    prefix: DefinedCallEmissionPrefixV1,
    request: ExecutionCallRequestV1,
    target: BasicBlock,
    terminator_first: usize,
    credit: usize,
}

struct SuspendedCallStartV1 {
    first_value: u32,
    next_block: u32,
    remaining_operations: usize,
    private_payload: PrivateArrayPayloadV1,
}

enum FunctionFrameStepV1 {
    Done,
    Block,
    AwaitChild(AwaitingDefinedCallV1),
}

struct DefinedCallEmissionPrefixV1 {
    header: usize,
    block: SemanticBlockIdV1,
    callee: SemanticFunctionIdV1,
    original_call: usize,
    source: Option<(ExecutionCallSourceV29, ProductionCallInstanceIdV1)>,
    ordinary_target: Option<FunctionId>,
    signature: LoweredFunctionSignatureV1,
    normal_return: bool,
    destination: PreparedSemanticCallDestinationV1,
    destination_witness: SemanticKirCallDestinationV1,
    arguments_first: u32,
}

impl SemanticFunctionLoweringV1<'_, '_> {
    fn request_defined_call_v1(
        &mut self,
        block: SemanticBlockIdV1,
        call: &SemanticDirectCallV1,
        callee: SemanticFunctionIdV1,
        operations: &mut Vec<Operation>,
    ) -> Result<(DefinedCallEmissionPrefixV1, ExecutionCallRequestV1), ProductionSemanticKirErrorV1>
    {
        self.with_scoped_memory_frame_v29(
            ScopedMemoryFrameV29 {
                site: execution_site_v29(block, None),
                role: None,
            },
            |this| {
                let mut prefix =
                    this.prepare_defined_call_prefix_v1(block, call, callee, operations)?;
                let consumer = this
                    .execution_calls
                    .take()
                    .ok_or_else(execution_call_error_v29)?;
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    consumer.request_child_v1(
                        this,
                        block,
                        call,
                        callee,
                        &prefix.signature.parameter_semantic_types,
                        &prefix.signature.call_arguments,
                        std::mem::take(&mut prefix.signature.parameter_types),
                        operations,
                    )
                }));
                this.execution_calls = Some(consumer);
                match result {
                    Ok(request) => Ok((prefix, request?)),
                    Err(payload) => std::panic::resume_unwind(payload),
                }
            },
        )
    }

    fn prepare_defined_call_prefix_v1(
        &mut self,
        block: SemanticBlockIdV1,
        call: &SemanticDirectCallV1,
        callee: SemanticFunctionIdV1,
        operations: &mut Vec<Operation>,
    ) -> Result<DefinedCallEmissionPrefixV1, ProductionSemanticKirErrorV1> {
        let header = if self.execution_calls.is_some() {
            let bytes = argument_sum_v1(&[
                std::mem::size_of::<DefinedCallEmissionPrefixV1>(),
                std::mem::size_of::<
                    Result<DefinedCallEmissionPrefixV1, ProductionSemanticKirErrorV1>,
                >(),
                std::mem::size_of::<(Terminator, usize)>(),
                std::mem::size_of::<
                    Result<(Terminator, usize), ProductionSemanticKirErrorV1>,
                >(),
            ])?;
            let budget = self
                .emission_work
                .as_deref_mut()
                .ok_or(ArgumentResourceV1::Accounting)?;
            budget.charge_work(3)?;
            budget.reserve_storage(bytes)?;
            bytes
        } else {
            0
        };
        if !matches!(call.unwind(), SemanticUnwindActionV1::Unreachable) {
            return Err(unsupported(
                self.semantic_function.index(),
                Some(block.index()),
                None,
                "defined scalar call does not have an unreachable unwind edge",
            ));
        }
        let scoped = self.execution_calls.is_some();
        if scoped && self.execution.is_none() {
            return Err(execution_call_error_v29());
        }
        if !scoped && self.execution.is_some() {
            match self.with_emission_budget_v1(|this, budget| {
                require_execution_free_types_v29(this.types, budget)
            }) {
                Err(ProductionSemanticKirErrorV1::Unsupported { .. }) => {
                    return Err(execution_call_error_v29());
                }
                result => result?,
            }
        }
        let ordinary_target = if scoped {
            None
        } else {
            Some(
                self.defined_function_ids
                    .get(&callee)
                    .cloned()
                    .ok_or_else(|| {
                        unsupported(
                            self.semantic_function.index(),
                            Some(block.index()),
                            None,
                            "defined call target is outside the lowered helper closure",
                        )
                    })?,
            )
        };
        let signature = self.defined_function_signatures.signature_for_call_v29(
            self.execution.as_ref(), self.semantic_function, block, call, callee, scoped,
            match self.emission_work.as_mut() {
                Some(budget) => Some(&mut **budget as &mut (dyn SemanticEmissionBudgetV1 + '_)),
                None => None,
            },
        )?;
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
        let normal_return = if scoped {
            self.with_emission_budget_v1(|this, budget| {
                match this
                    .execution
                    .as_ref()
                    .ok_or_else(execution_call_error_v29)?
                    .source_call_control_v29(block, call, budget)?
                {
                    ProductionCallControlV1::MayReturn => Ok(true),
                    ProductionCallControlV1::NoNormalReturn => Ok(false),
                    ProductionCallControlV1::Unreachable => Err(execution_call_error_v29()),
                }
            })?
        } else {
            true
        };
        let destination = call.destination();
        if normal_return && destination.is_none() {
            return Err(unsupported(
                self.semantic_function.index(),
                Some(block.index()),
                None,
                "returning defined call has no continuation destination",
            ));
        }
        if destination
            .is_some_and(|destination| destination.place().ty() != signature.result_semantic_type)
        {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
        let aggregate = !matches!(
            self.types[signature.result_semantic_type.index() as usize].shape(),
            SemanticTypeShapeV1::Scalar(_) | SemanticTypeShapeV1::ValidityScalar(_)
        );
        if normal_return
            && aggregate
            && let Some(destination) = destination
            && (!destination.place().projections().is_empty()
                || self.legacy_retained_slot_v29(destination.place().local())?.is_some())
        {
            return Err(unsupported(
                self.semantic_function.index(),
                Some(block.index()),
                None,
                "aggregate helper result requires a whole SSA destination",
            ));
        }
        let prepared_destination = match call.destination() {
            Some(destination) => {
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
            PreparedSemanticCallDestinationV1::Object { pointer, access, .. } => {
                if call.destination().is_some_and(|destination| destination.place().projections().is_empty()) {
                    SemanticKirCallDestinationV1::Retained { pointer: *pointer, access: *access }
                } else {
                    SemanticKirCallDestinationV1::Projected { pointer: *pointer, access: *access }
                }
            }
            PreparedSemanticCallDestinationV1::Unprojected => {
                let slot = match call.destination() {
                    Some(destination) => self.legacy_retained_slot_v29(destination.place().local())?,
                    None => None,
                };
                match slot {
                    Some(slot) => SemanticKirCallDestinationV1::Retained {
                        pointer: slot.pointer,
                        access: MemoryAccess::new(AddressSpace::Private, slot.storage.scalar_array()?.1),
                    },
                    None => SemanticKirCallDestinationV1::Local,
                }
            }
        };
        Ok(DefinedCallEmissionPrefixV1 {
            header,
            block,
            callee,
            original_call: std::ptr::from_ref(call) as usize,
            source: self
                .execution
                .as_ref()
                .map(|cursor| (cursor.source, cursor.instance)),
            ordinary_target,
            signature,
            normal_return,
            destination: prepared_destination,
            destination_witness,
            arguments_first: call_operation_ordinal_v1(operations, block)?,
        })
    }

    fn finish_defined_call_prefix_v1(
        &mut self,
        call: &SemanticDirectCallV1,
        prepared: DefinedCallEmissionPrefixV1,
        callee_id: FunctionId,
        arguments: Vec<ValueId>,
        nominal: Vec<Option<ExecutionCfgLeafV29>>,
        operations: &mut Vec<Operation>,
    ) -> Result<(Terminator, usize), ProductionSemanticKirErrorV1> {
        let DefinedCallEmissionPrefixV1 {
            header,
            block,
            callee,
            original_call,
            source,
            ordinary_target: _,
            mut signature,
            normal_return,
            destination: prepared_destination,
            destination_witness,
            arguments_first,
        } = prepared;
        if header != 0 {
            self.emission_work
                .as_deref_mut()
                .ok_or(ArgumentResourceV1::Accounting)?
                .charge_work(2)?;
        }
        if original_call != std::ptr::from_ref(call) as usize
            || source
                != self
                    .execution
                    .as_ref()
                    .map(|cursor| (cursor.source, cursor.instance))
        {
            return Err(execution_call_error_v29());
        }
        if !normal_return {
            if !nominal.is_empty() || header == 0 {
                return Err(execution_call_error_v29());
            }
            let actual = self.with_emission_budget_v1(|this, budget| {
                this.execution
                    .as_ref()
                    .ok_or_else(execution_call_error_v29)?
                    .source_call_control_v29(block, call, budget)
            })?;
            if actual != ProductionCallControlV1::NoNormalReturn {
                return Err(execution_call_error_v29());
            }
            let call_operation = call_operation_ordinal_v1(operations, block)?;
            let unused = self.emit_results(
                operations,
                std::mem::take(&mut signature.result_types),
                OperationKind::Call {
                    callee: callee_id,
                    arguments,
                },
            )?;
            drop(unused);
            self.record_call_return_v1(
                block,
                SemanticKirCallReturnKindV1::NoNormalReturnCall {
                    arguments_first,
                    call_operation,
                    destination: call.destination().map(|_| destination_witness),
                },
            )?;
            let nominal_bytes = argument_sum_v1(&[
                std::mem::size_of::<Vec<Option<ExecutionCfgLeafV29>>>(),
                argument_product_v1(
                    nominal.capacity(),
                    std::mem::size_of::<Option<ExecutionCfgLeafV29>>(),
                )?,
            ])?;
            drop(nominal);
            drop(prepared_destination);
            drop(signature);
            return Ok((
                Terminator::Unreachable,
                argument_sum_v1(&[header, nominal_bytes])?,
            ));
        }
        let destination = call.destination().ok_or_else(execution_call_error_v29)?;
        let scoped = self.execution_calls.is_some();
        let call_operation = call_operation_ordinal_v1(operations, block)?;
        let results = self.emit_results(
            operations,
            std::mem::take(&mut signature.result_types),
            OperationKind::Call {
                callee: callee_id,
                arguments,
            },
        )?;
        let binding = match self
            .source_reference_call_result_v29(block, call, callee, &results, &nominal)?
        {
            Some(binding) => binding,
            None => binding_from_value_defs(self.types, signature.result_semantic_type, &results)?,
        };
        let nominal_bytes = argument_sum_v1(&[
            std::mem::size_of::<Vec<Option<ExecutionCfgLeafV29>>>(),
            argument_product_v1(
                nominal.capacity(),
                std::mem::size_of::<Option<ExecutionCfgLeafV29>>(),
            )?,
        ])?;
        drop(nominal);
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
            target: self.kernel_block_id_v1(destination.edge().target())?,
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
        // A resumed frame's incoming floor includes these consumed inputs.
        // Keep their credits paid until its enclosing payload check finishes.
        drop(signature);
        Ok((
            terminator,
            argument_sum_v1(&[header, if scoped { nominal_bytes } else { 0 }])?,
        ))
    }
}
