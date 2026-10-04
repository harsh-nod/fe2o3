// Source-aware compiler spill schema and exact constructor payload preparation.
fn source_enum_spill_components_v55(
    cursor: &ExecutionAvailabilityV29<'_>,
    local: u32,
    variant: u32,
    field: u32,
    field_type: SemanticTypeIdV1,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<Option<Vec<(SemanticTypeIdV1, Type)>>, ProductionSemanticKirErrorV1> {
    cursor.check_ledger(budget)?;
    let references = cursor.references.ok_or_else(source_enum_tag_error_v55)?;
    references.check(budget)?;
    let plan = references.plan;
    if cursor.cfg.source_enum_locals.get(local as usize) != Some(&true) {
        return Err(source_enum_tag_error_v55());
    }
    source_reference_owned_prepay_v29::<(
        Option<Vec<Type>>,
        Vec<(SemanticTypeIdV1, Type)>,
        [usize; 8],
    )>(plan, budget)?;
    let mut selected: Option<Vec<Type>> = None;
    fn merge_node(
        plan: &SourceReferencePlanV29<'_, '_>,
        node: usize,
        variant: u32,
        field: u32,
        field_type: SemanticTypeIdV1,
        selected: &mut Option<Vec<Type>>,
        budget: &mut dyn SemanticEmissionBudgetV1,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        source_enum_tag_type_v55(plan, node, budget)?;
        let SourceReferenceNodeKindV29::Enum { first, count } = plan.nodes[node].kind else {
            return Err(source_enum_tag_error_v55());
        };
        for offset in 0..count {
            budget.source_reference_charge_v29(plan, 4)?;
            let member = plan.enum_members[argument_sum_v1(&[first, offset])?];
            let alternative = &plan.enum_alternatives[member];
            if alternative.variant != variant {
                continue;
            }
            if field as usize >= alternative.count {
                return Err(source_enum_tag_error_v55());
            }
            let child = plan.children[argument_sum_v1(&[alternative.first, field as usize])?];
            if plan.nodes[child].ty != field_type {
                return Err(source_enum_tag_error_v55());
            }
            let floor = budget.storage();
            let mut components = source_reference_owned_vec_v29(plan, 0, budget)?;
            source_reference_append_node_types_v29(
                plan,
                child,
                true,
                &mut components,
                &mut 0,
                budget,
            )?;
            if let Some(previous) = selected {
                budget.source_reference_charge_v29(plan, 1)?;
                if previous.len() != components.len() {
                    return Err(source_reference_error_v29(
                        "source enum alternatives require a common checked spill representation",
                    ));
                }
                for (a, b) in previous.iter().zip(&components) {
                    if !invocation_equal_types_v1(a, b, budget)? {
                        return Err(source_reference_error_v29(
                            "source enum alternatives require a common checked spill representation",
                        ));
                    }
                }
                drop(components);
                budget.release_storage(
                    budget
                        .storage()
                        .checked_sub(floor)
                        .ok_or(ArgumentResourceV1::Accounting)?,
                )?;
            } else {
                *selected = Some(components);
            }
        }
        Ok(())
    }
    // The schema includes every complete live-in parent and every transient
    // original whole-local write, not one representative loan per variant.
    for entry in &cursor.cfg.entries {
        budget.source_reference_charge_v29(plan, 2)?;
        if entry.local == local {
            merge_node(
                plan,
                entry.reference.ok_or_else(source_enum_tag_error_v55)?,
                variant,
                field,
                field_type,
                &mut selected,
                budget,
            )?;
        }
    }
    for demand in &plan.representation_demands {
        budget.source_reference_charge_v29(plan, 4)?;
        if demand.instance == cursor.instance
            && demand.local.index() == local
            && demand.projections.is_empty()
        {
            merge_node(
                plan,
                demand.node,
                variant,
                field,
                field_type,
                &mut selected,
                budget,
            )?;
        }
    }
    let Some(selected) = selected else {
        return Ok(None);
    };
    let mut result = source_reference_owned_vec_v29(plan, selected.len(), budget)?;
    for ty in selected {
        budget.source_reference_charge_v29(plan, 1)?;
        result.push((field_type, ty));
    }
    Ok(Some(result))
}

struct PreparedSourceEnumPayloadV55 {
    field: u32,
    value: ValueDef,
    pointer: ValueId,
    alignment: u32,
    source: Option<ScopedMemoryStoreSourceV29>,
}

fn check_source_enum_constant_v55(
    plan: &SourceReferencePlanV29<'_, '_>,
    operations: &[Operation],
    value: ValueId,
    ty: &Type,
    expected: Constant,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let mut found = false;
    for operation in operations {
        budget
            .source_reference_charge_v29(plan, argument_sum_v1(&[operation.results.len(), 2])?)?;
        if !operation.results.iter().any(|result| result.id == value) {
            continue;
        }
        let [result] = operation.results.as_slice() else {
            return Err(source_enum_tag_error_v55());
        };
        if found
            || !invocation_equal_types_v1(&result.ty, ty, budget)?
            || !matches!(&operation.kind, OperationKind::Constant(actual) if actual == &expected)
        {
            return Err(source_enum_tag_error_v55());
        }
        found = true;
    }
    if found {
        Ok(())
    } else {
        Err(source_enum_tag_error_v55())
    }
}

impl SemanticFunctionLoweringV1<'_, '_> {
    fn store_source_enum_payload_v55(
        &mut self,
        block: SemanticBlockIdV1,
        statement: Option<u32>,
        local: SemanticLocalIdV1,
        binding: &SemanticValueBindingV1,
        operations: &mut Vec<Operation>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        let (variant, prepared) = self.with_emission_budget_v1(|this, budget| {
            this.prepare_source_enum_payload_v55(
                block, statement, local, binding, operations, budget,
            )
        })?;
        let site = execution_site_v29(block, statement);
        for field in prepared {
            let position = operations.len();
            self.push_memory_store_v1(
                operations,
                field.pointer,
                field.value.id,
                MemoryAccess::new(AddressSpace::Private, field.alignment),
                None,
            )?;
            self.record_scoped_enum_spill_store_v55(
                site,
                local,
                variant,
                field.field,
                0,
                position,
                field.pointer,
                field.value.id,
                field.source,
            )?;
        }
        Ok(())
    }

    fn prepare_source_enum_payload_v55(
        &self,
        block: SemanticBlockIdV1,
        statement: Option<u32>,
        local: SemanticLocalIdV1,
        value: &SemanticValueBindingV1,
        operations: &[Operation],
        budget: &mut dyn SemanticEmissionBudgetV1,
    ) -> Result<(u32, Vec<PreparedSourceEnumPayloadV55>), ProductionSemanticKirErrorV1> {
        let cursor = self
            .execution
            .as_ref()
            .ok_or_else(source_enum_tag_error_v55)?;
        cursor.check_ledger(budget)?;
        let references = cursor.references.ok_or_else(source_enum_tag_error_v55)?;
        references.check(budget)?;
        let plan = references.plan;
        let result = (|| {
            source_reference_owned_prepay_v29::<(
                Vec<PreparedSourceEnumPayloadV55>,
                [&(); 12],
                [usize; 10],
                Option<ScopedMemoryOccurrenceV29>,
            )>(plan, budget)?;
            budget.source_reference_charge_v29(plan, 18)?;
            if cursor.cfg.source_enum_locals.get(local.index() as usize) != Some(&true) {
                return Err(source_enum_tag_error_v55());
            }
            let site = execution_site_v29(block, statement);
            let Some(SemanticStatementKindV1::Assign(assignment)) =
                scoped_source_statement_v29(self.function, site)
            else {
                return Err(source_reference_error_v29(
                    "source enum spill requires its original constructor assignment",
                ));
            };
            let SemanticRvalueKindV1::Aggregate(aggregate) = assignment.value().kind() else {
                return Err(source_reference_error_v29(
                    "source enum spill requires checked whole-value transfer",
                ));
            };
            let SemanticAggregateKindV1::EnumVariant(original_variant) = aggregate.kind() else {
                return Err(source_enum_tag_error_v55());
            };
            let SemanticValueBindingV1::Enum {
                discriminant,
                discriminant_ty,
                semantic_type,
                variant: Some(variant),
                payloads,
            } = value
            else {
                return Err(source_enum_tag_error_v55());
            };
            let destination = assignment.destination();
            if destination.local() != local
                || !destination.projections().is_empty()
                || assignment.value().result_type() != destination.ty()
                || *semantic_type != destination.ty()
                || variant != original_variant
                || payloads.len() != 1
            {
                return Err(source_enum_tag_error_v55());
            }
            let (tag_type, variants) = semantic_enum_shape(self.types, *semantic_type)?;
            let original = variants
                .get(*variant as usize)
                .filter(|row| !row.is_uninhabited())
                .ok_or_else(source_enum_tag_error_v55)?;
            let expected_tag = lower_scalar_type(self.types, tag_type)?;
            if !invocation_equal_types_v1(discriminant_ty, &expected_tag, budget)? {
                return Err(source_enum_tag_error_v55());
            }
            check_source_enum_constant_v55(
                plan,
                operations,
                *discriminant,
                discriminant_ty,
                integer_constant(discriminant_ty, original.discriminant())?,
                budget,
            )?;
            let fields = payloads
                .get(variant)
                .ok_or_else(source_enum_tag_error_v55)?;
            if fields.len() != aggregate.operands().len()
                || fields.len() != original.fields().fields().len()
                || fields.len() > MAX_SSA_VALUE_COMPONENTS_V1
            {
                return Err(source_enum_tag_error_v55());
            }
            let source_site = SourceReferenceSiteV29 {
                instance: cursor.instance,
                block,
                statement: statement.map(|index| index as usize),
            };
            let key = source_reference_access_key_v29(
                source_site,
                destination,
                SourceReferenceAccessV29::Write,
            );
            charge_execution_cfg_lookup_v29(plan.representation_demand_sites.len(), budget)?;
            let rows = plan
                .representation_demand_sites
                .get(&key)
                .ok_or_else(source_enum_tag_error_v55)?;
            if rows.is_empty() {
                return Err(source_enum_tag_error_v55());
            }
            for &index in rows {
                budget.source_reference_charge_v29(plan, 7)?;
                let demand = plan
                    .representation_demands
                    .get(index)
                    .ok_or_else(source_enum_tag_error_v55)?;
                if demand.instance != cursor.instance
                    || demand.local != local
                    || !demand.projections.is_empty()
                    || demand.selector_source.is_some()
                    || plan.nodes.get(demand.node).map(|row| row.ty) != Some(*semantic_type)
                {
                    return Err(source_enum_tag_error_v55());
                }
                merge_source_enum_tag_v55(references, demand.node, value, value, budget)?;
            }
            let mut prepared = source_reference_owned_vec_v29(plan, fields.len(), budget)?;
            for (index, (binding, operand)) in fields.iter().zip(aggregate.operands()).enumerate() {
                budget.source_reference_charge_v29(plan, 12)?;
                let ty = original.fields().fields()[index];
                let field = u32::try_from(index).map_err(|_| ArgumentResourceV1::Arithmetic)?;
                let role = ExecutionOperandV29::RvalueOperand(field);
                if operand.ty() != ty {
                    return Err(source_enum_tag_error_v55());
                }
                charge_execution_cfg_lookup_v29(self.enum_payload_storage.len(), budget)?;
                let storage = self
                    .enum_payload_storage
                    .get(&(local.index(), *variant, field))
                    .ok_or_else(source_enum_tag_error_v55)?;
                let [component] = storage.components.as_ref() else {
                    return Err(source_reference_error_v29(
                        "source enum spill field requires a checked component correspondence",
                    ));
                };
                if storage.semantic_type != ty
                    || storage.compiler_issued_binding.is_some()
                    || storage.exact_enum_variant.is_some()
                {
                    return Err(source_enum_tag_error_v55());
                }
                let (actual, source) = match (binding, operand) {
                    (
                        SemanticValueBindingV1::SourceReference(binding),
                        SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place),
                    ) => {
                        let declaration = self
                            .types
                            .get(ty.index() as usize)
                            .ok_or_else(source_enum_tag_error_v55)?;
                        if !source_object_reference_field_v44(declaration) {
                            return Err(source_enum_tag_error_v55());
                        }
                        let occurrence = scoped_claimed_operand_occurrence_v47(
                            cursor, site, role, place, budget,
                        )?;
                        let original = self.source_object_current_reference_v44(
                            plan, site, role, place, occurrence, budget,
                        )?;
                        source_reference_validate_binding_v29(plan, binding, budget)?;
                        source_object_reference_same_v44(plan, binding, original, budget)?;
                        let [actual] = binding.values.as_slice() else {
                            return Err(source_enum_tag_error_v55());
                        };
                        (
                            ValueDef::new(
                                actual.id,
                                execution_cfg_clone_type_v29(&actual.ty, budget)?,
                            ),
                            ScopedMemoryOperandSourceV29::Place(occurrence),
                        )
                    }
                    (SemanticValueBindingV1::Value { id, ty: actual }, operand) => {
                        let expected = lower_scalar_type(self.types, ty)?;
                        if !invocation_equal_types_v1(actual, &expected, budget)? {
                            return Err(source_enum_tag_error_v55());
                        }
                        let source = match operand {
                            SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place) => {
                                let occurrence = scoped_claimed_operand_occurrence_v47(
                                    cursor, site, role, place, budget,
                                )?;
                                check_scoped_payload_archive_v29(
                                    &self.semantic_ssa_bindings,
                                    place,
                                    occurrence,
                                    *id,
                                    budget,
                                )?;
                                ScopedMemoryOperandSourceV29::Place(occurrence)
                            }
                            SemanticOperandV1::Constant(constant) => {
                                let SemanticConstantValueV1::Scalar(literal) = constant.value()
                                else {
                                    return Err(source_enum_tag_error_v55());
                                };
                                check_source_enum_constant_v55(
                                    plan,
                                    operations,
                                    *id,
                                    actual,
                                    lower_constant(expected, *literal)?,
                                    budget,
                                )?;
                                ScopedMemoryOperandSourceV29::Constant
                            }
                        };
                        (
                            ValueDef::new(*id, execution_cfg_clone_type_v29(actual, budget)?),
                            source,
                        )
                    }
                    _ => {
                        return Err(source_reference_error_v29(
                            "source enum spill field requires a checked operand representation",
                        ));
                    }
                };
                if !invocation_equal_types_v1(&actual.ty, &component.kernel_type, budget)? {
                    return Err(source_enum_tag_error_v55());
                }
                prepared.push(PreparedSourceEnumPayloadV55 {
                    field,
                    value: actual,
                    pointer: component.pointer,
                    alignment: component.alignment,
                    source: matches!(binding, SemanticValueBindingV1::SourceReference(_))
                        .then_some(ScopedMemoryStoreSourceV29::Operand {
                            site,
                            role,
                            ty,
                            source,
                        }),
                });
            }
            Ok((*variant, prepared))
        })();
        result.inspect_err(|error| source_reference_record_failure_v29(plan, error))
    }
}
