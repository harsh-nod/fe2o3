// Call metadata retains the original borrowed allocation receiver independently
// of the emitted SliceLength and its later optimized occurrence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PendingSourceLengthV76 {
    instance: ProductionCallInstanceIdV1,
    block: SemanticBlockIdV1,
    definition: Option<SsaValueV1>,
    root_parameter: usize,
    root_input: ValueId,
    receiver: ValueId,
    length: ValueId,
    element: ScalarType,
    access: AccessMode,
}

fn source_length_call_v76(
    source: &fe2o3_mir_model::semantic_mir_v1::AdmittedInertSemanticMirV1,
    call: &SemanticDirectCallV1,
) -> Option<(SemanticTypeIdV1, SemanticTypeIdV1, SemanticTypeIdV1)> {
    match source.callables().get(call.callee().index() as usize)? {
        SemanticCallableDeclV1::CompilerIntrinsic {
            operation:
                SemanticCompilerIntrinsicOperationV1::DisjointSliceLen {
                    disjoint_slice,
                    element,
                    raw_index,
                    ..
                }
                | SemanticCompilerIntrinsicOperationV1::WriteOnlyDisjointSliceLen {
                    disjoint_slice,
                    element,
                    raw_index,
                    ..
                },
            ..
        } => Some((*disjoint_slice, *element, *raw_index)),
        _ => None,
    }
}

fn source_length_call_headers_v76() -> Result<usize, ArgumentResourceV1> {
    type Frame<'a> = (
        [&'a (); 18],
        [usize; 8],
        ProductionCallInstanceIdV1,
        SemanticBlockIdV1,
        Option<SsaValueV1>,
        [SemanticTypeIdV1; 3],
        SourceReferenceAnchorV29,
        SourceIssuedRootTransportV29,
        SourceIssuedActualValueV29<'a>,
        PendingSourceLengthV76,
        Option<(SemanticTypeIdV1, SemanticTypeIdV1, SemanticTypeIdV1)>,
        Result<
            (SourceIssuedRootTransportV29, PendingSourceLengthV76),
            ProductionSemanticKirErrorV1,
        >,
        Result<Option<&'a ValueDef>, ProductionSemanticKirErrorV1>,
        Result<&'a SemanticValueBindingV1, ProductionSemanticKirErrorV1>,
        Result<SourceIssuedRootTransportV29, ProductionSemanticKirErrorV1>,
        Result<(SemanticTypeIdV1, bool), ProductionSemanticKirErrorV1>,
        Result<Type, ProductionSemanticKirErrorV1>,
        Option<Type>,
        Type,
    );
    argument_sum_v1(&[size_of::<Frame<'_>>(), std::mem::align_of::<Frame<'_>>()])
}

impl SourceIssuedOriginalV29<'_, '_, '_> {
    fn actual_length_v76(
        &self,
        block: SemanticBlockIdV1,
        definition: Option<SsaValueV1>,
        references: &SourceReferenceEmissionV29<'_, '_>,
        actual: &SourceIssuedActualV29<'_>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(SourceIssuedRootTransportV29, PendingSourceLengthV76), ProductionSemanticKirErrorV1>
    {
        references.check(budget)?;
        budget.charge_work(24)?;
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
        let (carrier, element, raw_index) =
            source_length_call_v76(source, call).ok_or_else(source_issued_error_v29)?;
        let Type::Scalar(element) = lower_scalar_type(source.types(), element)? else {
            return Err(source_issued_error_v29());
        };
        if call.arguments().len() != 1
            || !call.variadic_argument_abis().is_empty()
            || call
                .destination()
                .is_none_or(|d| d.place().ty() != raw_index || !d.place().projections().is_empty())
            || matches!(call.unwind(), SemanticUnwindActionV1::Cleanup(_))
            || lower_scalar_type(source.types(), raw_index)? != Type::Scalar(ScalarType::U64)
            || allocation_receiver_contract_v29(
                function,
                source.callables(),
                block,
                call,
                0,
                budget,
            )? != (carrier, false)
        {
            return Err(source_issued_error_v29());
        }
        let (place, receiver) = self.direct_call_operand(call, block, 0, budget)?;
        let SemanticValueBindingV1::SourceReference(receiver) = receiver else {
            return Err(source_issued_error_v29());
        };
        let value = source_reference_allocation_borrowed_v29(
            references.plan,
            source.types(),
            receiver,
            place.ty(),
            carrier,
            false,
            budget,
        )?
        .ok_or_else(source_issued_error_v29)?;
        let loan = &references.plan.loans[receiver.origin.single_loan()?];
        let SourceReferenceRepresentationV29::ExistingAllocationBinding(anchor) =
            loan.representation
        else {
            return Err(source_issued_error_v29());
        };
        let expected = source_reference_anchor_type_v29(references.plan, anchor, carrier, budget)?
            .ok_or_else(source_issued_error_v29)?;
        let Type::Slice(slice) = &expected else {
            return Err(source_issued_error_v29());
        };
        if value.ty != expected
            || slice.address_space != AddressSpace::Global
            || *slice.element != Type::Scalar(element)
        {
            return Err(source_issued_error_v29());
        }
        let transport = self.check_root_slice(anchor, value.id, actual, budget)?;
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
        let last = span
            .first_operation_ordinal
            .checked_add(span.operation_count)
            .and_then(|end| end.checked_sub(1))
            .filter(|last| *last >= span.first_operation_ordinal)
            .ok_or_else(source_issued_error_v29)?;
        let operation = self.source_index.emitted.operation(
            self.instance,
            span.kernel_ir_block,
            last as usize,
            budget,
        )?;
        let [result] = operation.results.as_slice() else {
            return Err(source_issued_error_v29());
        };
        if !matches!(&operation.kind, OperationKind::SliceLength { slice } if *slice == value.id)
            || result.ty != Type::INDEX
            || actual.value(result.id, budget)?.ty != &Type::INDEX
        {
            return Err(source_issued_error_v29());
        }
        if let Some(definition) = definition {
            if !matches!(self.archived(definition, budget)?, SemanticValueBindingV1::Value { id, ty }
                if *id == result.id && *ty == Type::INDEX)
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
            PendingSourceLengthV76 {
                instance: self.instance,
                block,
                definition,
                root_parameter,
                root_input: transport.input,
                receiver: value.id,
                length: result.id,
                element,
                access: slice.access,
            },
        ))
    }
}
