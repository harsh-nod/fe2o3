// Compiler payload storage does not grant loan authority. Rejoin the original
// constructor, its complete reference tuple and its exact archived operand.
include!("production_compiler_enum_reference_custody_v55.rs");

fn check_scoped_compiler_enum_reference_store_v55<'archive>(
    references: &SourceReferenceEmissionV29<'_, '_>,
    instance: ProductionCallInstanceIdV1,
    archive: &'archive ExecutionArchiveV29,
    checked: &CheckedScopedCompilerEnumAccessV55<'archive>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<Option<&'archive SemanticSourceReferenceBindingV29>, ProductionSemanticKirErrorV1> {
    references.check(budget)?;
    let plan = references.plan;
    archive.check_original_v29(plan.instances, instance, budget)?;
    let result = (|| {
        source_reference_owned_prepay_v29::<(
            [&(); 16],
            [usize; 12],
            SourceReferenceSiteV29,
            SourceReferenceAccessIndexKeyV29,
            ScopedMemoryOccurrenceV29,
        )>(plan, budget)?;
        budget.source_reference_charge_v29(plan, 18)?;
        let spill = checked.spill;
        let types = plan.instances.owner().source_semantic().types();
        let declaration = types
            .get(spill.field_type.index() as usize)
            .ok_or_else(source_enum_tag_error_v55)?;
        let SemanticTypeShapeV1::Pointer(pointer) = declaration.shape() else {
            return Ok(None);
        };
        if pointer.kind() != SemanticPointerKindV1::Reference {
            return Ok(None);
        }
        if pointer.metadata() != SemanticPointerMetadataV1::None || spill.component != 0 {
            return Err(source_reference_error_v29(
                "compiler enum reference spill requires one checked thin-reference component",
            ));
        }
        let ScopedCompilerEnumRoleV55::Store {
            site,
            source:
                Some(ScopedMemoryStoreSourceV29::Operand {
                    site: source_site,
                    role,
                    ty,
                    source: ScopedMemoryOperandSourceV29::Place(occurrence),
                }),
            value,
        } = checked.record.role
        else {
            return Err(source_reference_error_v29(
                "compiler enum reference spill requires its original constructor operand",
            ));
        };
        if source_site != site
            || role != ExecutionOperandV29::RvalueOperand(spill.field)
            || ty != spill.field_type
        {
            return Err(source_enum_tag_error_v55());
        }
        let function = plan
            .instances
            .instance(instance)
            .ok_or_else(source_enum_tag_error_v55)?
            .declaration();
        let Some(SemanticStatementKindV1::Assign(assignment)) =
            scoped_source_statement_v29(function, site)
        else {
            return Err(source_enum_tag_error_v55());
        };
        let SemanticRvalueKindV1::Aggregate(aggregate) = assignment.value().kind() else {
            return Err(source_enum_tag_error_v55());
        };
        if aggregate.kind() != &SemanticAggregateKindV1::EnumVariant(spill.variant)
            || assignment.destination().local().index() != spill.local
            || !assignment.destination().projections().is_empty()
            || assignment.destination().ty() != spill.source_type
            || assignment.value().result_type() != spill.source_type
        {
            return Err(source_enum_tag_error_v55());
        }
        let original_operand = scoped_source_operand_v29(function, site, role)
            .ok_or_else(source_enum_tag_error_v55)?;
        let (SemanticOperandV1::Copy(original) | SemanticOperandV1::Move(original)) =
            original_operand
        else {
            return Err(source_enum_tag_error_v55());
        };
        if original.ty() != spill.field_type {
            return Err(source_enum_tag_error_v55());
        }
        let SemanticValueBindingV1::Enum {
            semantic_type,
            variant: Some(variant),
            payloads,
            ..
        } = checked.binding
        else {
            return Err(source_enum_tag_error_v55());
        };
        if *semantic_type != spill.source_type || *variant != spill.variant || payloads.len() != 1 {
            return Err(source_enum_tag_error_v55());
        }
        let fields = payloads
            .get(variant)
            .ok_or_else(source_enum_tag_error_v55)?;
        if fields.len() != aggregate.operands().len() {
            return Err(source_enum_tag_error_v55());
        }
        let Some(SemanticValueBindingV1::SourceReference(field)) = fields.get(spill.field as usize)
        else {
            return Err(source_enum_tag_error_v55());
        };
        let (block, statement) = match site {
            ExecutionSiteV29::Statement { block, statement } => (block, statement),
            ExecutionSiteV29::Terminator { .. } => return Err(source_enum_tag_error_v55()),
        };
        let key = source_reference_access_key_v29(
            SourceReferenceSiteV29 {
                instance,
                block: SemanticBlockIdV1::from_index(block.get()),
                statement: Some(statement as usize),
            },
            assignment.destination(),
            SourceReferenceAccessV29::Write,
        );
        charge_execution_cfg_lookup_v29(plan.representation_demand_sites.len(), budget)?;
        let demands = plan
            .representation_demand_sites
            .get(&key)
            .filter(|rows| !rows.is_empty())
            .ok_or_else(source_enum_tag_error_v55)?;
        for &index in demands {
            budget.source_reference_charge_v29(plan, 7)?;
            let demand = plan
                .representation_demands
                .get(index)
                .ok_or_else(source_enum_tag_error_v55)?;
            if demand.instance != instance
                || demand.local.index() != spill.local
                || !demand.projections.is_empty()
                || demand.selector_source.is_some()
                || plan.nodes.get(demand.node).map(|row| row.ty) != Some(spill.source_type)
            {
                return Err(source_enum_tag_error_v55());
            }
            merge_source_enum_tag_v55(
                references,
                demand.node,
                checked.binding,
                checked.binding,
                budget,
            )?;
        }
        let original = source_object_archived_reference_v44(
            plan, archive, instance, site, role, original, occurrence, budget,
        )?;
        source_reference_validate_binding_v29(plan, field, budget)?;
        source_object_reference_same_v44(plan, field, original, budget)?;
        let [payload] = original.values.as_slice() else {
            return Err(source_enum_tag_error_v55());
        };
        if payload.id != value || !invocation_equal_types_v1(&payload.ty, &spill.element, budget)? {
            return Err(source_enum_tag_error_v55());
        }
        // Selected views cannot be discharged with a representative loan.
        original.origin.single_loan()?;
        Ok(Some(original))
    })();
    result.inspect_err(|error| source_reference_record_failure_v29(plan, error))
}

impl SourceReferenceCellPointerProofV29<'_, '_, '_> {
    fn check_compiler_enum_reference_stores_v55(
        &mut self,
        references: &SourceReferenceEmissionV29<'_, '_>,
        emitted: &[Option<LoweredFunctionResultV1>],
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        references.check(budget)?;
        if !std::ptr::eq(self.plan, references.plan) {
            return Err(source_enum_tag_error_v55());
        }
        let plan = self.plan;
        for (ordinal, lowered) in emitted.iter().enumerate() {
            budget.source_reference_charge_v29(plan, 5)?;
            let Some(lowered) =
                source_reference_active_emitted_v29(plan, ordinal, lowered, budget)?
            else {
                continue;
            };
            let instance = plan
                .instances
                .id_at(ordinal)
                .ok_or_else(source_enum_tag_error_v55)?;
            let anchors = lowered
                .scoped_memory_anchors
                .as_ref()
                .ok_or_else(source_enum_tag_error_v55)?;
            let archive = lowered
                .execution_observation
                .as_ref()
                .ok_or_else(source_enum_tag_error_v55)?;
            let body = lowered
                .function
                .body
                .as_ref()
                .ok_or_else(source_enum_tag_error_v55)?;
            for role in &anchors.compiler_enum {
                budget
                    .source_reference_charge_v29(plan, argument_sum_v1(&[body.blocks.len(), 2])?)?;
                let anchor = anchors
                    .rows
                    .get(role.anchor)
                    .ok_or_else(source_enum_tag_error_v55)?;
                let operation = body
                    .blocks
                    .iter()
                    .find(|block| block.id == anchor.block)
                    .and_then(|block| block.operations.get(anchor.position))
                    .ok_or_else(source_enum_tag_error_v55)?;
                let checked = check_scoped_compiler_enum_access_v55(
                    plan.instances,
                    instance,
                    anchors,
                    archive,
                    role,
                    operation,
                    budget,
                )?;
                let Some(binding) = check_scoped_compiler_enum_reference_store_v55(
                    references, instance, archive, &checked, budget,
                )?
                else {
                    continue;
                };
                let loan = binding.origin.single_loan()?;
                let Some((cell, backing)) = plan.backing_cell(loan, budget)? else {
                    continue;
                };
                if backing.kind != SourceBackingKindV29::Scalar {
                    // Generation-qualified object pointers are checked by the
                    // original-object address solver, not this scalar-cell census.
                    continue;
                }
                let [value] = binding.values.as_slice() else {
                    return Err(source_enum_tag_error_v55());
                };
                self.bind_operand(
                    cell,
                    (
                        ordinal,
                        checked.anchor.block,
                        Some(checked.anchor.position),
                        1,
                    ),
                    value.id,
                    budget,
                )?;
            }
        }
        Ok(())
    }
}
