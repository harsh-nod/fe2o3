// Original-call facts are retained separately from address-formation and launch
// premises. Neither a matching suffix nor this record authorizes native memory.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PendingSourceWriteV86 {
    instance: ProductionCallInstanceIdV1,
    block: SemanticBlockIdV1,
    anchor: usize,
    definition: Option<SsaValueV1>,
    root_parameter: usize,
    root_input: ValueId,
    receiver: ValueId,
    index: ValueId,
    value: ValueId,
    tail: CheckedWriteTailV85,
    element: ScalarType,
    index_space: SemanticDisjointIndexSpaceV1,
    disjoint: bool,
}

impl SourceIssuedAccessesV29<'_, '_, '_> {
    fn write_v86(
        &mut self,
        references: &SourceReferenceEmissionV29<'_, '_>,
        instance: ProductionCallInstanceIdV1,
        anchor: usize,
        row: &ScopedMemoryAnchorV29,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        self.check(budget)?;
        references.plan.check_owner(self.instances, budget)?;
        let before = budget.storage();
        budget.reserve_storage(argument_sum_v1(&[
            source_write_call_headers_v86()?,
            size_of::<SourceIssuedOriginalV29<'_, '_, '_>>(),
            size_of::<Result<SourceIssuedOriginalV29<'_, '_, '_>, ProductionSemanticKirErrorV1>>(),
        ])?)?;
        budget.charge_work(3)?;
        let Some(ScopedMemoryFrameV29 {
            site: ExecutionSiteV29::Terminator { block },
            role: Some(ScopedMemoryRoleV29::IntrinsicWrite),
        }) = row.source
        else {
            return Err(source_issued_error_v29());
        };
        charge_execution_cfg_lookup_v29(self.originals.len(), budget)?;
        if !self.originals.contains_key(&instance.index()) {
            let original =
                SourceIssuedOriginalV29::new(self.instances, instance, self.source_index, budget)?;
            reserve_execution_cfg_map_entry_v29::<usize, SourceIssuedOriginalV29<'_, '_, '_>>(
                self.originals.len(),
                budget,
            )?;
            self.originals.insert(instance.index(), original);
        }
        charge_execution_cfg_lookup_v29(self.originals.len(), budget)?;
        let original = self
            .originals
            .get(&instance.index())
            .ok_or_else(source_issued_error_v29)?;
        let (transport, retained) = original.actual_thread_write_v86(
            SemanticBlockIdV1::from_index(block.get()),
            anchor,
            row,
            references,
            &self.actual,
            budget,
        )?;
        emission_push_v1(&mut self.transports, transport, budget)?;
        emission_push_v1(&mut self.retained.writes, retained, budget)?;
        self.owned = argument_sum_v1(&[
            self.owned,
            budget
                .storage()
                .checked_sub(before)
                .ok_or(ArgumentResourceV1::Accounting)?,
        ])?;
        Ok(())
    }
}

fn source_write_call_headers_v86() -> Result<usize, ArgumentResourceV1> {
    type Frame<'a> = (
        [&'a (); 24],
        [usize; 10],
        [ValueId; 4],
        [SemanticTypeIdV1; 4],
        ProductionCallInstanceIdV1,
        SemanticBlockIdV1,
        Option<SsaValueV1>,
        SourceReferenceAnchorV29,
        SourceIssuedRootTransportV29,
        SourceIssuedActualValueV29<'a>,
        PendingSourceWriteV86,
        Result<(SourceIssuedRootTransportV29, PendingSourceWriteV86), ProductionSemanticKirErrorV1>,
        Result<&'a Operation, ProductionSemanticKirErrorV1>,
        Result<&'a SemanticValueBindingV1, ProductionSemanticKirErrorV1>,
        Result<SourceIssuedRootTransportV29, ProductionSemanticKirErrorV1>,
        Result<(SemanticTypeIdV1, bool), ProductionSemanticKirErrorV1>,
        Result<Type, ProductionSemanticKirErrorV1>,
        Option<Type>,
        Type,
    );
    argument_sum_v1(&[
        size_of::<Frame<'_>>(),
        std::mem::align_of::<Frame<'_>>(),
        checked_write_tail_headers_v85()?,
        source_issued_memory_payload_headers_v30()?,
    ])
}

impl SourceIssuedOriginalV29<'_, '_, '_> {
    fn actual_thread_write_v86(
        &self,
        block: SemanticBlockIdV1,
        anchor: usize,
        row: &ScopedMemoryAnchorV29,
        references: &SourceReferenceEmissionV29<'_, '_>,
        actual: &SourceIssuedActualV29<'_>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(SourceIssuedRootTransportV29, PendingSourceWriteV86), ProductionSemanticKirErrorV1>
    {
        references.check(budget)?;
        budget.charge_work(40)?;
        let source = self.instances.owner().source_semantic();
        let function = self
            .instances
            .instance(self.instance)
            .ok_or_else(source_issued_error_v29)?
            .declaration();
        let Some(SemanticTerminatorKindV1::Call(call)) = function
            .blocks()
            .get(block.index() as usize)
            .map(|b| b.terminator().kind())
        else {
            return Err(source_issued_error_v29());
        };
        let Some(SemanticCallableDeclV1::CompilerIntrinsic {
            operation:
                SemanticCompilerIntrinsicOperationV1::WriteOnlyDisjointSliceWrite {
                    disjoint_slice: carrier,
                    witness,
                    element,
                    raw_index,
                    index_space,
                    kind: SemanticWriteOnlyDisjointWriteKindV1::Thread { disjoint },
                },
            ..
        }) = source.callables().get(call.callee().index() as usize)
        else {
            return Err(source_reference_error_v29(
                "checked write mapping lacks original-call replay",
            ));
        };
        let Some(destination) = call.destination().map(|d| d.place()) else {
            return Err(source_issued_error_v29());
        };
        let Type::Scalar(scalar) = lower_scalar_type(source.types(), *element)? else {
            return Err(source_issued_error_v29());
        };
        if call.arguments().len() != 3
            || !call.variadic_argument_abis().is_empty()
            || !destination.projections().is_empty()
            || lower_scalar_type(source.types(), destination.ty())? != Type::BOOL
            || lower_scalar_type(source.types(), *raw_index)? != Type::Scalar(ScalarType::U64)
            || call.arguments()[1].ty() != *witness
            || call.arguments()[2].ty() != *element
            || matches!(call.unwind(), SemanticUnwindActionV1::Cleanup(_))
            || allocation_receiver_contract_v29(
                function,
                source.callables(),
                block,
                call,
                0,
                budget,
            )? != (*carrier, true)
        {
            return Err(source_issued_error_v29());
        }
        let (place, receiver) = self.direct_call_operand(call, block, 0, budget)?;
        let SemanticValueBindingV1::SourceReference(receiver) = receiver else {
            return Err(source_issued_error_v29());
        };
        let slice = source_reference_allocation_borrowed_v29(
            references.plan,
            source.types(),
            receiver,
            place.ty(),
            *carrier,
            true,
            budget,
        )?
        .ok_or_else(source_issued_error_v29)?;
        let loan = &references.plan.loans[receiver.origin.single_loan()?];
        let SourceReferenceRepresentationV29::ExistingAllocationBinding(allocation) =
            loan.representation
        else {
            return Err(source_issued_error_v29());
        };
        let expected =
            source_reference_anchor_type_v29(references.plan, allocation, *carrier, budget)?
                .ok_or_else(source_issued_error_v29)?;
        for ty in [&slice.ty, &expected] {
            if !matches!(ty, Type::Slice(ty)
                if ty.address_space == AddressSpace::Global && ty.access == AccessMode::WriteOnly
                    && *ty.element == Type::Scalar(scalar))
            {
                return Err(source_issued_error_v29());
            }
        }
        let transport = self.check_root_slice(allocation, slice.id, actual, budget)?;
        let (_, witness) = self.direct_call_operand(call, block, 1, budget)?;
        let SemanticValueBindingV1::IndexWitness {
            id: index,
            index_space: actual_space,
            disjoint: actual_disjoint,
            availability: None,
        } = witness
        else {
            return Err(source_issued_error_v29());
        };
        if actual_space != index_space
            || actual_disjoint != disjoint
            || *actual.value(*index, budget)?.ty != Type::INDEX
        {
            return Err(source_issued_error_v29());
        }
        let site = ExecutionSiteV29::Terminator {
            block: SsaBlockIdV1::new(block.index()),
        };
        let ScopedMemoryAnchorKindV29::Access {
            pointer,
            payload:
                Some(ScopedMemoryPayloadV29::Store {
                    value,
                    source:
                        ScopedMemoryStoreSourceV29::Operand {
                            site: payload_site,
                            role: ExecutionOperandV29::CallArgument(2),
                            ty,
                            ..
                        },
                }),
            ..
        } = row.kind
        else {
            return Err(source_issued_error_v29());
        };
        if payload_site != site
            || ty != *element
            || row.source
                != Some(ScopedMemoryFrameV29 {
                    site,
                    role: Some(ScopedMemoryRoleV29::IntrinsicWrite),
                })
            || *actual.value(value, budget)?.ty != Type::Scalar(scalar)
        {
            return Err(source_issued_error_v29());
        }
        self.source_index.frame_gap(
            self.instance,
            row.source.unwrap(),
            row.block,
            row.position,
            budget,
        )?;
        budget.charge_work(argument_product_v1(
            call_splice_search_work_v1(self.source_index.terminators.len()),
            2,
        )?)?;
        let ordinal = self
            .source_index
            .terminators
            .binary_search_by_key(
                &(self.instance.index(), block.index()),
                SourceAddressTerminatorV29::key,
            )
            .map_err(|_| source_issued_error_v29())?;
        let span = self.source_index.terminators[ordinal].span;
        let first = span
            .first_operation_ordinal
            .checked_add(span.operation_count)
            .and_then(|end| end.checked_sub(7))
            .filter(|first| *first >= span.first_operation_ordinal)
            .ok_or_else(source_issued_error_v29)?;
        let first_operation = self.source_index.emitted.operation(
            self.instance,
            span.kernel_ir_block,
            first as usize,
            budget,
        )?;
        let mut operations = [first_operation; 7];
        for (offset, operation) in operations.iter_mut().enumerate().skip(1) {
            budget.charge_work(1)?;
            *operation = self.source_index.emitted.operation(
                self.instance,
                span.kernel_ir_block,
                first as usize + offset,
                budget,
            )?;
        }
        let tail = check_checked_write_tail_v85(
            CheckedWriteInputsV85 {
                slice: slice.id,
                index: *index,
                precondition: None,
                value,
                element: scalar,
            },
            &operations,
            budget,
        )?;
        let store =
            self.source_index
                .emitted
                .operation(self.instance, row.block, row.position, budget)?;
        if pointer != tail.pointer || !std::ptr::eq(store, operations[6]) {
            return Err(source_issued_error_v29());
        }
        let occurrences = self
            .instances
            .occurrences(self.instance)
            .ok_or_else(source_issued_error_v29)?;
        check_scoped_payload_v29(function, &occurrences, row, store, budget)?;
        check_source_issued_payload_v29(self, anchor, row, store, actual, budget)?;
        let mut definition = None;
        for edge in occurrences.edge_definitions() {
            budget.charge_work(5)?;
            if edge.is_reachable()
                && edge.is_promoted()
                && edge.edge().source().get() == block.index()
                && edge.edge().ordinal() == 0
                && edge.ordinal() == 0
                && edge.variable().get() == destination.local().index()
                && definition
                    .replace(edge.value().ok_or_else(source_issued_error_v29)?)
                    .is_some()
            {
                return Err(source_issued_error_v29());
            }
        }
        if let Some(definition) = definition {
            if !matches!(self.archived(definition, budget)?, SemanticValueBindingV1::Value { id, ty }
                if *id == tail.predicate && *ty == Type::BOOL)
            {
                return Err(source_issued_error_v29());
            }
        }
        let root_parameter = actual
            .value(transport.input, budget)?
            .input
            .ok_or_else(source_issued_error_v29)?;
        Ok((
            transport,
            PendingSourceWriteV86 {
                instance: self.instance,
                block,
                anchor,
                definition,
                root_parameter,
                root_input: transport.input,
                receiver: slice.id,
                index: *index,
                value,
                tail,
                element: scalar,
                index_space: *index_space,
                disjoint: *disjoint,
            },
        ))
    }
}
