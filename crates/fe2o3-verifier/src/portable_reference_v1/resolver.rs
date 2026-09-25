//! Shared live-extraction/replay resolver, including safe-helper substitution.

use super::*;

pub struct ReferenceExpressionResolverV1<'a> {
    effect_ir: &'a ReferenceEffectIrV1,
    definitions: BTreeMap<u32, &'a ReferenceValueV1>,
    ambiguous_definitions: BTreeSet<u32>,
}

impl<'a> ReferenceExpressionResolverV1<'a> {
    pub fn new(
        meter: &impl ReferenceWorkV1,
        effect_ir: &'a ReferenceEffectIrV1,
    ) -> Result<Self, ReferenceBindingErrorV1> {
        meter.charge(1)?;
        let mut definitions = BTreeMap::new();
        let mut ambiguous_definitions = BTreeSet::new();
        for block in &effect_ir.blocks {
            meter.charge(1)?;
            for assignment in &block.assignments {
                meter.charge(1)?;
                if !assignment.destination.projection.is_empty() {
                    continue;
                }
                if assignment.destination.local > 0
                    && assignment.destination.local <= effect_ir.argument_count
                {
                    return Err(ReferenceBindingErrorV1::new(format!(
                        "reference effect reassigns logical argument {}; mutable argument-local normalization is outside reference-effect V1",
                        assignment.destination.local,
                    )));
                }
                meter.tree::<(u32, &ReferenceValueV1)>(definitions.len())?;
                meter.tree::<u32>(ambiguous_definitions.len())?;
                if definitions
                    .insert(assignment.destination.local, &assignment.value)
                    .is_some()
                {
                    ambiguous_definitions.insert(assignment.destination.local);
                }
            }
        }
        Ok(Self {
            effect_ir,
            definitions,
            ambiguous_definitions,
        })
    }

    pub fn resolve_local_v1(
        &self,
        meter: &impl ReferenceWorkV1,
        local: u32,
    ) -> Result<ReferenceEffectExpressionV1, ReferenceBindingErrorV1> {
        self.resolve_local_inner_v1(meter, local, &mut BTreeSet::new(), &mut 0, 0)
    }

    pub fn resolve_value_v1(
        &self,
        meter: &impl ReferenceWorkV1,
        value: &ReferenceValueV1,
    ) -> Result<ReferenceEffectExpressionV1, ReferenceBindingErrorV1> {
        self.resolve_value_inner_v1(meter, value, &mut BTreeSet::new(), &mut 0, 0)
    }

    pub(super) fn charge_node_v1(
        meter: &impl ReferenceWorkV1,
        work: &mut usize,
    ) -> Result<(), ReferenceBindingErrorV1> {
        meter.rows::<ReferenceEffectExpressionV1>(2)?;
        *work = work
            .checked_add(1)
            .ok_or_else(|| ReferenceBindingErrorV1::new("reference expression work overflowed"))?;
        if *work > MAX_REFERENCE_EXPRESSION_NODES_V1 {
            return Err(ReferenceBindingErrorV1::new(format!(
                "reference effect expression exceeds {MAX_REFERENCE_EXPRESSION_NODES_V1} nodes",
            )));
        }
        Ok(())
    }

    pub fn require_depth_v1(depth: usize) -> Result<(), ReferenceBindingErrorV1> {
        if depth > fe2o3_pliron::MAX_PRODUCTION_SEMANTIC_EXPRESSION_DEPTH_V2 {
            return Err(ReferenceBindingErrorV1::new(format!(
                "reference effect expression exceeds {} resolution levels",
                fe2o3_pliron::MAX_PRODUCTION_SEMANTIC_EXPRESSION_DEPTH_V2,
            )));
        }
        Ok(())
    }

    pub(super) fn resolve_local_inner_v1(
        &self,
        meter: &impl ReferenceWorkV1,
        local: u32,
        visiting: &mut BTreeSet<u32>,
        work: &mut usize,
        depth: usize,
    ) -> Result<ReferenceEffectExpressionV1, ReferenceBindingErrorV1> {
        Self::require_depth_v1(depth)?;
        Self::charge_node_v1(meter, work)?;
        meter.product(self.effect_ir.relations.len(), 3)?;
        meter.tree::<u32>(self.ambiguous_definitions.len())?;
        meter.tree::<(u32, &ReferenceValueV1)>(self.definitions.len())?;
        meter.tree::<u32>(visiting.len())?;
        if local > 0 && local <= self.effect_ir.argument_count {
            let reference_argument = local - 1;
            if let Some((axis, _)) =
                self.effect_ir
                    .relations
                    .iter()
                    .find_map(|relation| match relation {
                        ReferenceArgumentRelationV1::PointCoordinate {
                            reference_argument: actual,
                            axis,
                        } if *actual == reference_argument => Some((*axis, *actual)),
                        _ => None,
                    })
            {
                return Ok(ReferenceEffectExpressionV1::PointCoordinate { axis });
            }
            let point_count = self.effect_ir.point_coordinate_count_v1()?;
            let kernel_argument = reference_argument.checked_sub(point_count).ok_or_else(|| {
                ReferenceBindingErrorV1::new("reference argument has no logical ABI relation")
            })?;
            return match self
                .effect_ir
                .relations
                .iter()
                .find(|relation| match relation {
                    ReferenceArgumentRelationV1::ScalarInput { argument, .. }
                    | ReferenceArgumentRelationV1::SharedSliceInput { argument, .. }
                    | ReferenceArgumentRelationV1::DisjointOutputSlice { argument, .. }
                    | ReferenceArgumentRelationV1::DisjointOutputCoordinate { argument, .. } => {
                        *argument == kernel_argument
                    }
                    ReferenceArgumentRelationV1::PointCoordinate { .. } => false,
                }) {
                Some(ReferenceArgumentRelationV1::ScalarInput { .. }) => {
                    Ok(ReferenceEffectExpressionV1::KernelScalarArgument {
                        argument: kernel_argument,
                    })
                }
                Some(_) => Err(ReferenceBindingErrorV1::new(format!(
                    "reference effect expression reads non-scalar logical argument {}",
                    kernel_argument + 1,
                ))),
                None => Err(ReferenceBindingErrorV1::new(format!(
                    "reference argument {} has no logical ABI relation",
                    reference_argument + 1,
                ))),
            };
        }
        if self.ambiguous_definitions.contains(&local) {
            return Err(ReferenceBindingErrorV1::new(format!(
                "reference effect local _{local} has multiple definitions; path-sensitive scalar phi normalization is outside reference-effect V1",
            )));
        }
        let value = self.definitions.get(&local).ok_or_else(|| {
            ReferenceBindingErrorV1::new(format!(
                "reference effect local _{local} has no unique scalar definition",
            ))
        })?;
        if !visiting.insert(local) {
            return Err(ReferenceBindingErrorV1::new(format!(
                "reference effect local _{local} has a cyclic scalar definition",
            )));
        }
        let resolved = self.resolve_value_inner_v1(meter, value, visiting, work, depth);
        visiting.remove(&local);
        resolved
    }

    pub(super) fn resolve_operand_inner_v1(
        &self,
        meter: &impl ReferenceWorkV1,
        operand: &ReferenceOperandV1,
        visiting: &mut BTreeSet<u32>,
        work: &mut usize,
        depth: usize,
    ) -> Result<ReferenceEffectExpressionV1, ReferenceBindingErrorV1> {
        Self::require_depth_v1(depth)?;
        meter.charge(meter.operand(operand)?)?;
        meter.tree::<(u32, &ReferenceValueV1)>(self.definitions.len())?;
        meter.product(self.effect_ir.relations.len(), 2)?;
        match operand {
            ReferenceOperandV1::Constant(constant) => {
                Self::charge_node_v1(meter, work)?;
                Ok(ReferenceEffectExpressionV1::Constant(constant.clone()))
            }
            ReferenceOperandV1::Copy(place) | ReferenceOperandV1::Move(place)
                if place.projection.is_empty() =>
            {
                self.resolve_local_inner_v1(meter, place.local, visiting, work, depth)
            }
            ReferenceOperandV1::Copy(place) | ReferenceOperandV1::Move(place)
                if matches!(
                    place.projection.as_ref(),
                    [ReferencePlaceProjectionV1::Field(0)]
                ) =>
            {
                let value = self.definitions.get(&place.local).ok_or_else(|| {
                    ReferenceBindingErrorV1::new(format!(
                        "reference checked scalar pair _{} has no unique definition",
                        place.local,
                    ))
                })?;
                match value {
                    ReferenceValueV1::Binary { checked: true, .. } => {
                        self.resolve_value_inner_v1(meter, value, visiting, work, depth + 1)
                    }
                    _ => Err(ReferenceBindingErrorV1::new(format!(
                        "reference field projection {:?} is not the value field of one checked scalar operation",
                        place.projection,
                    ))),
                }
            }
            ReferenceOperandV1::Copy(place) | ReferenceOperandV1::Move(place)
                if matches!(
                    place.projection.as_ref(),
                    [
                        ReferencePlaceProjectionV1::Dereference,
                        ReferencePlaceProjectionV1::Index(_)
                    ]
                ) =>
            {
                let [
                    ReferencePlaceProjectionV1::Dereference,
                    ReferencePlaceProjectionV1::Index(index),
                ] = place.projection.as_ref()
                else {
                    unreachable!()
                };
                let reference_argument = place.local.checked_sub(1).ok_or_else(|| {
                    ReferenceBindingErrorV1::new(
                        "safe reference load uses the return-place local as its base",
                    )
                })?;
                let point_count = self.effect_ir.point_coordinate_count_v1()?;
                let kernel_argument =
                    reference_argument.checked_sub(point_count).ok_or_else(|| {
                        ReferenceBindingErrorV1::new(
                            "safe reference load base has no logical kernel argument",
                        )
                    })?;
                if !self.effect_ir.relations.iter().any(|relation| {
                    matches!(
                        relation,
                        ReferenceArgumentRelationV1::SharedSliceInput { argument, .. }
                            if *argument == kernel_argument
                    )
                }) {
                    return Err(ReferenceBindingErrorV1::new(
                        "safe reference load base is not a shared-slice input",
                    ));
                }
                Self::charge_node_v1(meter, work)?;
                Ok(ReferenceEffectExpressionV1::InputLoad {
                    reference_argument,
                    index: Box::new(self.resolve_local_inner_v1(
                        meter,
                        *index,
                        visiting,
                        work,
                        depth + 1,
                    )?),
                })
            }
            ReferenceOperandV1::Copy(place) | ReferenceOperandV1::Move(place) => {
                Err(ReferenceBindingErrorV1::new(format!(
                    "reference effect scalar operand uses unsupported place projection {:?}",
                    place.projection,
                )))
            }
        }
    }

    pub(super) fn resolve_value_inner_v1(
        &self,
        meter: &impl ReferenceWorkV1,
        value: &ReferenceValueV1,
        visiting: &mut BTreeSet<u32>,
        work: &mut usize,
        depth: usize,
    ) -> Result<ReferenceEffectExpressionV1, ReferenceBindingErrorV1> {
        Self::require_depth_v1(depth)?;
        Self::charge_node_v1(meter, work)?;
        match value {
            ReferenceValueV1::Use(operand) => {
                self.resolve_operand_inner_v1(meter, operand, visiting, work, depth + 1)
            }
            ReferenceValueV1::Binary {
                operation,
                lhs,
                rhs,
                checked,
            } => Ok(ReferenceEffectExpressionV1::Binary {
                operation: *operation,
                lhs: Box::new(self.resolve_operand_inner_v1(
                    meter,
                    lhs,
                    visiting,
                    work,
                    depth + 1,
                )?),
                rhs: Box::new(self.resolve_operand_inner_v1(
                    meter,
                    rhs,
                    visiting,
                    work,
                    depth + 1,
                )?),
                checked: *checked,
            }),
            ReferenceValueV1::Unary { operation, operand } => {
                Ok(ReferenceEffectExpressionV1::Unary {
                    operation: *operation,
                    operand: Box::new(self.resolve_operand_inner_v1(
                        meter,
                        operand,
                        visiting,
                        work,
                        depth + 1,
                    )?),
                })
            }
            ReferenceValueV1::Cast {
                kind,
                source,
                target,
                operand,
            } => Ok(ReferenceEffectExpressionV1::Cast {
                kind: *kind,
                source: *source,
                target: *target,
                operand: Box::new(self.resolve_operand_inner_v1(
                    meter,
                    operand,
                    visiting,
                    work,
                    depth + 1,
                )?),
            }),
            ReferenceValueV1::InputLength { reference_argument } => {
                Ok(ReferenceEffectExpressionV1::InputLength {
                    reference_argument: *reference_argument,
                })
            }
            ReferenceValueV1::SafeHelperCall {
                parameters,
                arguments,
                summary,
                ..
            } => {
                if parameters.len() != arguments.len() {
                    return Err(ReferenceBindingErrorV1::new(
                        "authenticated safe helper summary argument count changed",
                    ));
                }
                meter.rows::<ReferenceEffectExpressionV1>(arguments.len())?;
                let arguments = arguments
                    .iter()
                    .map(|argument| {
                        self.resolve_operand_inner_v1(meter, argument, visiting, work, depth + 1)
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                substitute_helper_summary_v2(meter, summary, &arguments, work, depth + 1)
            }
        }
    }
}

pub fn substitute_helper_summary_v2(
    meter: &impl ReferenceWorkV1,
    expression: &ReferenceEffectExpressionV1,
    arguments: &[ReferenceEffectExpressionV1],
    work: &mut usize,
    depth: usize,
) -> Result<ReferenceEffectExpressionV1, ReferenceBindingErrorV1> {
    ReferenceExpressionResolverV1::require_depth_v1(depth)?;
    ReferenceExpressionResolverV1::charge_node_v1(meter, work)?;
    Ok(match expression {
        ReferenceEffectExpressionV1::KernelScalarArgument { argument } => {
            let argument = arguments.get(*argument as usize).ok_or_else(|| {
                ReferenceBindingErrorV1::new(format!(
                    "safe helper summary refers to missing argument {}",
                    argument + 1,
                ))
            })?;
            meter.clone_expression(argument)?;
            argument.clone()
        }
        ReferenceEffectExpressionV1::PointCoordinate { .. } => {
            return Err(ReferenceBindingErrorV1::new(
                "safe scalar helper summary unexpectedly contains a point-coordinate symbol",
            ));
        }
        ReferenceEffectExpressionV1::InputLoad { .. } => {
            return Err(ReferenceBindingErrorV1::new(
                "safe scalar helper summaries cannot capture reference loads",
            ));
        }
        ReferenceEffectExpressionV1::InputLength { .. } => {
            return Err(ReferenceBindingErrorV1::new(
                "safe scalar helper summaries cannot capture slice lengths",
            ));
        }
        ReferenceEffectExpressionV1::Constant(constant) => {
            ReferenceEffectExpressionV1::Constant(constant.clone())
        }
        ReferenceEffectExpressionV1::Binary {
            operation,
            lhs,
            rhs,
            checked,
        } => ReferenceEffectExpressionV1::Binary {
            operation: *operation,
            lhs: Box::new(substitute_helper_summary_v2(
                meter,
                lhs,
                arguments,
                work,
                depth + 1,
            )?),
            rhs: Box::new(substitute_helper_summary_v2(
                meter,
                rhs,
                arguments,
                work,
                depth + 1,
            )?),
            checked: *checked,
        },
        ReferenceEffectExpressionV1::Unary { operation, operand } => {
            ReferenceEffectExpressionV1::Unary {
                operation: *operation,
                operand: Box::new(substitute_helper_summary_v2(
                    meter,
                    operand,
                    arguments,
                    work,
                    depth + 1,
                )?),
            }
        }
        ReferenceEffectExpressionV1::Cast {
            kind,
            source,
            target,
            operand,
        } => ReferenceEffectExpressionV1::Cast {
            kind: *kind,
            source: *source,
            target: *target,
            operand: Box::new(substitute_helper_summary_v2(
                meter,
                operand,
                arguments,
                work,
                depth + 1,
            )?),
        },
    })
}
