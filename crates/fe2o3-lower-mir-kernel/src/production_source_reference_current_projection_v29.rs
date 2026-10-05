struct SourceReferenceRawProjectionV29 {
    previous: Option<usize>,
    first: Option<usize>,
    pointer_type: Option<SemanticTypeIdV1>,
}

impl SourceReferencePlanV29<'_, '_> {
    fn current_projection_path(
        &self,
        range: std::ops::Range<usize>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Vec<SemanticProjectionV1>, ProductionSemanticKirErrorV1> {
        let result = (|| {
            self.check_owner(self.instances, budget)?;
            self.charge(2, budget)?;
            let source = self
                .projections
                .get(range)
                .ok_or(ArgumentResourceV1::Accounting)?;
            let mut path = source_reference_emission_vec_v29(source.len(), budget)?;
            self.charge(source.len(), budget)?;
            path.extend_from_slice(source);
            Ok(path)
        })();
        result.inspect_err(|error| source_reference_record_failure_v29(self, error))
    }

    fn raw_projection_range(
        &self,
        set: usize,
        ty: SemanticTypeIdV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<std::ops::Range<usize>, ProductionSemanticKirErrorV1> {
        let result = (|| {
            self.check_owner(self.instances, budget)?;
            source_reference_emission_prepay_v29::<std::ops::Range<usize>>(budget)?;
            source_reference_emission_prepay_v29::<SourceReferenceRawProjectionV29>(budget)?;
            self.charge(3, budget)?;
            let choices = self
                .raw_sets
                .get(set)
                .ok_or(ArgumentResourceV1::Accounting)?;
            let end = argument_sum_v1(&[choices.first, choices.count])?;
            if choices.count == 0 || end > self.raw_choices.len() || choices.ty != ty {
                return Err(ArgumentResourceV1::Accounting.into());
            }
            let mut common = SourceReferenceRawProjectionV29 {
                previous: None,
                first: None,
                pointer_type: None,
            };
            for index in choices.first..end {
                self.charge(12, budget)?;
                let choice = self.raw_choices[index];
                if common
                    .previous
                    .is_some_and(|previous| previous >= choice.origin)
                {
                    return Err(ArgumentResourceV1::Accounting.into());
                }
                common.previous = Some(choice.origin);
                let origin = self
                    .raw_origins
                    .get(choice.origin)
                    .ok_or(ArgumentResourceV1::Accounting)?;
                let path_end = argument_sum_v1(&[origin.first, origin.count])?;
                let path = self
                    .projections
                    .get(origin.first..path_end)
                    .ok_or(ArgumentResourceV1::Accounting)?;
                for projection in path {
                    self.charge(1, budget)?;
                    if matches!(projection.kind(), SemanticProjectionKindV1::Index(_)) {
                        return Err(source_reference_error_v29(
                            "source raw selected address requires correlated selector transport",
                        ));
                    }
                    if matches!(projection.kind(), SemanticProjectionKindV1::Dereference) {
                        return Err(ArgumentResourceV1::Accounting.into());
                    }
                }
                if origin.instance != choices.instance
                    || origin.local != choices.local
                    || origin.ty != ty
                    || origin.parent != choices.parent
                    || origin.mutable != choices.mutable
                    || origin.count != choices.projection_count
                    || common
                        .pointer_type
                        .is_some_and(|previous| previous != origin.pointer_type)
                {
                    return Err(source_reference_error_v29(
                        "source raw alternatives do not retain one original target path",
                    ));
                }
                let Some(SemanticTypeShapeV1::Pointer(pointer)) = self
                    .instances
                    .owner()
                    .source_semantic()
                    .types()
                    .get(origin.pointer_type.index() as usize)
                    .map(|row| row.shape())
                else {
                    return Err(ArgumentResourceV1::Accounting.into());
                };
                if pointer.kind() != SemanticPointerKindV1::Raw
                    || pointer.pointee() != ty
                    || pointer.metadata() != SemanticPointerMetadataV1::None
                {
                    return Err(ArgumentResourceV1::Accounting.into());
                }
                if let Some(parent) = origin.parent {
                    self.charge(7, budget)?;
                    let loan = self
                        .loans
                        .get(parent)
                        .ok_or(ArgumentResourceV1::Accounting)?;
                    let borrowed = self
                        .origins
                        .get(loan.origin)
                        .ok_or(ArgumentResourceV1::Accounting)?;
                    let prefix = self
                        .projections
                        .get(borrowed.projections.clone())
                        .ok_or(ArgumentResourceV1::Accounting)?;
                    if borrowed.instance != origin.instance
                        || borrowed.local != origin.local
                        || borrowed.generation != origin.generation
                        || prefix.len() > path.len()
                        || (origin.mutable && loan.kind != SemanticBorrowKindV1::Mutable)
                    {
                        return Err(ArgumentResourceV1::Accounting.into());
                    }
                    for (expected, actual) in prefix.iter().zip(path) {
                        self.charge(1, budget)?;
                        if expected != actual {
                            return Err(ArgumentResourceV1::Accounting.into());
                        }
                    }
                }
                common.pointer_type = Some(origin.pointer_type);
                // An initial range is only an equality candidate. Every retained
                // original alternative is checked before this query returns it.
                if let Some(first) = common.first {
                    for (offset, projection) in path.iter().enumerate() {
                        self.charge(1, budget)?;
                        if self.projections.get(argument_sum_v1(&[first, offset])?)
                            != Some(projection)
                        {
                            return Err(source_reference_error_v29(
                                "source raw alternatives do not retain one original target path",
                            ));
                        }
                    }
                } else {
                    common.first = Some(origin.first);
                }
            }
            let first = common.first.ok_or(ArgumentResourceV1::Accounting)?;
            Ok(first..argument_sum_v1(&[first, choices.projection_count])?)
        })();
        result.inspect_err(|error| source_reference_record_failure_v29(self, error))
    }
}

struct SourceReferenceCurrentProjectionV29 {
    node: usize,
    range: std::ops::Range<usize>,
    expected: SemanticTypeIdV1,
}

impl SourceReferenceBuilderV29<'_, '_, '_> {
    fn current_projection_node(
        &mut self,
        node: usize,
        range: std::ops::Range<usize>,
        expected: SemanticTypeIdV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<usize, ProductionSemanticKirErrorV1> {
        let result = (|| {
            self.plan.check_owner(self.plan.instances, budget)?;
            source_reference_emission_prepay_v29::<SourceReferenceCurrentProjectionV29>(budget)?;
            source_reference_emission_prepay_v29::<(SourceReferenceNodeV29, SemanticProjectionV1)>(
                budget,
            )?;
            source_reference_emission_prepay_v29::<usize>(budget)?;
            self.plan.charge(2, budget)?;
            if range.start > range.end
                || range.end > self.plan.projections.len()
                || self.plan.nodes.get(node).is_none()
            {
                return Err(ArgumentResourceV1::Accounting.into());
            }
            let mut current = SourceReferenceCurrentProjectionV29 {
                node,
                range,
                expected,
            };
            for index in current.range.clone() {
                self.plan.charge(2, budget)?;
                let (row, projection) = (
                    *self
                        .plan
                        .nodes
                        .get(current.node)
                        .ok_or(ArgumentResourceV1::Accounting)?,
                    self.plan.projections[index],
                );
                let identity = matches!(
                    projection.kind(),
                    SemanticProjectionKindV1::OpaqueCast | SemanticProjectionKindV1::Subtype
                ) && projection.result_type() == row.ty;
                let next = if identity {
                    current.node
                } else {
                    match (projection.kind(), row.kind) {
                        (
                            SemanticProjectionKindV1::Downcast(_),
                            SourceReferenceNodeKindV29::Plain(_),
                        ) => {
                            let source = self.expand_plain_enum(current.node, budget)?;
                            self.project_enum_value(source, projection, budget)?
                        }
                        (
                            SemanticProjectionKindV1::Downcast(_),
                            SourceReferenceNodeKindV29::Enum { .. },
                        )
                        | (
                            SemanticProjectionKindV1::Field(_)
                            | SemanticProjectionKindV1::Downcast(_)
                            | SemanticProjectionKindV1::ConstantIndex { .. },
                            SourceReferenceNodeKindV29::EnumView(_),
                        ) => self.project_enum_value(current.node, projection, budget)?,
                        (
                            SemanticProjectionKindV1::Field(_),
                            SourceReferenceNodeKindV29::Aggregate { .. },
                        ) => {
                            self.check_current_projection_shape(row.ty, projection, budget)?;
                            let SemanticProjectionKindV1::Field(field) = projection.kind() else {
                                return Err(ArgumentResourceV1::Accounting.into());
                            };
                            let child = self.field(current.node, field as usize, budget)?;
                            if child >= current.node {
                                return Err(ArgumentResourceV1::Accounting.into());
                            }
                            child
                        }
                        (
                            SemanticProjectionKindV1::ConstantIndex { .. }
                            | SemanticProjectionKindV1::Index(_),
                            SourceReferenceNodeKindV29::Aggregate { .. },
                        ) => {
                            if matches!(projection.kind(), SemanticProjectionKindV1::Index(_)) {
                                let SourceReferenceNodeKindV29::Aggregate { first, count } =
                                    row.kind
                                else {
                                    return Err(ArgumentResourceV1::Accounting.into());
                                };
                                let end = argument_sum_v1(&[first, count])?;
                                let children = self
                                    .plan
                                    .children
                                    .get(first..end)
                                    .ok_or(ArgumentResourceV1::Accounting)?;
                                self.plan.charge(count, budget)?;
                                for &child in children {
                                    if child >= current.node
                                        || self.plan.nodes.get(child).is_none_or(|child| {
                                            child.ty != projection.result_type()
                                        })
                                    {
                                        return Err(ArgumentResourceV1::Accounting.into());
                                    }
                                }
                            }
                            let child =
                                self.array_projection_node(current.node, projection, budget)?;
                            if matches!(
                                projection.kind(),
                                SemanticProjectionKindV1::ConstantIndex { .. }
                            ) && child >= current.node
                            {
                                return Err(ArgumentResourceV1::Accounting.into());
                            }
                            child
                        }
                        (
                            SemanticProjectionKindV1::Field(_)
                            | SemanticProjectionKindV1::ConstantIndex { .. },
                            SourceReferenceNodeKindV29::Plain(_),
                        ) => {
                            self.check_current_projection_shape(row.ty, projection, budget)?;
                            source_reference_check_opaque_projection_v29(
                                self.plan.instances.owner().source_semantic().types(),
                                projection.result_type(),
                                budget,
                            )?;
                            self.plain(projection.result_type(), budget)?
                        }
                        _ => {
                            return Err(source_reference_error_v29(
                                "source reference nested referent projection requires exact cell state",
                            ));
                        }
                    }
                };
                self.plan.charge(1, budget)?;
                let child = self
                    .plan
                    .nodes
                    .get(next)
                    .ok_or(ArgumentResourceV1::Accounting)?;
                if child.ty != projection.result_type() {
                    return Err(source_reference_error_v29(
                        "source reference referent node type differs",
                    ));
                }
                // A retained opaque enum can expose a scalar, but cannot invent
                // a reference, raw origin, descriptor or nominal child fact.
                if !identity
                    && matches!(
                        row.kind,
                        SourceReferenceNodeKindV29::Enum { .. }
                            | SourceReferenceNodeKindV29::EnumView(_)
                    )
                    && matches!(child.kind, SourceReferenceNodeKindV29::Plain(_))
                    && child.descriptor.is_none()
                {
                    source_reference_check_opaque_projection_v29(
                        self.plan.instances.owner().source_semantic().types(),
                        child.ty,
                        budget,
                    )?;
                }
                current.node = next;
            }
            self.plan.charge(1, budget)?;
            if self.plan.nodes[current.node].ty != current.expected {
                return Err(source_reference_error_v29(
                    "source reference referent node type differs",
                ));
            }
            Ok(current.node)
        })();
        result.inspect_err(|error| source_reference_record_failure_v29(&self.plan, error))
    }

    fn check_current_projection_shape(
        &self,
        ty: SemanticTypeIdV1,
        projection: SemanticProjectionV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        source_reference_emission_prepay_v29::<()>(budget)?;
        self.plan.charge(4, budget)?;
        let shape = self
            .plan
            .instances
            .owner()
            .source_semantic()
            .types()
            .get(ty.index() as usize)
            .ok_or(ArgumentResourceV1::Accounting)?
            .shape();
        let valid = match (shape, projection.kind()) {
            (
                SemanticTypeShapeV1::Tuple(fields)
                | SemanticTypeShapeV1::Aggregate(fields)
                | SemanticTypeShapeV1::Union(fields),
                SemanticProjectionKindV1::Field(field),
            ) => fields.fields().get(field as usize) == Some(&projection.result_type()),
            (
                SemanticTypeShapeV1::Array { element, length },
                SemanticProjectionKindV1::ConstantIndex {
                    offset,
                    minimum_length,
                    from_end,
                },
            ) => {
                let index = if from_end {
                    length.checked_sub(offset)
                } else {
                    Some(offset)
                };
                projection.result_type() == *element
                    && minimum_length <= *length
                    && index.is_some_and(|index| index < *length)
            }
            _ => false,
        };
        if valid {
            Ok(())
        } else {
            Err(source_reference_error_v29(
                "source current projection differs from its original type",
            ))
        }
    }
}

fn source_reference_check_opaque_projection_v29(
    types: &[SemanticTypeDeclV1],
    ty: SemanticTypeIdV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    source_reference_emission_prepay_v29::<()>(budget)?;
    let mut pending = source_reference_emission_vec_v29(1, budget)?;
    pending.push(ty);
    let mut nodes = 0;
    while let Some(ty) = pending.pop() {
        execution_cfg_charge_node_v29(&mut nodes, budget)?;
        if execution_cfg_nominal_kind_v29(types, ty)?.is_some() {
            return Err(source_reference_error_v29(
                "source opaque projection lacks retained child facts",
            ));
        }
        match types
            .get(ty.index() as usize)
            .ok_or(ArgumentResourceV1::Accounting)?
            .shape()
        {
            SemanticTypeShapeV1::Unit
            | SemanticTypeShapeV1::Never
            | SemanticTypeShapeV1::Scalar(_)
            | SemanticTypeShapeV1::ValidityScalar(_) => {}
            SemanticTypeShapeV1::Tuple(fields)
            | SemanticTypeShapeV1::Aggregate(fields)
            | SemanticTypeShapeV1::Union(fields) => {
                for field in fields.fields() {
                    emission_push_v1(&mut pending, *field, budget)?;
                }
            }
            SemanticTypeShapeV1::Array { element, .. } => {
                emission_push_v1(&mut pending, *element, budget)?;
            }
            SemanticTypeShapeV1::Enum { variants, .. } => {
                for variant in variants {
                    budget.charge_work(1)?;
                    for field in variant.fields().fields() {
                        emission_push_v1(&mut pending, *field, budget)?;
                    }
                }
            }
            SemanticTypeShapeV1::Pointer(_)
            | SemanticTypeShapeV1::Slice { .. }
            | SemanticTypeShapeV1::FunctionPointer { .. }
            | SemanticTypeShapeV1::Opaque => {
                return Err(source_reference_error_v29(
                    "source opaque projection lacks retained child facts",
                ));
            }
        }
    }
    Ok(())
}
