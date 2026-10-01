// Construction does not assert an already-active variant. Every payload comes
// from the exact original aggregate operand; the tag is committed last.
include!("production_source_object_reference_payloads_v44.rs");

fn source_object_enum_field_types_v43<'types>(
    types: &'types [SemanticTypeDeclV1],
    ty: SemanticTypeIdV1,
    variant: u32,
    operands: &[SemanticOperandV1],
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<&'types [SemanticTypeIdV1], ProductionSemanticKirErrorV1> {
    budget.charge_work(6)?;
    let declaration = types
        .get(ty.index() as usize)
        .ok_or_else(scoped_object_error_v29)?;
    require_ordinary_execution_representation_v29(declaration)?;
    let SemanticTypeShapeV1::Enum { variants, .. } = declaration.shape() else {
        return Err(scoped_object_error_v29());
    };
    let row = variants
        .get(variant as usize)
        .filter(|row| !row.is_uninhabited())
        .ok_or_else(scoped_object_error_v29)?;
    let fields = row.fields().fields();
    if fields.len() != operands.len() || fields.len() > MAX_SSA_VALUE_COMPONENTS_V1 {
        return Err(scoped_object_pending_v29());
    }
    for (&ty, operand) in fields.iter().zip(operands) {
        budget.charge_work(3)?;
        let declaration = types
            .get(ty.index() as usize)
            .ok_or_else(scoped_object_error_v29)?;
        require_ordinary_execution_representation_v29(declaration)?;
        if operand.ty() != ty
            || !(matches!(
                declaration.shape(),
                SemanticTypeShapeV1::Scalar(_) | SemanticTypeShapeV1::ValidityScalar(_)
            ) || source_object_reference_field_v44(declaration)
                && matches!(
                    operand,
                    SemanticOperandV1::Copy(_) | SemanticOperandV1::Move(_)
                ))
        {
            return Err(scoped_object_pending_v29());
        }
    }
    Ok(fields)
}

fn source_enum_construction_location_v43(
    instances: &ExecutionInstancesV29<'_>,
    plan: &SourceReferencePlanV29<'_, '_>,
    index: &SourceAddressSourceIndexV29<'_>,
    slots: &OwnedScopedSourceSlotsV29,
    endpoint: ScopedObjectEndpointV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<Option<SourceStaticObjectLocationV29>, ProductionSemanticKirErrorV1> {
    let (site, source_local, operand, claimed_variant) = match endpoint.source {
        ScopedObjectSourceV29::Place {
            site,
            local,
            role: ExecutionOperandV29::Destination,
            prefix: 0,
            ..
        } if endpoint.path.count == 1
            && endpoint.source_path.count == 0
            && endpoint.root_schema != endpoint.projected_schema =>
        {
            (site, local, None, None)
        }
        ScopedObjectSourceV29::AggregateComponent {
            site,
            destination,
            operand,
            variant: Some(variant),
            ..
        } => (site, destination, Some(operand), Some(variant)),
        _ => return Ok(None),
    };
    plan.check_owner(instances, budget)?;
    source_reference_owned_prepay_v29::<(
        [SemanticProjectionV1; 2],
        Option<SourceStaticObjectLocationV29>,
        [ScopedObjectComponentV29; 2],
    )>(plan, budget)?;
    budget.source_reference_charge_v29(plan, 20)?;
    let ScopedObjectIdentityV29::Local {
        instance, local, ..
    } = endpoint.object
    else {
        return Err(scoped_object_error_v29());
    };
    let function = instances
        .instance(instance)
        .ok_or_else(scoped_object_error_v29)?
        .declaration();
    let Some(SemanticStatementKindV1::Assign(assignment)) =
        scoped_source_statement_v29(function, site)
    else {
        return Err(scoped_object_error_v29());
    };
    let SemanticRvalueKindV1::Aggregate(aggregate) = assignment.value().kind() else {
        return Err(scoped_object_error_v29());
    };
    let SemanticAggregateKindV1::EnumVariant(variant) = aggregate.kind() else {
        return Err(scoped_object_error_v29());
    };
    let place = assignment.destination();
    if source_local != local
        || place.local() != local
        || !place.projections().is_empty()
        || assignment.value().result_type() != place.ty()
        || endpoint.root_type != place.ty()
        || claimed_variant.is_some_and(|value| value != *variant)
        || endpoint.source_path.count != 0
    {
        return Err(scoped_object_error_v29());
    }
    let types = instances.owner().source_semantic().types();
    let declaration = types
        .get(place.ty().index() as usize)
        .ok_or_else(scoped_object_error_v29)?;
    require_ordinary_execution_representation_v29(declaration)?;
    let SemanticTypeShapeV1::Enum { variants, .. } = declaration.shape() else {
        return Err(scoped_object_error_v29());
    };
    let variant_row = variants
        .get(*variant as usize)
        .filter(|row| !row.is_uninhabited())
        .ok_or_else(scoped_object_error_v29)?;
    let fields = variant_row.fields().fields();
    if fields.len() != aggregate.operands().len() || fields.len() > MAX_SSA_VALUE_COMPONENTS_V1 {
        return Err(scoped_object_error_v29());
    }
    let ty = match operand {
        Some(operand) => {
            let ty = *fields
                .get(operand as usize)
                .ok_or_else(scoped_object_error_v29)?;
            let value = aggregate
                .operands()
                .get(operand as usize)
                .ok_or_else(scoped_object_error_v29)?;
            let declaration = types
                .get(ty.index() as usize)
                .ok_or_else(scoped_object_error_v29)?;
            let scalar = matches!(
                declaration.shape(),
                SemanticTypeShapeV1::Scalar(_) | SemanticTypeShapeV1::ValidityScalar(_)
            );
            let reference = source_object_reference_field_v44(declaration)
                && matches!(
                    value,
                    SemanticOperandV1::Copy(_) | SemanticOperandV1::Move(_)
                );
            if value.ty() != ty || !(scalar || reference) {
                return Err(scoped_object_error_v29());
            }
            ty
        }
        None if !fields.is_empty() => place.ty(),
        None => return Err(scoped_object_error_v29()),
    };
    if endpoint.projected_type != ty {
        return Err(scoped_object_error_v29());
    }
    let projections = [
        SemanticProjectionV1::new(SemanticProjectionKindV1::Downcast(*variant), place.ty())
            .map_err(|_| ArgumentResourceV1::Accounting)?,
        SemanticProjectionV1::new(SemanticProjectionKindV1::Field(operand.unwrap_or(0)), ty)
            .map_err(|_| ArgumentResourceV1::Accounting)?,
    ];
    let path = &projections[..if operand.is_some() { 2 } else { 1 }];
    let components = if operand.is_some() {
        budget.source_object_projection_v29(plan, place.ty(), endpoint.root_schema, path)?
    } else {
        source_object_projection_terminal_v46(
            plan,
            place.ty(),
            endpoint.root_schema,
            path,
            Some(*variant),
            budget,
        )?
    };
    let sidecar = index.sidecar(instance, budget)?;
    let anchors = sidecar
        .scoped_memory_anchors
        .as_ref()
        .ok_or_else(scoped_object_error_v29)?;
    let recorded = anchors.object_path(endpoint.path, budget)?;
    if recorded.len() != components.len() || recorded.len() != path.len() {
        return Err(scoped_object_error_v29());
    }
    let mut schema = endpoint.root_schema;
    let mut offset = 0u64;
    for (ordinal, component) in components.iter().enumerate() {
        budget.charge_work(8)?;
        if component.source_type != place.ty() || component.source_schema != schema {
            return Err(scoped_object_error_v29());
        }
        let expected = match component.kind {
            source_storage_v29::SourceSelectedComponentKindV29::Variant {
                original,
                physical: true,
            } if ordinal == 0 && original == *variant && component.result_type == place.ty() => {
                ScopedObjectViewProjectionV29::Variant(original)
            }
            source_storage_v29::SourceSelectedComponentKindV29::Field {
                original,
                physical,
                byte_offset,
            } if ordinal == 1 && Some(original) == operand && component.result_type == ty => {
                offset = byte_offset;
                ScopedObjectViewProjectionV29::Field(
                    u32::try_from(physical).map_err(|_| ArgumentResourceV1::Arithmetic)?,
                )
            }
            _ => return Err(scoped_object_error_v29()),
        };
        if recorded[ordinal]
            != (ScopedObjectComponentV29::View {
                projection: expected,
                ty: component.result_type,
            })
        {
            return Err(scoped_object_error_v29());
        }
        schema = component
            .result_schema
            .ok_or_else(scoped_object_error_v29)?;
    }
    if schema != endpoint.projected_schema {
        return Err(scoped_object_error_v29());
    }
    let (block, statement) = scoped_memory_site_key_v29(site);
    let slot = source_address_object_direct_v29(
        instances,
        plan,
        slots,
        SourceReferenceSiteV29 {
            instance,
            block: SemanticBlockIdV1::from_index(block),
            statement: statement.map(|value| value as usize),
        },
        place,
        SourceReferenceAccessV29::Write,
        endpoint,
        budget,
    )?;
    Ok(Some(SourceStaticObjectLocationV29 {
        slot,
        offset,
        schema: Some(schema),
    }))
}

impl SemanticFunctionLoweringV1<'_, '_> {
    #[allow(clippy::too_many_arguments)]
    fn write_source_object_enum_v43(
        &mut self,
        block: SemanticBlockIdV1,
        statement: Option<u32>,
        place: &SemanticPlaceV1,
        destination: ScopedObjectEndpointV29,
        address: ValueId,
        access: MemoryAccess,
        value: &SemanticValueBindingV1,
        volatility: SemanticVolatilityV1,
        operations: &mut Vec<Operation>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        let (variant, payload_schema, prepared) = self.with_emission_budget_v1(|this, budget| {
            let cursor = this.execution.as_ref().ok_or(ArgumentResourceV1::Accounting)?;
            let references = cursor.references.ok_or(ArgumentResourceV1::Accounting)?;
            references.check(budget)?;
            let plan = references.plan;
            source_reference_owned_prepay_v29::<(
                u32, Option<fe2o3_kernel_ir::StorageLayoutIdV1>, Vec<SourceObjectAggregateFieldV29>,
            )>(plan, budget)?;
            budget.source_reference_charge_v29(plan, 22)?;
            let site = execution_site_v29(block, statement);
            let Some(SemanticStatementKindV1::Assign(assignment)) =
                scoped_source_statement_v29(this.function, site)
            else { return Err(scoped_object_allocation_error_v29()) };
            let SemanticRvalueKindV1::Aggregate(aggregate) = assignment.value().kind()
            else { return Err(scoped_object_allocation_error_v29()) };
            let SemanticAggregateKindV1::EnumVariant(original_variant) = aggregate.kind()
            else { return Err(scoped_object_allocation_error_v29()) };
            let SemanticValueBindingV1::Enum { semantic_type, variant: Some(variant), payloads, .. } = value
            else { return Err(scoped_object_allocation_error_v29()) };
            if !std::ptr::eq(assignment.destination(), place)
                || !place.projections().is_empty()
                || assignment.value().result_type() != place.ty()
                || *semantic_type != place.ty() || variant != original_variant
                || destination.root_type != place.ty() || destination.projected_type != place.ty()
                || destination.root_schema != destination.projected_schema
                || destination.path.count != 0 || destination.source_path.count != 0
                || destination.source != (ScopedObjectSourceV29::Place {
                    site, role: ExecutionOperandV29::Destination, local: place.local(), prefix: 0,
                })
                || !matches!(destination.object, ScopedObjectIdentityV29::Local { instance, local, .. }
                    if instance == cursor.instance && local == place.local())
                || volatility != SemanticVolatilityV1::NonVolatile
            { return Err(scoped_object_allocation_error_v29()) }
            let declaration = this.types.get(place.ty().index() as usize)
                .ok_or(ArgumentResourceV1::Accounting)?;
            require_ordinary_execution_representation_v29(declaration)?;
            let SemanticTypeShapeV1::Enum { variants, .. } = declaration.shape()
            else { return Err(scoped_object_allocation_error_v29()) };
            let original = variants.get(*variant as usize)
                .filter(|row| !row.is_uninhabited()).ok_or_else(scoped_object_allocation_error_v29)?;
            let types = original.fields().fields();
            charge_execution_cfg_lookup_v29(payloads.len(), budget)?;
            let fields = payloads.get(variant).ok_or_else(scoped_object_allocation_error_v29)?;
            if fields.len() != types.len() || fields.len() != aggregate.operands().len()
                || fields.len() > MAX_SSA_VALUE_COMPONENTS_V1
            { return Err(scoped_object_allocation_error_v29()) }
            let mut prepared = source_reference_owned_vec_v29(plan, fields.len(), budget)?;
            let mut reads = None;
            let mut payload_schema = None;
            for (index, (binding, operand)) in fields.iter().zip(aggregate.operands()).enumerate() {
                budget.source_reference_charge_v29(plan, 14)?;
                let ty = types[index];
                let declaration = this.types.get(ty.index() as usize).ok_or(ArgumentResourceV1::Accounting)?;
                require_ordinary_execution_representation_v29(declaration)?;
                if operand.ty() != ty || !(matches!(declaration.shape(),
                    SemanticTypeShapeV1::Scalar(_) | SemanticTypeShapeV1::ValidityScalar(_))
                    || source_object_reference_field_v44(declaration))
                { return Err(scoped_object_allocation_error_v29()) }
                let ordinal = u32::try_from(index).map_err(|_| ArgumentResourceV1::Arithmetic)?;
                let path = [
                    SemanticProjectionV1::new(SemanticProjectionKindV1::Downcast(*variant), place.ty())
                        .map_err(|_| ArgumentResourceV1::Accounting)?,
                    SemanticProjectionV1::new(SemanticProjectionKindV1::Field(ordinal), ty)
                        .map_err(|_| ArgumentResourceV1::Accounting)?,
                ];
                let components = budget.source_object_projection_v29(plan, place.ty(), destination.root_schema, &path)?;
                let [view, field] = components.as_slice() else { return Err(ArgumentResourceV1::Accounting.into()) };
                if view.source_type != place.ty() || view.source_schema != destination.root_schema
                    || view.result_type != place.ty()
                    || !matches!(view.kind, source_storage_v29::SourceSelectedComponentKindV29::Variant {
                        original, physical: true,
                    } if original == *variant)
                    || field.source_type != place.ty() || field.source_schema != view.result_schema.ok_or(ArgumentResourceV1::Accounting)?
                    || field.result_type != ty
                { return Err(scoped_object_allocation_error_v29()) }
                let view_schema = view.result_schema.ok_or(ArgumentResourceV1::Accounting)?;
                if payload_schema.is_some_and(|schema| schema != view_schema) {
                    return Err(ArgumentResourceV1::Accounting.into());
                }
                payload_schema = Some(view_schema);
                let source_storage_v29::SourceSelectedComponentKindV29::Field { original, physical, .. } = field.kind
                else { return Err(scoped_object_allocation_error_v29()) };
                if original != ordinal { return Err(ArgumentResourceV1::Accounting.into()) }
                let schema = field.result_schema.ok_or(ArgumentResourceV1::Accounting)?;
                let role = ExecutionOperandV29::RvalueOperand(ordinal);
                let (value, reference_source) = match binding {
                    SemanticValueBindingV1::Value { id, ty: actual }
                        if !source_object_reference_field_v44(declaration) => {
                        source_reference_owned_prepay_v29::<Type>(plan, budget)?;
                        let expected = lower_scalar_type(this.types, ty)?;
                        if !invocation_equal_types_v1(actual, &expected, budget)? {
                            return Err(ArgumentResourceV1::Accounting.into());
                        }
                        (*id, None)
                    }
                    SemanticValueBindingV1::SourceReference(binding)
                        if source_object_reference_field_v44(declaration) => {
                        let (SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place)) = operand
                        else { return Err(source_object_reference_payload_error_v44()) };
                        let occurrence = scoped_claimed_operand_occurrence_v47(cursor, site, role, place, budget)?;
                        let original = this.source_object_current_reference_v44(
                            plan, site, role, place, occurrence, budget,
                        )?;
                        source_object_reference_same_v44(plan, binding, original, budget)?;
                        let value = budget.source_object_reference_value_v44(plan, original, schema)?.id;
                        (value, Some(ScopedMemoryOperandSourceV29::Place(occurrence)))
                    }
                    _ => return Err(scoped_object_allocation_error_v29()),
                };
                let source = if let Some(source) = reference_source { source } else {
                let source = match operand {
                    SemanticOperandV1::Constant(_) => ScopedMemoryOperandSourceV29::Constant,
                    SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place) => {
                        let occurrence = scoped_payload_occurrence_v29(cursor, site, role, place, budget)?
                            .ok_or_else(scoped_object_allocation_error_v29)?;
                        match occurrence {
                            ScopedMemoryOccurrenceV29::Promoted { .. } =>
                                check_scoped_payload_archive_v29(&this.semantic_ssa_bindings, place, occurrence, value, budget)?,
                            ScopedMemoryOccurrenceV29::Retained { .. } => {
                                if reads.is_none() {
                                    reads = Some(this.source_object_aggregate_reads_v29(site, fields.len(), operations, budget)?);
                                }
                                budget.source_reference_charge_v29(plan, 7)?;
                                let Some((actual, read)) = reads.as_ref().and_then(|reads| reads.get(index)).copied().flatten()
                                else { return Err(scoped_object_allocation_error_v29()) };
                                if !place.projections().is_empty() || actual != value || read.site != site
                                    || read.role != role || read.ty != ty || read.prefix != 0 || read.occurrence != occurrence
                                { return Err(scoped_object_allocation_error_v29()) }
                            }
                        }
                        ScopedMemoryOperandSourceV29::Place(occurrence)
                    }
                };
                source
                };
                prepared.push(SourceObjectAggregateFieldV29 {
                    computed: false,
                    operand: ordinal, projection: ScopedObjectViewProjectionV29::Field(
                        u32::try_from(physical).map_err(|_| ArgumentResourceV1::Arithmetic)?),
                    ty, schema, value,
                    source: ScopedMemoryStoreSourceV29::Operand { site, role, ty, source },
                });
            }
            Ok((*variant, payload_schema, prepared))
        })?;
        if let Some(schema) = payload_schema {
            let view = self.with_emission_budget_v1(|this, budget| {
                source_reference_emission_prepay_v29::<ScopedObjectEndpointV29>(budget)?;
                let path = this
                    .scoped_memory
                    .as_mut()
                    .ok_or(ArgumentResourceV1::Accounting)?
                    .anchors
                    .append_object_path(
                        &[ScopedObjectComponentV29::View {
                            projection: ScopedObjectViewProjectionV29::Variant(variant),
                            ty: place.ty(),
                        }],
                        budget,
                    )?;
                Ok(ScopedObjectEndpointV29 {
                    path,
                    projected_schema: schema,
                    ..destination
                })
            })?;
            let base = self
                .with_scoped_object_role_v29(
                    ScopedObjectRoleV29::Project {
                        source: destination,
                        projected: view,
                    },
                    |this| {
                        this.emit(
                            operations,
                            Type::pointer(
                                Type::StorageObject(schema),
                                access.address_space,
                                AccessMode::WriteOnly,
                            ),
                            OperationKind::Storage(ScopedObjectOperationV29::Project {
                                base: address,
                                step: ScopedObjectProjectionV29::VariantForWrite { index: variant },
                            }),
                        )
                    },
                )?
                .value()
                .map_err(|_| ArgumentResourceV1::Accounting)?
                .0;
            for field in prepared {
                let projected = self.with_emission_budget_v1(|this, budget| {
                    source_reference_emission_prepay_v29::<ScopedObjectEndpointV29>(budget)?;
                    let path = this
                        .scoped_memory
                        .as_mut()
                        .ok_or(ArgumentResourceV1::Accounting)?
                        .anchors
                        .append_object_path(
                            &[
                                ScopedObjectComponentV29::View {
                                    projection: ScopedObjectViewProjectionV29::Variant(variant),
                                    ty: place.ty(),
                                },
                                ScopedObjectComponentV29::View {
                                    projection: field.projection,
                                    ty: field.ty,
                                },
                            ],
                            budget,
                        )?;
                    Ok(ScopedObjectEndpointV29 {
                        source: ScopedObjectSourceV29::AggregateComponent {
                            site: execution_site_v29(block, statement),
                            operand: field.operand,
                            destination: place.local(),
                            variant: Some(variant),
                        },
                        projected_type: field.ty,
                        projected_schema: field.schema,
                        path,
                        ..destination
                    })
                })?;
                let ScopedObjectViewProjectionV29::Field(physical) = field.projection else {
                    return Err(ArgumentResourceV1::Accounting.into());
                };
                let pointer = self
                    .with_scoped_object_role_v29(
                        ScopedObjectRoleV29::Project {
                            source: view,
                            projected,
                        },
                        |this| {
                            this.emit(
                                operations,
                                Type::pointer(
                                    Type::StorageObject(field.schema),
                                    access.address_space,
                                    AccessMode::WriteOnly,
                                ),
                                OperationKind::Storage(ScopedObjectOperationV29::Project {
                                    base,
                                    step: ScopedObjectProjectionV29::Field(physical),
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
                                    address: pointer,
                                    value: field.value,
                                    access: MemoryAccess::new(access.address_space, 1),
                                }),
                            )
                        })
                    },
                )?;
            }
        }
        self.with_scoped_object_role_v29(
            ScopedObjectRoleV29::SetDiscriminant {
                destination,
                origin: ScopedObjectTagOriginV29::Aggregate(execution_site_v29(block, statement)),
                variant,
            },
            |this| {
                this.push_operation(operations, || {
                    Operation::new(
                        Vec::new(),
                        OperationKind::Storage(ScopedObjectOperationV29::SetDiscriminant {
                            address,
                            variant,
                            access: MemoryAccess::new(access.address_space, 1),
                        }),
                    )
                })
            },
        )
    }
}
