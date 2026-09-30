fn source_reference_array_assignment_index_v29(
    element: SemanticTypeIdV1,
    length: u64,
    projection: SemanticProjectionV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(usize, usize), ProductionSemanticKirErrorV1> {
    budget.charge_work(6)?;
    let SemanticProjectionKindV1::ConstantIndex {
        offset,
        minimum_length,
        from_end,
    } = projection.kind()
    else {
        return Err(source_reference_error_v29(
            "source array assignment requires a constant element",
        ));
    };
    let selected = if from_end {
        length.checked_sub(offset)
    } else {
        Some(offset)
    };
    if projection.result_type() != element
        || minimum_length > length
        || selected.is_none_or(|index| index >= length)
    {
        return Err(source_reference_error_v29(
            "source array assignment element differs from its declaration",
        ));
    }
    Ok((
        usize::try_from(selected.unwrap()).map_err(|_| ArgumentResourceV1::Arithmetic)?,
        usize::try_from(length).map_err(|_| ArgumentResourceV1::Arithmetic)?,
    ))
}

fn source_reference_scalar_array_merge_headers_v29() -> Result<usize, ArgumentResourceV1> {
    argument_sum_v1(&[
        source_reference_cfg_return_headers_v29::<usize>()?,
        std::mem::size_of::<[SourceReferenceNodeV29; 2]>(),
        std::mem::size_of::<(SemanticTypeIdV1, u64)>(),
        std::mem::size_of::<Option<&SourceReferenceNodeV29>>(),
        std::mem::size_of::<Result<&SourceReferenceNodeV29, ProductionSemanticKirErrorV1>>(),
        std::mem::size_of::<Option<&SemanticTypeDeclV1>>(),
        std::mem::size_of::<Result<&SemanticTypeDeclV1, ProductionSemanticKirErrorV1>>(),
        std::mem::size_of::<Option<&[usize]>>(),
        std::mem::size_of::<Result<&[usize], ProductionSemanticKirErrorV1>>(),
        std::mem::size_of::<std::ops::Range<usize>>(),
        std::mem::size_of::<std::slice::Iter<'_, usize>>(),
        std::mem::size_of::<Result<usize, std::num::TryFromIntError>>(),
    ])
}

impl SourceReferenceBuilderV29<'_, '_, '_> {
    fn expand_plain_scalar_array_for_merge_v29(
        &mut self,
        original: usize,
        represented: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<usize, ProductionSemanticKirErrorV1> {
        budget.reserve_storage(source_reference_scalar_array_merge_headers_v29()?)?;
        budget.charge_work(12)?;
        let plain = *self
            .plan
            .nodes
            .get(original)
            .ok_or(ArgumentResourceV1::Accounting)?;
        let aggregate = *self
            .plan
            .nodes
            .get(represented)
            .ok_or(ArgumentResourceV1::Accounting)?;
        let SourceReferenceNodeKindV29::Aggregate { first, count } = aggregate.kind else {
            return Err(source_reference_error_v29(
                "source reference scalar array merge lacks its represented roster",
            ));
        };
        if plain.ty != aggregate.ty
            || !matches!(plain.kind, SourceReferenceNodeKindV29::Plain(_))
            || plain.inactive.is_some()
            || aggregate.inactive.is_some()
        {
            return Err(source_reference_error_v29(
                "source reference scalar array merge differs from its declaration",
            ));
        }
        let types = self.plan.instances.owner().source_semantic().types();
        let declaration = types
            .get(plain.ty.index() as usize)
            .ok_or(ArgumentResourceV1::Accounting)?;
        let SemanticTypeShapeV1::Array { element, length } = declaration.shape() else {
            return Err(source_reference_error_v29(
                "source reference CFG merge changes loan identity",
            ));
        };
        let (element, length) = (*element, *length);
        let element_type = types
            .get(element.index() as usize)
            .ok_or(ArgumentResourceV1::Accounting)?;
        if !matches!(
            element_type.shape(),
            SemanticTypeShapeV1::Scalar(_) | SemanticTypeShapeV1::ValidityScalar(_)
        ) {
            return Err(source_reference_error_v29(
                "source reference CFG merge changes loan identity",
            ));
        }
        let length = usize::try_from(length).map_err(|_| ArgumentResourceV1::Arithmetic)?;
        if count != length {
            return Err(source_reference_error_v29(
                "source reference scalar array merge differs from its declaration",
            ));
        }
        let end = argument_sum_v1(&[first, count])?;
        let children = self
            .plan
            .children
            .get(first..end)
            .ok_or(ArgumentResourceV1::Accounting)?;
        budget.charge_work(argument_product_v1(count, 3)?)?;
        for &child in children {
            let leaf = self
                .plan
                .nodes
                .get(child)
                .ok_or(ArgumentResourceV1::Accounting)?;
            if child >= represented
                || leaf.ty != element
                || leaf.inactive.is_some()
                || !matches!(leaf.kind, SourceReferenceNodeKindV29::Plain(_))
            {
                return Err(source_reference_error_v29(
                    "source reference scalar array merge differs from its declaration",
                ));
            }
        }
        // Dynamic writes discard component value facts. Reconstitute only the
        // original primitive roster; initialization remains in the C2 snapshot.
        let first = self.plan.children.len();
        for _ in 0..count {
            budget.charge_work(1)?;
            let child = self.plain(element, budget)?;
            emission_push_v1(&mut self.plan.children, child, budget)?;
        }
        self.node(
            plain.ty,
            SourceReferenceNodeKindV29::Aggregate { first, count },
            budget,
        )
    }

    fn install_plain_array_element_v29(
        &mut self,
        target: &SourceReferencePlaceV29,
        replacement: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<bool, ProductionSemanticKirErrorV1> {
        budget.charge_work(10)?;
        let mut local = self.local(target.instance, target.local)?;
        let Some(original) = local.node else {
            return Ok(false);
        };
        let root = *self
            .plan
            .nodes
            .get(original)
            .ok_or(ArgumentResourceV1::Accounting)?;
        if !matches!(root.kind, SourceReferenceNodeKindV29::Plain(_)) {
            return Ok(false);
        }
        let types = self.plan.instances.owner().source_semantic().types();
        let Some(SemanticTypeShapeV1::Array { element, length }) = types
            .get(root.ty.index() as usize)
            .map(SemanticTypeDeclV1::shape)
        else {
            return Ok(false);
        };
        let (element, length) = (*element, *length);
        if self.storage_root.is_none()
            || local.storage.is_none()
            || local.generation != target.generation
            || original != target.representation_root
            || root.inactive.is_some()
            || target.projections.len() != 1
        {
            return Err(source_reference_error_v29(
                "source array assignment lacks its original live storage",
            ));
        }
        let leaf = self
            .plan
            .nodes
            .get(target.node)
            .ok_or(ArgumentResourceV1::Accounting)?;
        let value = self
            .plan
            .nodes
            .get(replacement)
            .ok_or(ArgumentResourceV1::Accounting)?;
        if leaf.ty != element
            || value.ty != element
            || leaf.kind != SourceReferenceNodeKindV29::Plain(None)
            || leaf.storage.is_some()
            || leaf.inactive.is_some()
        {
            return Err(source_reference_error_v29(
                "source array assignment lost its resolved element",
            ));
        }
        let (selected, count) = source_reference_array_assignment_index_v29(
            element,
            length,
            target.projections[0],
            budget,
        )?;
        // Only the original write supplies a represented element. Other leaves
        // carry no pointer facts; their initialization remains in the C2 snapshot.
        let first = self.plan.children.len();
        for index in 0..count {
            budget.charge_work(1)?;
            let child = if index == selected {
                replacement
            } else {
                self.plain(element, budget)?
            };
            emission_push_v1(&mut self.plan.children, child, budget)?;
        }
        let rebuilt = self.node(
            root.ty,
            SourceReferenceNodeKindV29::Aggregate { first, count },
            budget,
        )?;
        local.node = Some(rebuilt);
        self.set_local(target.instance, target.local, local)?;
        Ok(true)
    }
}
