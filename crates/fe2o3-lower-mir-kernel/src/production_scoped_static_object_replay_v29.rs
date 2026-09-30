fn check_immutable_static_object_projects_v29(
    correspondence: &ProductionSourceCorrespondenceV18<'_>,
    root: usize,
    pending: &PendingSourceMemoryV29,
    graph: &SourceAddressMemoryV29<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<()> {
    correspondence.query(budget)?;
    budget.charge_work(1)?;
    if pending.projects.len() != graph.projections.len() {
        return correspondence
            .source
            .missing("final static Project census changed");
    }
    if pending.projects.is_empty() {
        return Ok(());
    }
    budget.reserve_storage(std::mem::size_of::<Vec<bool>>())?;
    let mut seen =
        emission_vec_v1(graph.projections.len(), budget).map_err(immutable_memory_error_v29)?;
    budget.charge_work(graph.projections.len())?;
    seen.resize(graph.projections.len(), false);
    for row in &pending.projects {
        let key = TileAttachmentKeyV29 {
            root,
            family: TileAttachmentFamilyV29::MemoryAnchor,
            instance: row.instance.index(),
            row: row.anchor,
            field: TileAttachmentFieldV29::MemoryPosition,
            component: 0,
            part: 0,
        };
        let [position] = correspondence.attachment_range(key, budget)? else {
            return correspondence
                .source
                .missing("final static Project position cardinality");
        };
        let ProductionSourceOperationV18::Operation(operation) =
            correspondence.mapped_source_operation(position.location, budget)?
        else {
            return correspondence
                .source
                .missing("final static Project erased its operation");
        };
        let payload = correspondence.retained_object_payload_at_v29(
            root,
            row.instance.index(),
            row.anchor,
            operation,
            budget,
        )?;
        let ScopedObjectOperationV29::Project {
            base,
            step: ScopedObjectProjectionV29::Field(_),
        } = payload.actual.operation
        else {
            return correspondence
                .source
                .missing("final static Project changed effect family");
        };
        let result = payload
            .actual
            .result
            .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                "final static Project result",
            ))?;
        if graph
            .object_location(base, budget)
            .map_err(immutable_memory_error_v29)?
            != row.source
            || graph
                .object_location(result, budget)
                .map_err(immutable_memory_error_v29)?
                != row.projected
        {
            return correspondence
                .source
                .missing("final static Project changed original subobject");
        }
        let node = graph
            .value(result, budget)
            .map_err(immutable_memory_error_v29)?;
        budget.charge_work(call_splice_search_work_v1(graph.projections.len()))?;
        let index = graph
            .projections
            .binary_search_by_key(&node, |row| row.0)
            .map_err(|_| {
                ProductionSourceOwnedViewErrorV18::Binding(
                    "final static Project has no actual equation",
                )
            })?;
        if std::mem::replace(&mut seen[index], true) {
            return correspondence
                .source
                .missing("final static Project duplicated its source role");
        }
    }
    budget.charge_work(seen.len())?;
    if seen.iter().any(|seen| !seen) {
        return correspondence
            .source
            .missing("final static Project lacks an original role");
    }
    let bytes = argument_sum_v1(&[
        std::mem::size_of::<Vec<bool>>(),
        argument_product_v1(seen.capacity(), std::mem::size_of::<bool>())?,
    ])?;
    drop(seen);
    budget.release_storage(bytes)?;
    Ok(())
}

// Reconstructed source captures fix the original operand/SSA identity; the
// immutable inventory must still prove that a literal's actual producer has
// not changed. No cached literal or type is an expression proof.
fn check_immutable_static_object_value_v29(
    correspondence: &ProductionSourceCorrespondenceV18<'_>,
    root: usize,
    object: &SourcePhysicalObjectV18<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<()> {
    let ScopedObjectRoleV29::WriteValue {
        destination,
        value: origin,
    } = object.source.role
    else {
        return Ok(());
    };
    let ScopedObjectSourceV29::AggregateComponent {
        site,
        operand,
        destination: local,
        variant: None,
    } = destination.source
    else {
        return Ok(());
    };
    let ScopedObjectValueOriginV29::Original(ScopedMemoryStoreSourceV29::Operand {
        site: value_site,
        role: ExecutionOperandV29::RvalueOperand(value_operand),
        ty,
        source,
    }) = origin
    else {
        return correspondence
            .source
            .missing("aggregate field lost original operand role");
    };
    let ScopedObjectOperationV29::WriteValue { value, .. } = object.actual.operation else {
        return correspondence
            .source
            .missing("aggregate field changed actual write family");
    };
    let semantic = correspondence.source.source_semantic(budget)?;
    let (function, _) = correspondence
        .source
        .instance(root, object.instance, budget)?;
    budget.charge_work(10)?;
    let declaration = semantic.functions().get(function.index() as usize).ok_or(
        ProductionSourceOwnedViewErrorV18::Binding("aggregate original function"),
    )?;
    let Some(SemanticStatementKindV1::Assign(assignment)) =
        scoped_source_statement_v29(declaration, site)
    else {
        return correspondence
            .source
            .missing("aggregate original assignment");
    };
    let SemanticRvalueKindV1::Aggregate(aggregate) = assignment.value().kind() else {
        return correspondence.source.missing("aggregate original value");
    };
    let Some(original) = aggregate.operands().get(operand as usize) else {
        return correspondence.source.missing("aggregate original operand");
    };
    if value_site != site
        || value_operand != operand
        || original.ty() != ty
        || assignment.destination().local() != local
        || !assignment.destination().projections().is_empty()
        || destination.projected_type != ty
    {
        return correspondence
            .source
            .missing("aggregate field changed original operand identity");
    }
    match (source, original) {
        (
            ScopedMemoryOperandSourceV29::Place(
                ScopedMemoryOccurrenceV29::Promoted { .. }
                | ScopedMemoryOccurrenceV29::Retained { .. },
            ),
            SemanticOperandV1::Copy(_) | SemanticOperandV1::Move(_),
        ) => {
            // Reconstruction checked either the exact archived ValueId or the
            // exact earlier source read receipt through the indexed stored-read
            // join. The immutable definition/use attachment and complete memory
            // replay retain that value and the read's initialization/currentness.
            Ok(())
        }
        (ScopedMemoryOperandSourceV29::Constant, SemanticOperandV1::Constant(original)) => {
            let SemanticConstantValueV1::Scalar(bits) = original.value() else {
                return correspondence
                    .source
                    .missing("aggregate field requires scalar literal");
            };
            let expected_type =
                lower_scalar_type(semantic.types(), ty).map_err(source_emission_error_v18)?;
            let expected =
                lower_constant(expected_type.clone(), *bits).map_err(source_emission_error_v18)?;
            let owner = correspondence.source.root_row(root)?;
            let function = fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1(
                u32::try_from(owner.function_ordinal)
                    .map_err(|_| ArgumentResourceV1::Arithmetic)?,
            );
            let definition = correspondence
                .inventory
                .definition_for_value(function, value, budget)
                .map_err(source_pointer_inventory_error_v18)?
                .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                    "aggregate literal actual definition",
                ))?;
            let fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1::Result {
                operation,
                result: 0,
            } = definition.coordinate
            else {
                return correspondence
                    .source
                    .missing("aggregate literal changed actual producer");
            };
            budget.charge_work(5)?;
            let actual = correspondence
                .inventory
                .functions()
                .get(owner.function_ordinal)
                .and_then(|row| row.function.body.as_ref())
                .and_then(|body| body.blocks.get(operation.block.block as usize))
                .and_then(|block| block.operations.get(operation.operation as usize))
                .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                    "aggregate literal actual operation",
                ))?;
            if definition.ty != &expected_type
                || !matches!(&actual.kind, OperationKind::Constant(constant) if *constant == expected)
            {
                return correspondence
                    .source
                    .missing("aggregate literal differs from its original constant");
            }
            Ok(())
        }
        _ => correspondence
            .source
            .missing("aggregate field changed source value representation"),
    }
}
