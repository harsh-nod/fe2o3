// Preserve facts supplied by an actual write through a mixed field/array path.
// Unrepresented siblings remain opaque; a type never supplies a pointer origin.
#[derive(Clone, Copy)]
struct SourceReferenceStaticParentV43 {
    node: usize,
    selected: usize,
    count: usize,
}

fn source_reference_static_assignment_headers_v43() -> Result<usize, ArgumentResourceV1> {
    source_reference_emission_headers_v29::<(
        SourceReferenceStaticParentV43,
        Vec<SourceReferenceStaticParentV43>,
        SourceReferenceLocalV29,
        SourceReferenceNodeV29,
        SemanticProjectionV1,
        (usize, usize, SemanticTypeIdV1),
        Option<usize>,
        [usize; 8],
        [&(); 8],
    )>()
}

impl SourceReferenceBuilderV29<'_, '_, '_> {
    fn static_assignment_component_v43(
        &self,
        node: usize,
        projection: SemanticProjectionV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(usize, usize, SemanticTypeIdV1), ProductionSemanticKirErrorV1> {
        budget.charge_work(7)?;
        let ty = self
            .plan
            .nodes
            .get(node)
            .ok_or(ArgumentResourceV1::Accounting)?
            .ty;
        let types = self.plan.instances.owner().source_semantic().types();
        if execution_cfg_nominal_kind_v29(types, ty)?.is_some() {
            return Err(source_reference_error_v29(
                "source static assignment cannot synthesize nominal child facts",
            ));
        }
        let shape = types
            .get(ty.index() as usize)
            .ok_or(ArgumentResourceV1::Accounting)?
            .shape();
        let (field, count, child) = match (shape, projection.kind()) {
            (
                SemanticTypeShapeV1::Tuple(fields) | SemanticTypeShapeV1::Aggregate(fields),
                SemanticProjectionKindV1::Field(field),
            ) => (
                field as usize,
                fields.fields().len(),
                *fields.fields().get(field as usize).ok_or_else(|| {
                    source_reference_error_v29("source static assignment field is out of range")
                })?,
            ),
            (
                SemanticTypeShapeV1::Array { element, length },
                SemanticProjectionKindV1::ConstantIndex { .. },
            ) => {
                let (field, count) = source_reference_array_assignment_index_v29(
                    *element, *length, projection, budget,
                )?;
                (field, count, *element)
            }
            _ => {
                return Err(source_reference_error_v29(
                    "source static assignment path differs from its original type",
                ));
            }
        };
        if projection.result_type() != child {
            return Err(source_reference_error_v29(
                "source static assignment child type differs",
            ));
        }
        Ok((field, count, child))
    }

    fn static_assignment_child_type_v43(
        &self,
        node: usize,
        field: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<SemanticTypeIdV1, ProductionSemanticKirErrorV1> {
        budget.charge_work(4)?;
        let ty = self
            .plan
            .nodes
            .get(node)
            .ok_or(ArgumentResourceV1::Accounting)?
            .ty;
        match self
            .plan
            .instances
            .owner()
            .source_semantic()
            .types()
            .get(ty.index() as usize)
            .ok_or(ArgumentResourceV1::Accounting)?
            .shape()
        {
            SemanticTypeShapeV1::Tuple(fields) | SemanticTypeShapeV1::Aggregate(fields) => fields
                .fields()
                .get(field)
                .copied()
                .ok_or(ArgumentResourceV1::Accounting.into()),
            SemanticTypeShapeV1::Array { element, length }
                if u64::try_from(field).is_ok_and(|field| field < *length) =>
            {
                Ok(*element)
            }
            _ => Err(ArgumentResourceV1::Accounting.into()),
        }
    }

    fn install_nested_static_components_v43(
        &mut self,
        target: &SourceReferencePlaceV29,
        mut replacement: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<bool, ProductionSemanticKirErrorV1> {
        budget.charge_work(1)?;
        if target.projections.len() < 2 {
            return Ok(false);
        }
        budget.charge_work(target.projections.len())?;
        let mut array = false;
        for projection in &target.projections {
            match projection.kind() {
                SemanticProjectionKindV1::Field(_) => (),
                SemanticProjectionKindV1::ConstantIndex { .. } => array = true,
                _ => return Ok(false),
            }
        }
        if !array {
            return Ok(false);
        }
        budget.reserve_storage(source_reference_static_assignment_headers_v43()?)?;
        budget.charge_work(8)?;
        let mut local = self.local(target.instance, target.local)?;
        let Some(mut current) = local.node else {
            return Err(source_reference_error_v29(
                "source static assignment lacks its original root",
            ));
        };
        if local.generation != target.generation || current != target.representation_root {
            return Err(source_reference_error_v29(
                "source static assignment current representation changed",
            ));
        }
        let mut parents = source_reference_scratch_v29(target.projections.len(), budget)?;
        for (depth, &projection) in target.projections.iter().enumerate() {
            budget.charge_work(6)?;
            let parent = *self
                .plan
                .nodes
                .get(current)
                .ok_or(ArgumentResourceV1::Accounting)?;
            let (selected, count, child_type) =
                self.static_assignment_component_v43(current, projection, budget)?;
            if parent.inactive.is_some() || parent.descriptor.is_some() {
                return Err(source_reference_error_v29(
                    "source static assignment cannot replace inactive or descriptor state",
                ));
            }
            parents.push(SourceReferenceStaticParentV43 {
                node: current,
                selected,
                count,
            });
            current = match parent.kind {
                SourceReferenceNodeKindV29::Aggregate {
                    first,
                    count: actual,
                } if actual == count => {
                    let child = *self
                        .plan
                        .children
                        .get(argument_sum_v1(&[first, selected])?)
                        .ok_or(ArgumentResourceV1::Accounting)?;
                    if child >= current {
                        return Err(ArgumentResourceV1::Accounting.into());
                    }
                    child
                }
                SourceReferenceNodeKindV29::Plain(_) => {
                    // Expanding an opaque parent creates no initialized child. The
                    // source storage snapshot continues to govern partial writes.
                    if self.storage_root.is_none() || local.storage.is_none() {
                        return Err(source_reference_error_v29(
                            "source static assignment lacks its original live storage",
                        ));
                    }
                    if depth + 1 == target.projections.len() {
                        let leaf = self
                            .plan
                            .nodes
                            .get(target.node)
                            .ok_or(ArgumentResourceV1::Accounting)?;
                        if leaf.kind != SourceReferenceNodeKindV29::Plain(None)
                            || leaf.storage.is_some()
                            || leaf.inactive.is_some()
                            || leaf.descriptor.is_some()
                        {
                            return Err(source_reference_error_v29(
                                "source static assignment lost its resolved opaque leaf",
                            ));
                        }
                        target.node
                    } else {
                        self.plain(child_type, budget)?
                    }
                }
                _ => {
                    return Err(source_reference_error_v29(
                        "source static assignment requires an exact component roster",
                    ));
                }
            };
            if self.plan.nodes.get(current).map(|row| row.ty) != Some(child_type) {
                return Err(source_reference_error_v29(
                    "source static assignment retained child type differs",
                ));
            }
        }
        budget.charge_work(3)?;
        if current != target.node
            || self.plan.nodes.get(current).map(|row| row.ty)
                != self.plan.nodes.get(replacement).map(|row| row.ty)
        {
            return Err(source_reference_error_v29(
                "source static assignment target changed during resolution",
            ));
        }
        while let Some(parent) = parents.pop() {
            budget.charge_work(3)?;
            let previous = self.plan.nodes[parent.node];
            let existing = match previous.kind {
                SourceReferenceNodeKindV29::Aggregate { first, .. } => Some(first),
                SourceReferenceNodeKindV29::Plain(_) => None,
                _ => return Err(ArgumentResourceV1::Accounting.into()),
            };
            let first = self.plan.children.len();
            for field in 0..parent.count {
                budget.charge_work(2)?;
                let ty = self.static_assignment_child_type_v43(parent.node, field, budget)?;
                let child = if field == parent.selected {
                    replacement
                } else if let Some(first) = existing {
                    *self
                        .plan
                        .children
                        .get(argument_sum_v1(&[first, field])?)
                        .ok_or(ArgumentResourceV1::Accounting)?
                } else {
                    self.plain(ty, budget)?
                };
                if self.plan.nodes.get(child).map(|row| row.ty) != Some(ty) {
                    return Err(ArgumentResourceV1::Accounting.into());
                }
                emission_push_v1(&mut self.plan.children, child, budget)?;
            }
            replacement = self.node(
                previous.ty,
                SourceReferenceNodeKindV29::Aggregate {
                    first,
                    count: parent.count,
                },
                budget,
            )?;
        }
        local.node = Some(replacement);
        self.set_local(target.instance, target.local, local)?;
        Ok(true)
    }
}
