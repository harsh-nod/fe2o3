fn source_reference_array_assignment_index_v29(
    element: SemanticTypeIdV1,
    length: u64,
    projection: SemanticProjectionV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(usize, usize), ProductionSemanticKirErrorV1> {
    budget.charge_work(6)?;
    let SemanticProjectionKindV1::ConstantIndex { offset, minimum_length, from_end } = projection.kind() else {
        return Err(source_reference_error_v29("source array assignment requires a constant element"));
    };
    let selected = if from_end { length.checked_sub(offset) } else { Some(offset) };
    if projection.result_type() != element || minimum_length > length
        || selected.is_none_or(|index| index >= length)
    {
        return Err(source_reference_error_v29("source array assignment element differs from its declaration"));
    }
    Ok((
        usize::try_from(selected.unwrap()).map_err(|_| ArgumentResourceV1::Arithmetic)?,
        usize::try_from(length).map_err(|_| ArgumentResourceV1::Arithmetic)?,
    ))
}

impl SourceReferenceBuilderV29<'_, '_, '_> {
    fn install_plain_array_element_v29(
        &mut self,
        target: &SourceReferencePlaceV29,
        replacement: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<bool, ProductionSemanticKirErrorV1> {
        budget.charge_work(10)?;
        let mut local = self.local(target.instance, target.local)?;
        let Some(original) = local.node else { return Ok(false); };
        let root = *self.plan.nodes.get(original).ok_or(ArgumentResourceV1::Accounting)?;
        if !matches!(root.kind, SourceReferenceNodeKindV29::Plain(_)) { return Ok(false); }
        let types = self.plan.instances.owner().source_semantic().types();
        let Some(SemanticTypeShapeV1::Array { element, length }) = types
            .get(root.ty.index() as usize).map(SemanticTypeDeclV1::shape)
        else { return Ok(false); };
        let (element, length) = (*element, *length);
        if self.storage_root.is_none() || local.storage.is_none()
            || local.generation != target.generation || original != target.representation_root
            || root.inactive.is_some() || target.projections.len() != 1
        {
            return Err(source_reference_error_v29("source array assignment lacks its original live storage"));
        }
        let leaf = self.plan.nodes.get(target.node).ok_or(ArgumentResourceV1::Accounting)?;
        let value = self.plan.nodes.get(replacement).ok_or(ArgumentResourceV1::Accounting)?;
        if leaf.ty != element || value.ty != element
            || leaf.kind != SourceReferenceNodeKindV29::Plain(None)
            || leaf.storage.is_some() || leaf.inactive.is_some()
        {
            return Err(source_reference_error_v29("source array assignment lost its resolved element"));
        }
        let (selected, count) = source_reference_array_assignment_index_v29(
            element, length, target.projections[0], budget)?;
        // Only the original write supplies a represented element. Other leaves
        // carry no pointer facts; their initialization remains in the C2 snapshot.
        let first = self.plan.children.len();
        for index in 0..count {
            budget.charge_work(1)?;
            let child = if index == selected { replacement } else { self.plain(element, budget)? };
            emission_push_v1(&mut self.plan.children, child, budget)?;
        }
        let rebuilt = self.node(root.ty, SourceReferenceNodeKindV29::Aggregate { first, count }, budget)?;
        local.node = Some(rebuilt);
        self.set_local(target.instance, target.local, local)?;
        Ok(true)
    }
}
