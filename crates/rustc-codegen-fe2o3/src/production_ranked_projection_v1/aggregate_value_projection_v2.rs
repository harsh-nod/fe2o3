// Typed aggregate components in the existing source-to-reference expression resolver.

use super::{
    GpuSemanticExpressionResolverV2, ProductionSemanticExpressionV2, SemanticAggregateKindV1,
    SemanticLocalRoleV1, SemanticOperandV1, SemanticPlaceV1, SemanticProjectionKindV1,
    SemanticRvalueKindV1, SemanticRvalueV1, SemanticScalarTypeV1, SemanticStatementKindV1,
    SemanticTypeDeclV1, SemanticTypeIdV1, SemanticTypeShapeV1,
};
use fe2o3_mir_model::semantic_mir_v1::SemanticProjectionV1;

impl<'a> GpuSemanticExpressionResolverV2<'a> {
    pub(super) fn resolve_projected_place_v2(
        &mut self,
        place: &'a SemanticPlaceV1,
        tail: &[SemanticProjectionV1],
        depth: usize,
    ) -> Result<ProductionSemanticExpressionV2, &'static str> {
        Self::require_depth_v2(depth)?;
        if tail.is_empty()
            && let Some(load) = self.place_loads.get(&(place as *const SemanticPlaceV1))
        {
            return Ok(ProductionSemanticExpressionV2::Load(load.clone()));
        }
        let local = place.local().index();
        let declaration = self
            .function
            .locals()
            .get(local as usize)
            .ok_or("GPU semantic scalar local is out of bounds")?;
        let site = self
            .use_site
            .ok_or("GPU semantic local has no exact use site")?;
        if self
            .function
            .blocks()
            .get(site.block)
            .is_none_or(|block| site.statement > block.statements().len())
        {
            return Err("GPU semantic local use site is out of bounds");
        }
        if self.borrowed.contains(&local)
            || self
                .definitions
                .address_escaped
                .get(local as usize)
                .copied()
                != Some(false)
        {
            return Err("GPU semantic local has an escaped or borrowed address");
        }
        let count = place
            .projections()
            .len()
            .checked_add(tail.len())
            .filter(|count| *count <= fe2o3_pliron::MAX_PRODUCTION_SEMANTIC_EXPRESSION_NODES_V2)
            .ok_or("GPU semantic projection exceeds its bounded node budget")?;
        let mut projections = Vec::new();
        projections
            .try_reserve_exact(count)
            .map_err(|_| "GPU semantic projection storage cannot be reserved")?;
        let mut ty = declaration.ty();
        for projection in place.projections() {
            self.charge_v2()?;
            let projection = self.freeze_projection_v2(ty, *projection)?;
            ty = self.projected_component_v2(ty, projection)?.1;
            projections.push(projection);
        }
        if ty != place.ty() {
            return Err("GPU semantic place type disagrees with its projection");
        }
        for projection in tail {
            self.charge_v2()?;
            let projection = self.freeze_projection_v2(ty, *projection)?;
            ty = self.projected_component_v2(ty, projection)?.1;
            projections.push(projection);
        }
        self.scalar_v2(ty)?;
        if let SemanticLocalRoleV1::Argument(argument) = declaration.role() {
            if self
                .definitions
                .definition_counts
                .get(local as usize)
                .copied()
                != Some(0)
            {
                return Err("GPU semantic input argument is not an unchanged entry value");
            }
            if !place.projections().is_empty() || !tail.is_empty() {
                return Err(
                    "GPU semantic aggregate argument needs authenticated component binding",
                );
            }
            let symbol = crate::reference_effect_v1::kernel_scalar_symbol_v2(argument)
                .ok_or("kernel scalar argument exceeds the reserved semantic symbol namespace")?;
            return Ok(ProductionSemanticExpressionV2::Symbol {
                symbol,
                scalar: self.scalar_v2(declaration.ty())?,
            });
        }
        let definition = self
            .definitions
            .exact_reaching_assignment_v1(local as usize, site)
            .map_err(|_| "GPU semantic reaching-definition analysis is incomplete")?;
        let Some(definition) = definition else {
            if projections.is_empty() {
                let scalar = self
                    .scalar_calls
                    .get(local as usize)
                    .is_some_and(Option::is_some);
                let inline = self
                    .inline_calls_v30
                    .as_ref()
                    .is_some_and(|roster| roster.contains_local(local as usize));
                // The indexes select exactly one resolver. Never hide a semantic
                // refusal by attempting the other interpretation of a call.
                return match (scalar, inline) {
                    (true, false) => {
                        self.resolve_saturating_call_local_v2(local as usize, site, depth)
                    }
                    (false, true) => self.resolve_gfx942_inline_call_result_v30(place, depth),
                    (true, true) => Err("GPU scalar call belongs to inconsistent callable indexes"),
                    (false, false) => Err("GPU semantic local has no exact reaching assignment"),
                };
            }
            return Err("GPU semantic local has no exact reaching assignment");
        };
        let SemanticStatementKindV1::Assign(assignment) =
            self.function.blocks()[definition.block].statements()[definition.statement].kind()
        else {
            return Err("GPU semantic reaching definition is not an assignment");
        };
        if assignment.destination().ty() != declaration.ty()
            || assignment.value().result_type() != declaration.ty()
        {
            return Err("GPU semantic assignment type disagrees with its local");
        }
        let key = (local, definition.block, definition.statement);
        if !self.visiting.insert(key) {
            return Err("GPU semantic scalar local has a cyclic definition");
        }
        // An alias captures its operands at construction, not at the eventual store.
        let previous = self.use_site.replace(definition);
        let resolved = self.resolve_projected_rvalue_v2(assignment.value(), &projections, depth);
        self.use_site = previous;
        self.visiting.remove(&key);
        resolved
    }

    fn freeze_projection_v2(
        &mut self,
        ty: SemanticTypeIdV1,
        projection: SemanticProjectionV1,
    ) -> Result<SemanticProjectionV1, &'static str> {
        let SemanticProjectionKindV1::Index(index) = projection.kind() else {
            return Ok(projection);
        };
        let Some(SemanticTypeShapeV1::Array { length, .. }) = self
            .types
            .get(ty.index() as usize)
            .map(SemanticTypeDeclV1::shape)
        else {
            return Err("GPU semantic index does not select a fixed array");
        };
        let declaration = self
            .function
            .locals()
            .get(index.index() as usize)
            .ok_or("GPU semantic index local is out of bounds")?;
        if !matches!(
            self.types
                .get(declaration.ty().index() as usize)
                .map(SemanticTypeDeclV1::shape),
            Some(SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                signed: false,
                bits: 1..=128,
            }))
        ) {
            return Err("GPU semantic index is not an unsigned integer");
        }
        if self.borrowed.contains(&index.index()) {
            return Err("GPU semantic index has an escaped or borrowed address");
        }
        let site = self
            .use_site
            .ok_or("GPU semantic index has no exact use site")?;
        let operand = SemanticOperandV1::Copy(
            SemanticPlaceV1::new(index, vec![], declaration.ty())
                .map_err(|_| "GPU semantic index place is malformed")?,
        );
        let range = self
            .definitions
            .range_at_operand(&operand, site.block, site.statement)
            .map_err(|_| "GPU semantic index range analysis is incomplete")?
            .filter(|range| range.minimum == range.maximum && range.minimum < u128::from(*length))
            .ok_or("GPU semantic index has no exact in-bounds value")?;
        let offset = u64::try_from(range.minimum)
            .map_err(|_| "GPU semantic index exceeds its canonical width")?;
        // Index locals belong to this access, not to the aggregate's earlier definition.
        SemanticProjectionV1::new(
            SemanticProjectionKindV1::ConstantIndex {
                offset,
                minimum_length: *length,
                from_end: false,
            },
            projection.result_type(),
        )
        .map_err(|_| "GPU semantic frozen index is malformed")
    }

    fn projected_component_v2(
        &self,
        ty: SemanticTypeIdV1,
        projection: SemanticProjectionV1,
    ) -> Result<(usize, SemanticTypeIdV1), &'static str> {
        let shape = self
            .types
            .get(ty.index() as usize)
            .ok_or("GPU semantic aggregate type is out of bounds")?
            .shape();
        let (index, component) = match (shape, projection.kind()) {
            (
                SemanticTypeShapeV1::Tuple(fields) | SemanticTypeShapeV1::Aggregate(fields),
                SemanticProjectionKindV1::Field(index),
            ) => {
                let index = usize::try_from(index)
                    .map_err(|_| "GPU semantic field index exceeds host width")?;
                (
                    index,
                    *fields
                        .fields()
                        .get(index)
                        .ok_or("GPU semantic field index is out of bounds")?,
                )
            }
            (
                SemanticTypeShapeV1::Array { element, length },
                SemanticProjectionKindV1::ConstantIndex {
                    offset,
                    minimum_length,
                    from_end,
                },
            ) => {
                if minimum_length > *length {
                    return Err("GPU semantic constant index requires a larger array");
                }
                let index = if from_end {
                    length
                        .checked_sub(offset)
                        .ok_or("GPU semantic from-end index underflows")?
                } else {
                    offset
                };
                if index >= *length {
                    return Err("GPU semantic constant index is out of bounds");
                }
                (
                    usize::try_from(index)
                        .map_err(|_| "GPU semantic array index exceeds host width")?,
                    *element,
                )
            }
            _ => return Err("GPU semantic aggregate projection is unsupported"),
        };
        if projection.result_type() != component {
            return Err("GPU semantic component type disagrees with its aggregate");
        }
        Ok((index, component))
    }

    fn resolve_projected_operand_v2(
        &mut self,
        operand: &'a SemanticOperandV1,
        projections: &[SemanticProjectionV1],
        depth: usize,
    ) -> Result<ProductionSemanticExpressionV2, &'static str> {
        if projections.is_empty() {
            return self.resolve_operand_v2(operand, depth);
        }
        Self::require_depth_v2(depth)?;
        self.charge_v2()?;
        match operand {
            SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place) => {
                self.resolve_projected_place_v2(place, projections, depth)
            }
            SemanticOperandV1::Constant(_) => {
                Err("GPU semantic aggregate constant has no typed component binding")
            }
        }
    }

    fn resolve_projected_rvalue_v2(
        &mut self,
        value: &'a SemanticRvalueV1,
        projections: &[SemanticProjectionV1],
        depth: usize,
    ) -> Result<ProductionSemanticExpressionV2, &'static str> {
        let Some((projection, remaining)) = projections.split_first() else {
            return self.resolve_rvalue_inner_v2(value, depth);
        };
        Self::require_depth_v2(depth)?;
        self.charge_v2()?;
        let (index, _) = self.projected_component_v2(value.result_type(), *projection)?;
        match value.kind() {
            SemanticRvalueKindV1::Use(operand) => {
                if operand.ty() != value.result_type() {
                    return Err("GPU semantic aggregate alias changes type");
                }
                self.resolve_projected_operand_v2(operand, projections, depth + 1)
            }
            SemanticRvalueKindV1::Aggregate(aggregate) => {
                let shape = self.types[value.result_type().index() as usize].shape();
                let operands = aggregate.operands();
                match (shape, aggregate.kind()) {
                    (
                        SemanticTypeShapeV1::Array { element, length },
                        SemanticAggregateKindV1::Array,
                    ) => {
                        if u64::try_from(operands.len()).ok() != Some(*length) {
                            return Err("GPU semantic array literal has the wrong arity");
                        }
                        for operand in operands {
                            self.charge_v2()?;
                            if operand.ty() != *element {
                                return Err(
                                    "GPU semantic array literal has the wrong element type",
                                );
                            }
                        }
                    }
                    (SemanticTypeShapeV1::Tuple(fields), SemanticAggregateKindV1::Tuple)
                    | (
                        SemanticTypeShapeV1::Aggregate(fields),
                        SemanticAggregateKindV1::Aggregate,
                    ) => {
                        if operands.len() != fields.fields().len() {
                            return Err("GPU semantic aggregate literal has the wrong arity");
                        }
                        for (operand, field) in operands.iter().zip(fields.fields()) {
                            self.charge_v2()?;
                            if operand.ty() != *field {
                                return Err(
                                    "GPU semantic aggregate literal has the wrong field type",
                                );
                            }
                        }
                    }
                    _ => return Err("GPU semantic aggregate literal kind disagrees with its type"),
                }
                let operand = operands
                    .get(index)
                    .ok_or("GPU semantic aggregate literal has no selected component")?;
                self.resolve_projected_operand_v2(operand, remaining, depth + 1)
            }
            _ => Err("GPU semantic projected value has no admitted aggregate producer"),
        }
    }
}
