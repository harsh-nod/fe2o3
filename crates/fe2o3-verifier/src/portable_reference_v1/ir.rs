//! Existing acyclic output derivation and canonical IR digest.

use super::*;

impl ReferenceEffectIrV1 {
    pub fn resolved_bounds_checks_v1(
        &self,
    ) -> Result<Vec<ResolvedReferenceBoundsCheckV1>, ReferenceBindingErrorV1> {
        let meter = &InspectionWorkV1;
        let resolver = ReferenceExpressionResolverV1::new(meter, self)?;
        let mut checks = Vec::new();
        for block in &self.blocks {
            let ReferenceTerminatorV1::Assert {
                condition,
                expected,
                bounds_check: Some(bounds_check),
                ..
            } = &block.terminator
            else {
                continue;
            };
            checks.push(ResolvedReferenceBoundsCheckV1 {
                block: block.block,
                expected: *expected,
                condition: resolver.resolve_operand_inner_v1(
                    meter,
                    condition,
                    &mut BTreeSet::new(),
                    &mut 0,
                    1,
                )?,
                index: resolver.resolve_operand_inner_v1(
                    meter,
                    &bounds_check.index,
                    &mut BTreeSet::new(),
                    &mut 0,
                    1,
                )?,
                length: resolver.resolve_operand_inner_v1(
                    meter,
                    &bounds_check.length,
                    &mut BTreeSet::new(),
                    &mut 0,
                    1,
                )?,
            });
        }
        Ok(checks)
    }

    pub fn observable_output_writes_v1(
        &self,
        meter: &impl ReferenceWorkV1,
    ) -> Result<Vec<ReferenceOutputWriteV1>, ReferenceBindingErrorV1> {
        meter.rows::<ReferenceEffectExpressionV1>(self.relations.len())?;
        let guards = reference_block_path_predicates_v1(meter, self)?;
        let resolver = ReferenceExpressionResolverV1::new(meter, self)?;
        let point_coordinates = self
            .relations
            .iter()
            .filter_map(|relation| match relation {
                ReferenceArgumentRelationV1::PointCoordinate { axis, .. } => {
                    Some(ReferenceEffectExpressionV1::PointCoordinate { axis: *axis })
                }
                _ => None,
            })
            .collect::<Vec<_>>()
            .into_boxed_slice();
        let mut writes = Vec::new();
        for relation in &self.relations {
            meter.charge(1)?;
            let (argument, coordinate_output) = match relation {
                ReferenceArgumentRelationV1::DisjointOutputSlice { argument, .. } => {
                    (*argument, false)
                }
                ReferenceArgumentRelationV1::DisjointOutputCoordinate { argument, .. } => {
                    (*argument, true)
                }
                ReferenceArgumentRelationV1::ScalarInput { .. }
                | ReferenceArgumentRelationV1::SharedSliceInput { .. }
                | ReferenceArgumentRelationV1::PointCoordinate { .. } => continue,
            };
            meter.charge(self.relations.len())?;
            let local = self
                .reference_argument_for_kernel_argument_v1(argument)?
                .checked_add(1)
                .ok_or_else(|| ReferenceBindingErrorV1::new("reference local index overflowed"))?;
            for block in &self.blocks {
                meter.charge(1)?;
                let guard = guards.get(block.block as usize).ok_or_else(|| {
                    ReferenceBindingErrorV1::new("reference block identity is out of bounds")
                })?;
                if guard.is_unreachable_v1() {
                    continue;
                }
                for assignment in &block.assignments {
                    meter.charge(1)?;
                    if assignment.destination.local != local {
                        continue;
                    }
                    let projection = assignment.destination.projection.as_ref();
                    let coordinate = match projection {
                        [ReferencePlaceProjectionV1::Dereference] if coordinate_output => {
                            if point_coordinates.is_empty() {
                                ReferenceOutputCoordinateV1::SingleCoordinate
                            } else {
                                meter
                                    .rows::<ReferenceEffectExpressionV1>(point_coordinates.len())?;
                                ReferenceOutputCoordinateV1::LogicalPoint(point_coordinates.clone())
                            }
                        }
                        [
                            ReferencePlaceProjectionV1::Dereference,
                            ReferencePlaceProjectionV1::Index(index),
                        ] if !coordinate_output => ReferenceOutputCoordinateV1::Dynamic(
                            resolver.resolve_local_v1(meter, *index)?,
                        ),
                        [
                            ReferencePlaceProjectionV1::Dereference,
                            ReferencePlaceProjectionV1::ConstantIndex {
                                offset,
                                minimum_length,
                                from_end,
                            },
                        ] if !coordinate_output => ReferenceOutputCoordinateV1::Constant {
                            offset: *offset,
                            minimum_length: *minimum_length,
                            from_end: *from_end,
                        },
                        _ => {
                            return Err(ReferenceBindingErrorV1::new(format!(
                                "observable output argument {} uses unsupported write projection {:?}; reference-effect V1 cannot omit a global output write",
                                argument + 1,
                                assignment.destination.projection,
                            )));
                        }
                    };
                    meter.clone_predicate(guard)?;
                    meter.charge(meter.value(&assignment.value)?)?;
                    meter.grow::<ReferenceOutputWriteV1>(writes.len())?;
                    writes.push(ReferenceOutputWriteV1 {
                        argument,
                        block: block.block,
                        statement: assignment.statement,
                        coordinate,
                        guard: guard.clone(),
                        rhs: resolver.resolve_value_v1(meter, &assignment.value)?,
                        value: assignment.value.clone(),
                    });
                }
            }
        }
        Ok(writes)
    }

    pub fn point_coordinate_count_v1(&self) -> Result<u32, ReferenceBindingErrorV1> {
        u32::try_from(
            self.relations
                .iter()
                .filter(|relation| {
                    matches!(
                        relation,
                        ReferenceArgumentRelationV1::PointCoordinate { .. }
                    )
                })
                .count(),
        )
        .map_err(|_| ReferenceBindingErrorV1::new("point coordinate count exceeds u32"))
    }

    pub fn reference_argument_for_kernel_argument_v1(
        &self,
        kernel_argument: u32,
    ) -> Result<u32, ReferenceBindingErrorV1> {
        self.point_coordinate_count_v1()?
            .checked_add(kernel_argument)
            .ok_or_else(|| ReferenceBindingErrorV1::new("reference argument index overflowed"))
    }

    pub fn canonical_sha256_v1(&self) -> [u8; 32] {
        let mut digest = Sha256::new();
        digest.update(b"fe2o3/reference-effect-ir/v1\0");
        digest.update(self.argument_count.to_le_bytes());
        digest.update(self.local_count.to_le_bytes());
        put_len(&mut digest, self.relations.len());
        for relation in &self.relations {
            match relation {
                ReferenceArgumentRelationV1::PointCoordinate {
                    reference_argument,
                    axis,
                } => {
                    digest.update([4]);
                    digest.update(reference_argument.to_le_bytes());
                    digest.update(axis.to_le_bytes());
                }
                ReferenceArgumentRelationV1::ScalarInput { argument, scalar } => {
                    digest.update([0, scalar_tag(*scalar)]);
                    digest.update(argument.to_le_bytes());
                }
                ReferenceArgumentRelationV1::SharedSliceInput { argument, element } => {
                    digest.update([1, scalar_tag(*element)]);
                    digest.update(argument.to_le_bytes());
                }
                ReferenceArgumentRelationV1::DisjointOutputSlice { argument, element } => {
                    digest.update([2, scalar_tag(*element)]);
                    digest.update(argument.to_le_bytes());
                }
                ReferenceArgumentRelationV1::DisjointOutputCoordinate { argument, element } => {
                    digest.update([3, scalar_tag(*element)]);
                    digest.update(argument.to_le_bytes());
                }
            }
        }
        put_len(&mut digest, self.blocks.len());
        for block in &self.blocks {
            digest.update(block.block.to_le_bytes());
            put_len(&mut digest, block.assignments.len());
            for assignment in &block.assignments {
                digest.update(assignment.statement.to_le_bytes());
                digest_place(&mut digest, &assignment.destination);
                digest_value(&mut digest, &assignment.value);
            }
            digest_terminator(&mut digest, &block.terminator);
        }
        put_len(&mut digest, self.loop_summaries.len());
        for summary in &self.loop_summaries {
            digest.update(summary.header.to_le_bytes());
            digest.update(summary.latch.to_le_bytes());
            digest.update(summary.exit.to_le_bytes());
            match summary.exact_iterations {
                Some(iterations) => {
                    digest.update([1]);
                    digest.update(iterations.to_le_bytes());
                }
                None => digest.update([0]),
            }
            digest.update(summary.maximum_iterations.to_le_bytes());
            put_len(&mut digest, summary.carried_locals.len());
            for local in &summary.carried_locals {
                digest.update(local.to_le_bytes());
            }
            digest.update(summary.initial_state_sha256);
            digest.update(summary.transition_sha256);
            digest.update(summary.variant_sha256);
        }
        put_len(&mut digest, self.observable_output_effects.len());
        for effect in &self.observable_output_effects {
            digest_output_effect_v1(&mut digest, effect);
        }
        digest.finalize().into()
    }
}
