fn source_private_write_row_v22(
    original: &ProductionSourceCorrespondenceV18<'_>,
    root: usize,
    row: &OptimizedSourceObjectV18<'_>,
    source_writes: bool,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<Option<SourceEntryWriteRowV18>> {
    if let Some(entry) = source_entry_write_row_v18(original, root, row, budget)? {
        return Ok(Some(entry));
    }
    if !source_writes {
        return Ok(None);
    }
    let ScopedObjectRoleV29::WriteValue {
        destination,
        value: ScopedObjectValueOriginV29::Original(source),
    } = row.original.source.role
    else {
        return Ok(None);
    };
    let (site, ty) = match source {
        ScopedMemoryStoreSourceV29::Assignment { site, ty }
        | ScopedMemoryStoreSourceV29::Operand { site, ty, .. } => (site, ty),
        _ => {
            return original
                .source
                .missing("private source write has unsupported original value");
        }
    };
    let semantic = original.source.source_semantic(budget)?;
    let (id, _) = original
        .source
        .instance(root, row.original.instance, budget)?;
    let function = semantic.functions().get(id.index() as usize).ok_or(
        ProductionSourceOwnedViewErrorV18::Binding("private source write original function"),
    )?;
    budget.charge_work(15)?;
    let ScopedObjectSourceV29::Place {
        site: destination_site,
        role,
        local,
        prefix,
    } = destination.source
    else {
        return original
            .source
            .missing("private source write has no exact destination place");
    };
    let place = scoped_source_place_v29(function, destination_site, role).ok_or(
        ProductionSourceOwnedViewErrorV18::Binding("private source write original destination"),
    )?;
    if site != destination_site
        || place.local() != local
        || place.ty() != ty
        || prefix as usize != place.projections().len()
        || destination.root_type != ty
        || destination.projected_type != ty
        || destination.root_schema != destination.projected_schema
        || destination.path.count != 0
        || !matches!(
            semantic
                .types()
                .get(ty.index() as usize)
                .map(SemanticTypeDeclV1::shape),
            Some(SemanticTypeShapeV1::Scalar(_))
        )
        || row
            .original
            .anchor
            .source
            .is_none_or(|frame| frame.site != site)
        || row.original.source.result.is_some()
        || row.original.actual.role != row.original.source.role
        || row.actual.role != row.original.source.role
    {
        return original
            .source
            .missing("private source write differs from its whole scalar destination");
    }
    let (
        ScopedObjectOperationV29::WriteValue {
            value: input_rhs, ..
        },
        ScopedObjectOperationV29::WriteValue {
            value: output_rhs, ..
        },
    ) = (row.original.actual.operation, row.actual.operation)
    else {
        return original
            .source
            .missing("private source write changed typed endpoints");
    };
    let scalar = kir_semantic_scalar_v1(
        &lower_scalar_type(semantic.types(), ty).map_err(source_emission_error_v18)?,
    )
    .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
        "private source write scalar type",
    ))?;
    Ok(Some(SourceEntryWriteRowV18 {
        instance: row.original.instance,
        anchor: row.original.row,
        input: row.input,
        output: row.output,
        input_rhs,
        output_rhs,
        local,
        ty,
        scalar,
        schema: destination.projected_schema,
        source_write: true,
    }))
}

impl ProductionSourceEntryWriteV18<'_> {
    fn source_value_v22(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<ScopedMemoryStoreSourceV29> {
        self.check(budget)?;
        let original = self.leaves.original.leaves.relation;
        original.retain_query((|| {
            let payload = original.retained_object_payload_at_v29(
                self.leaves.original.leaves.root,
                self.row.instance,
                self.row.anchor,
                self.row.input,
                budget,
            )?;
            budget.charge_work(1)?;
            let ScopedObjectRoleV29::WriteValue {
                value: ScopedObjectValueOriginV29::Original(source),
                ..
            } = payload.source.role
            else {
                return original
                    .source
                    .missing("private source write value role disappeared");
            };
            Ok(source)
        })())
    }
}
