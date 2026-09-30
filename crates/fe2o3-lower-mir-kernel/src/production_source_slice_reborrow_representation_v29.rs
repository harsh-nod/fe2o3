fn source_slice_reborrow_representation_headers_v29() -> Result<usize, ArgumentResourceV1> {
    argument_sum_v1(&[
        source_reference_emission_headers_v29::<AddressSpace>()?,
        source_reference_emission_headers_v29::<Type>()?,
        // The exact expected slice retains one boxed scalar element type.
        std::mem::size_of::<Type>(),
        source_reference_emission_headers_v29::<usize>()?,
        std::mem::size_of::<ExecutionSiteV29>(),
        std::mem::size_of::<SemanticTypeIdV1>(),
        std::mem::size_of::<&ExecutionAvailabilityV29<'_>>(),
        std::mem::size_of::<&SourceReferenceEmissionV29<'_, '_>>(),
        std::mem::size_of::<&SemanticFunctionDeclV1>(),
        std::mem::size_of::<&[SemanticTypeDeclV1]>(),
        std::mem::size_of::<&[SemanticCallableDeclV1]>(),
        std::mem::size_of::<&SemanticPlaceV1>(),
        std::mem::size_of::<&mut dyn SemanticEmissionBudgetV1>(),
        std::mem::size_of::<&Type>(),
        std::mem::size_of::<&SemanticValueBindingV1>(),
        std::mem::size_of::<Result<(), ProductionSemanticKirErrorV1>>(),
    ])
}

// This lookup does not consume the use. The existing source-use/archive path
// must still consume it before binding the reborrow's destination definition.
fn source_slice_reborrow_representation_v29(
    cursor: &ExecutionAvailabilityV29<'_>,
    function: &SemanticFunctionDeclV1,
    types: &[SemanticTypeDeclV1],
    callables: &[SemanticCallableDeclV1],
    site: ExecutionSiteV29,
    result_type: SemanticTypeIdV1,
    place: &SemanticPlaceV1,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<AddressSpace, ProductionSemanticKirErrorV1> {
    let references = cursor.references.ok_or_else(source_descriptor_error_v29)?;
    references.check(budget)?;
    cursor
        .check_ledger(budget)
        .inspect_err(|error| source_reference_record_failure_v29(references.plan, error))?;
    budget.source_reference_reserve_v29(
        references.plan,
        source_slice_reborrow_representation_headers_v29()?,
    )?;
    budget.source_reference_charge_v29(references.plan, 18)?;
    let source = references.plan.instances.owner().source_semantic();
    let original = references
        .plan
        .instances
        .instance(cursor.instance)
        .ok_or_else(source_descriptor_error_v29)?;
    if !std::ptr::eq(original.declaration(), function)
        || !std::ptr::eq(cursor.function, function)
        || !std::ptr::eq(cursor.ssa, original.ssa())
        || !std::ptr::eq(types, source.types())
        || !std::ptr::eq(callables, source.callables())
        || !scoped_source_place_v29(function, site, ExecutionOperandV29::RvaluePlace)
            .is_some_and(|original| std::ptr::eq(original, place))
        || function
            .locals()
            .get(place.local().index() as usize)
            .is_none_or(|local| local.ty() != result_type)
    {
        return Err(source_descriptor_error_v29());
    }
    let Some(SemanticTypeShapeV1::Pointer(pointer)) = types
        .get(result_type.index() as usize)
        .map(SemanticTypeDeclV1::shape)
    else {
        return Err(source_descriptor_error_v29());
    };
    if pointer.kind() != SemanticPointerKindV1::Reference
        || pointer.address_space() != 0
        || pointer.pointer_width_bits() != 64
        || pointer.metadata() != SemanticPointerMetadataV1::SliceLength
        || pointer.pointee() != place.ty()
        || !matches!(place.projections(), [projection]
            if projection.kind() == SemanticProjectionKindV1::Dereference
                && projection.result_type() == place.ty())
        || !matches!(
            types
                .get(place.ty().index() as usize)
                .map(SemanticTypeDeclV1::shape),
            Some(SemanticTypeShapeV1::Slice { .. })
        )
    {
        return Err(source_descriptor_error_v29());
    }
    let ExecutionSiteV29::Statement { block, statement } = site else {
        return Err(source_descriptor_error_v29());
    };
    let Some(SemanticStatementKindV1::Assign(assignment)) = function
        .blocks()
        .get(block.get() as usize)
        .and_then(|block| block.statements().get(statement as usize))
        .map(|statement| statement.kind())
    else {
        return Err(source_descriptor_error_v29());
    };
    if assignment.destination().ty() != result_type
        || !matches!(assignment.value().kind(), SemanticRvalueKindV1::Borrow { kind, place: original }
            if std::ptr::eq(original, place)
                && matches!((kind, pointer.mutability()),
                    (SemanticBorrowKindV1::Shared, SemanticMutabilityV1::Immutable)
                    | (SemanticBorrowKindV1::Mutable, SemanticMutabilityV1::Mutable)))
    {
        return Err(source_descriptor_error_v29());
    }
    let occurrence = cursor
        .event(
            site,
            ExecutionOperandV29::RvaluePlace,
            ExecutionEventV29::BaseUse,
            budget,
        )
        .inspect_err(|error| source_reference_record_failure_v29(references.plan, error))?;
    references.plan.descriptor_value_space(
        cursor.instance,
        occurrence,
        place,
        0,
        result_type,
        budget,
    )
}

fn check_source_slice_reborrow_binding_v29(
    expected: &Type,
    binding: &SemanticValueBindingV1,
) -> Result<(), ProductionSemanticKirErrorV1> {
    if matches!(binding, SemanticValueBindingV1::Value { ty, .. }
        if matches!(ty, Type::Slice(_)) && ty == expected)
    {
        Ok(())
    } else {
        Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
    }
}

impl SemanticFunctionLoweringV1<'_, '_> {
    fn source_slice_reborrow_expected_v29(
        &mut self,
        block: SemanticBlockIdV1,
        statement: Option<u32>,
        result_type: SemanticTypeIdV1,
        place: &SemanticPlaceV1,
    ) -> Result<Type, ProductionSemanticKirErrorV1> {
        if self
            .execution
            .as_ref()
            .and_then(|cursor| cursor.references)
            .is_none()
        {
            return lower_parameter_type(self.types, &[], result_type);
        }
        let representation = self.with_emission_budget_v1(|this, budget| {
            source_slice_reborrow_representation_v29(
                this.execution
                    .as_ref()
                    .ok_or_else(source_descriptor_error_v29)?,
                this.function,
                this.types,
                this.callables,
                execution_site_v29(block, statement),
                result_type,
                place,
                budget,
            )
        })?;
        let mut expected = lower_parameter_type(self.types, &[], result_type)?;
        let Type::Slice(slice) = &mut expected else {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        };
        slice.address_space = representation;
        Ok(expected)
    }
}
