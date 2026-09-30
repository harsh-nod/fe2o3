// Generated field views are not original Rust projections. They retain the
// aggregate assignment and its actual operand coordinates as inert recipes.
#[derive(Clone, Copy)]
struct SourceObjectAggregateFieldV29 {
    operand: u32,
    projection: ScopedObjectViewProjectionV29,
    ty: SemanticTypeIdV1,
    schema: fe2o3_kernel_ir::StorageLayoutIdV1,
    value: ValueId,
    source: ScopedMemoryStoreSourceV29,
}

fn source_object_aggregate_field_types_v29<'types>(
    types: &'types [SemanticTypeDeclV1],
    ty: SemanticTypeIdV1,
    kind: &SemanticAggregateKindV1,
    count: usize,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<&'types [SemanticTypeIdV1], ProductionSemanticKirErrorV1> {
    budget.charge_work(6)?;
    let declaration = types
        .get(ty.index() as usize)
        .ok_or(ArgumentResourceV1::Accounting)?;
    require_ordinary_execution_representation_v29(declaration)?;
    let fields = match (declaration.shape(), kind) {
        (SemanticTypeShapeV1::Tuple(fields), SemanticAggregateKindV1::Tuple)
        | (SemanticTypeShapeV1::Aggregate(fields), SemanticAggregateKindV1::Aggregate) => {
            fields.fields()
        }
        _ => return Err(scoped_object_allocation_error_v29()),
    };
    if count == 0 || count > MAX_SSA_VALUE_COMPONENTS_V1 || count != fields.len() {
        return Err(scoped_object_allocation_error_v29());
    }
    for &field in fields {
        budget.charge_work(2)?;
        let declaration = types
            .get(field.index() as usize)
            .ok_or(ArgumentResourceV1::Accounting)?;
        require_ordinary_execution_representation_v29(declaration)?;
        if !matches!(
            declaration.shape(),
            SemanticTypeShapeV1::Scalar(_) | SemanticTypeShapeV1::ValidityScalar(_)
        ) {
            return Err(scoped_object_allocation_error_v29());
        }
    }
    Ok(fields)
}

impl SemanticFunctionLoweringV1<'_, '_> {
    #[allow(clippy::too_many_arguments)]
    fn write_source_object_aggregate_fields_v29(
        &mut self,
        block: SemanticBlockIdV1,
        statement: Option<u32>,
        place: &SemanticPlaceV1,
        destination: ScopedObjectEndpointV29,
        address: ValueId,
        access: MemoryAccess,
        fields: &[SemanticValueBindingV1],
        volatility: SemanticVolatilityV1,
        operations: &mut Vec<Operation>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        // Direct scalar fields and original scalar/thin-pointer array elements
        // share the same operand occurrence and selected-layout checks.
        // Transfers, nested aggregates, tags, and indirect roots keep their
        // existing refusal until their complete component obligations exist.
        let prepared = self.with_emission_budget_v1(|this, budget| {
            let cursor = this.execution.as_ref().ok_or(ArgumentResourceV1::Accounting)?;
            let references = cursor.references.ok_or(ArgumentResourceV1::Accounting)?;
            references.check(budget)?;
            let plan = references.plan;
            budget.source_reference_charge_v29(plan, 14)?;
            let site = execution_site_v29(block, statement);
            let Some(SemanticStatementKindV1::Assign(assignment)) = scoped_source_statement_v29(this.function, site) else {
                return Err(scoped_object_allocation_error_v29());
            };
            let SemanticRvalueKindV1::Aggregate(aggregate) = assignment.value().kind() else {
                return Err(scoped_object_allocation_error_v29());
            };
            if !std::ptr::eq(assignment.destination(), place) || !place.projections().is_empty()
                || assignment.value().result_type() != place.ty()
                || destination.root_type != place.ty() || destination.projected_type != place.ty()
                || destination.root_schema != destination.projected_schema
                || destination.path.count != 0 || destination.source_path.count != 0
                || destination.source != (ScopedObjectSourceV29::Place {
                    site, role: ExecutionOperandV29::Destination, local: place.local(), prefix: 0,
                })
                || !matches!(destination.object, ScopedObjectIdentityV29::Local { instance, local, .. }
                    if instance == cursor.instance && local == place.local())
                || volatility != SemanticVolatilityV1::NonVolatile
            { return Err(scoped_object_allocation_error_v29()); }
            budget.source_reference_charge_v29(plan, 6)?;
            let array_element = match (this.types.get(place.ty().index() as usize).map(SemanticTypeDeclV1::shape), aggregate.kind()) {
                (Some(SemanticTypeShapeV1::Array { element, length }), SemanticAggregateKindV1::Array)
                    if *length != 0 && *length <= MAX_SSA_VALUE_COMPONENTS_V1 as u64
                        && usize::try_from(*length).ok() == Some(fields.len()) => Some(*element),
                _ => None,
            };
            if array_element.is_some() {
                require_ordinary_execution_representation_v29(&this.types[place.ty().index() as usize])?;
            }
            let field_types = if array_element.is_some() { None } else {
                Some(source_object_aggregate_field_types_v29(
                    this.types, place.ty(), aggregate.kind(), fields.len(), budget)?)
            };
            if fields.len() != aggregate.operands().len()
            { return Err(scoped_object_allocation_error_v29()); }
            let mut reads = None;
            let mut prepared = source_reference_owned_vec_v29(plan, fields.len(), budget)?;
            for (index, (binding, operand)) in fields.iter().zip(aggregate.operands()).enumerate() {
                budget.source_reference_charge_v29(plan, 12)?;
                let ty = array_element.or_else(|| field_types.and_then(|types| types.get(index).copied()))
                    .ok_or_else(scoped_object_allocation_error_v29)?;
                if operand.ty() != ty
                { return Err(scoped_object_allocation_error_v29()); }
                let SemanticValueBindingV1::Value { id: value, ty: actual } = binding else {
                    return Err(scoped_object_allocation_error_v29());
                };
                let ordinal = u32::try_from(index).map_err(|_| ArgumentResourceV1::Arithmetic)?;
                let projection = SemanticProjectionV1::new(if array_element.is_some() {
                    SemanticProjectionKindV1::ConstantIndex {
                        offset: u64::from(ordinal), minimum_length: fields.len() as u64, from_end: false,
                    }
                } else { SemanticProjectionKindV1::Field(ordinal) }, ty)
                    .map_err(|_| ArgumentResourceV1::Accounting)?;
                let path = [projection];
                let components = budget.source_object_projection_v29(plan, place.ty(), destination.root_schema, &path)?;
                let [component] = components.as_slice() else { return Err(ArgumentResourceV1::Accounting.into()); };
                if component.source_type != place.ty() || component.source_schema != destination.root_schema
                    || component.result_type != ty {
                    return Err(ArgumentResourceV1::Accounting.into());
                }
                let schema = component.result_schema.ok_or(ArgumentResourceV1::Accounting)?;
                let selected_projection = match component.kind {
                    source_storage_v29::SourceSelectedComponentKindV29::Field { original, physical, .. }
                        if array_element.is_none() && original == ordinal =>
                        ScopedObjectViewProjectionV29::Field(u32::try_from(physical).map_err(|_| ArgumentResourceV1::Arithmetic)?),
                    source_storage_v29::SourceSelectedComponentKindV29::Index { length, .. }
                        if array_element.is_some() && length == fields.len() as u64 => ScopedObjectViewProjectionV29::ArrayElement,
                    _ => return Err(scoped_object_allocation_error_v29()),
                };
                let expected = if array_element.is_some() {
                    budget.source_object_original_leaf_type_v29(plan, ty, schema)?
                } else {
                    source_reference_owned_prepay_v29::<Type>(plan, budget)?;
                    lower_scalar_type(this.types, ty)?
                };
                if !invocation_equal_types_v1(actual, &expected, budget)? {
                    return Err(ArgumentResourceV1::Accounting.into());
                }
                let role = ExecutionOperandV29::RvalueOperand(ordinal);
                let source = match operand {
                    SemanticOperandV1::Constant(_) => ScopedMemoryOperandSourceV29::Constant,
                    SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place) => {
                        let occurrence = scoped_payload_occurrence_v29(cursor, site, role, place, budget)?
                            .ok_or_else(scoped_object_allocation_error_v29)?;
                        match occurrence {
                            ScopedMemoryOccurrenceV29::Promoted { .. } =>
                                check_scoped_payload_archive_v29(&this.semantic_ssa_bindings, place, occurrence, *value, budget)?,
                            ScopedMemoryOccurrenceV29::Retained { .. } => {
                                if reads.is_none() {
                                    reads = Some(this.source_object_aggregate_reads_v29(site, fields.len(), operations, budget)?);
                                }
                                budget.source_reference_charge_v29(plan, 7)?;
                                let Some((actual, read)) = reads.as_ref().and_then(|reads| reads.get(index)).copied().flatten() else {
                                    return Err(scoped_object_allocation_error_v29());
                                };
                                if !place.projections().is_empty() || actual != *value || read.site != site
                                    || read.role != role || read.ty != ty || read.prefix != 0 || read.occurrence != occurrence
                                { return Err(scoped_object_allocation_error_v29()); }
                            }
                        }
                        ScopedMemoryOperandSourceV29::Place(occurrence)
                    }
                };
                prepared.push(SourceObjectAggregateFieldV29 {
                    operand: ordinal, projection: selected_projection,
                    ty, schema, value: *value,
                    source: ScopedMemoryStoreSourceV29::Operand { site, role, ty, source },
                });
            }
            Ok(prepared)
        })?;
        for field in prepared {
            let (projected, pointer_type) = self.with_emission_budget_v1(|this, budget| {
                let plan = this
                    .execution
                    .as_ref()
                    .and_then(|cursor| cursor.references)
                    .ok_or(ArgumentResourceV1::Accounting)?
                    .plan;
                source_reference_owned_prepay_v29::<(ScopedObjectEndpointV29, Type)>(plan, budget)?;
                budget.source_reference_reserve_v29(plan, std::mem::size_of::<Type>())?;
                budget.source_reference_charge_v29(plan, 5)?;
                let path = this
                    .scoped_memory
                    .as_mut()
                    .ok_or(ArgumentResourceV1::Accounting)?
                    .anchors
                    .append_object_path(
                        &[ScopedObjectComponentV29::View {
                            projection: field.projection,
                            ty: field.ty,
                        }],
                        budget,
                    )?;
                Ok((
                    ScopedObjectEndpointV29 {
                        source: ScopedObjectSourceV29::AggregateComponent {
                            site: execution_site_v29(block, statement),
                            operand: field.operand,
                            destination: place.local(),
                            variant: None,
                        },
                        projected_type: field.ty,
                        projected_schema: field.schema,
                        path,
                        ..destination
                    },
                    Type::pointer(
                        Type::StorageObject(field.schema),
                        access.address_space,
                        AccessMode::ReadWrite,
                    ),
                ))
            })?;
            self.with_emission_budget_v1(|_, budget| {
                source_reference_emission_prepay_v29::<ScopedObjectProjectionV29>(budget)?;
                budget.charge_work(3)
            })?;
            let step = match field.projection {
                ScopedObjectViewProjectionV29::Field(physical) => {
                    ScopedObjectProjectionV29::Field(physical)
                }
                ScopedObjectViewProjectionV29::ArrayElement => {
                    ScopedObjectProjectionV29::ArrayIndex(
                        self.emit_index_constant(operations, u64::from(field.operand))?,
                    )
                }
                ScopedObjectViewProjectionV29::Variant(_) => {
                    return Err(ArgumentResourceV1::Accounting.into());
                }
            };
            let projected_address = self
                .with_scoped_object_role_v29(
                    ScopedObjectRoleV29::Project {
                        source: destination,
                        projected,
                    },
                    |this| {
                        this.emit(
                            operations,
                            pointer_type,
                            OperationKind::Storage(ScopedObjectOperationV29::Project {
                                base: address,
                                step,
                            }),
                        )
                    },
                )?
                .value()
                .map_err(|_| ArgumentResourceV1::Accounting)?
                .0;
            self.with_scoped_object_role_v29(
                ScopedObjectRoleV29::WriteValue {
                    destination: projected,
                    value: ScopedObjectValueOriginV29::Original(field.source),
                },
                |this| {
                    this.push_operation(operations, || {
                        Operation::new(
                            Vec::new(),
                            OperationKind::Storage(ScopedObjectOperationV29::WriteValue {
                                address: projected_address,
                                value: field.value,
                                access: MemoryAccess::new(access.address_space, 1),
                            }),
                        )
                    })
                },
            )?;
        }
        Ok(())
    }

    // Operand evaluation has completed, but no component write has been emitted.
    // Index only this statement's contiguous receipt suffix, once. A missing or
    // interrupted suffix refuses; earlier same-typed reads cannot satisfy it.
    fn source_object_aggregate_reads_v29(
        &self,
        site: ExecutionSiteV29,
        fields: usize,
        operations: &[Operation],
        budget: &mut dyn SemanticEmissionBudgetV1,
    ) -> Result<Vec<Option<(ValueId, ScopedMemoryReadV29)>>, ProductionSemanticKirErrorV1> {
        let cursor = self
            .execution
            .as_ref()
            .ok_or(ArgumentResourceV1::Accounting)?;
        let references = cursor.references.ok_or(ArgumentResourceV1::Accounting)?;
        let plan = references.plan;
        budget.source_reference_owner_v29(plan)?;
        if fields == 0 || fields > MAX_SSA_VALUE_COMPONENTS_V1 {
            return Err(scoped_object_allocation_error_v29());
        }
        let recorder = self
            .scoped_memory
            .as_ref()
            .ok_or(ArgumentResourceV1::Accounting)?;
        if recorder.anchors.subject != ScopedInitializationSubjectV29::from_cursor(cursor) {
            return Err(scoped_object_error_v29());
        }
        let mut reads = source_reference_owned_vec_v29(plan, fields, budget)?;
        budget.source_reference_charge_v29(plan, fields)?;
        reads.resize(fields, None);
        for (ordinal, row) in recorder.anchors.rows.iter().enumerate().rev() {
            budget.source_reference_charge_v29(plan, 1)?;
            if !row.source.is_some_and(|frame| frame.site == site) {
                break;
            }
            if Some(row.block) != recorder.block {
                return Err(scoped_object_error_v29());
            }
            let (value, read) = match row.kind {
                ScopedMemoryAnchorKindV29::Access {
                    payload: Some(ScopedMemoryPayloadV29::Load { result, read }),
                    ..
                } => {
                    let actual = operations
                        .get(row.position)
                        .ok_or_else(scoped_object_error_v29)?;
                    check_scoped_payload_v29(
                        self.function,
                        &cursor.occurrences,
                        row,
                        actual,
                        budget,
                    )?;
                    (result, read)
                }
                ScopedMemoryAnchorKindV29::Object(_) => {
                    let payload = recorder.anchors.object_payload(row, budget)?;
                    let ScopedObjectRoleV29::ReadValue {
                        read: ScopedObjectReadOriginV29::Original(read),
                        ..
                    } = payload.role
                    else {
                        continue;
                    };
                    let actual = operations
                        .get(row.position)
                        .ok_or_else(scoped_object_error_v29)?;
                    payload.check_operation(actual, budget)?;
                    recorder.anchors.check_object_source(
                        self.function,
                        &cursor.occurrences,
                        ordinal,
                        row,
                        payload,
                        budget,
                    )?;
                    (payload.result.ok_or_else(scoped_object_error_v29)?, read)
                }
                _ => continue,
            };
            source_object_aggregate_insert_read_v29(&mut reads, site, value, read, budget)?;
        }
        Ok(reads)
    }
}

fn source_object_aggregate_insert_read_v29(
    reads: &mut [Option<(ValueId, ScopedMemoryReadV29)>],
    site: ExecutionSiteV29,
    value: ValueId,
    read: ScopedMemoryReadV29,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<(), ProductionSemanticKirErrorV1> {
    budget.charge_work(5)?;
    let ExecutionOperandV29::RvalueOperand(index) = read.role else {
        return Err(scoped_object_error_v29());
    };
    let slot = reads
        .get_mut(index as usize)
        .ok_or_else(scoped_object_error_v29)?;
    if read.site != site || slot.is_some() {
        return Err(scoped_object_error_v29());
    }
    *slot = Some((value, read));
    Ok(())
}

#[cfg(test)]
#[path = "production_source_object_aggregate_emission_v29_tests.rs"]
mod aggregate_field_emission_tests_v29;
