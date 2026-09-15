use super::*;

impl TotalUnsignedIndexProjectorV1<'_, '_, '_> {
    pub(super) fn resolve_joined_enum_field_v1(
        &mut self,
        local: usize,
        field: Field,
        use_site: ScalarAssignmentSiteV1,
        projected: ProjectedUse,
    ) -> Result<Option<TotalUnsignedIndexValueV1>, ProductionRankedProjectionErrorV1> {
        let Some(conditions) = self.exclusive_enum_conditions else {
            return Ok(None);
        };
        let Some(SemanticTypeShapeV1::Enum { variants, .. }) = self
            .types
            .get(field.carrier_type.index() as usize)
            .map(|ty| ty.shape())
        else {
            return Ok(None);
        };
        self.assertion_proofs.charge(
            variants.len()
                + 8
                + std::mem::size_of::<[Option<ScalarAssignmentSiteV1>; 2]>()
                    .div_ceil(std::mem::size_of::<usize>()),
        )?;
        if !conditions.allows(
            self.types,
            self.function,
            SemanticLocalIdV1::from_index(projected.local as u32),
            field.variant,
            projected.site.block,
        ) || !self.enum_join_guard_v1(field, projected)?
        {
            return Ok(None);
        }
        let Some(definitions) = self.enum_join_definitions_v1(local)? else {
            return Ok(None);
        };
        let function = self.function;
        let mut selected = None;
        let mut previous_variant = None;
        for definition in definitions {
            let SemanticStatementKindV1::Assign(assignment) =
                function.blocks()[definition.block].statements()[definition.statement].kind()
            else {
                return Ok(None);
            };
            let SemanticRvalueKindV1::Aggregate(aggregate) = assignment.value().kind() else {
                return Ok(None);
            };
            let SemanticAggregateKindV1::EnumVariant(actual) = aggregate.kind() else {
                return Ok(None);
            };
            if !assignment.destination().projections().is_empty()
                || assignment.destination().ty() != field.carrier_type
                || assignment.value().result_type() != field.carrier_type
                || previous_variant == Some(*actual)
            {
                return Ok(None);
            }
            previous_variant = Some(*actual);
            let Some(variant) = variants.get(*actual as usize) else {
                return Ok(None);
            };
            let fields = variant.fields().fields();
            self.assertion_proofs.charge(4 + fields.len())?;
            if variant.is_uninhabited()
                || fields.len() != aggregate.operands().len()
                || fields
                    .iter()
                    .zip(aggregate.operands())
                    .any(|(ty, operand)| *ty != operand.ty())
            {
                return Ok(None);
            }
            if *actual == field.variant {
                if fields.get(field.field as usize) != Some(&field.scalar_type) {
                    return Ok(None);
                }
                selected = Some((definition, &aggregate.operands()[field.field as usize]));
            }
        }
        let Some((selected_site, payload)) = selected else {
            return Ok(None);
        };
        let other = definitions[usize::from(definitions[0].block == selected_site.block)].block;
        let proof = &mut self.assertion_proofs;
        if !proof.graph.query(
            CsrWorkV1::new(&mut proof.work, MAX_PROJECTED_LOOP_GRAPH_WORK_V1),
            |mut query| {
                Ok(query.enum_definitions_cover_use(definitions, use_site)?
                    && query.enum_constructor_is_fresh(
                        selected_site.block,
                        other,
                        projected.site.block,
                    )?)
            },
        )? || !self.enum_join_captures_v1(local, selected_site, projected)?
        {
            return Ok(None);
        }
        self.states
            .insert(local, TotalUnsignedIndexStateV1::Visiting);
        // The other incoming value is an explicit different variant, not an
        // unknown value or another owner of the same payload. Keep arithmetic
        // and invocation provenance at this original constructor operand.
        let result = self.resolve_operand(payload, selected_site.block, selected_site.statement);
        self.states.remove(&local);
        result
    }

    fn enum_join_definitions_v1(
        &mut self,
        local: usize,
    ) -> Result<Option<[ScalarAssignmentSiteV1; 2]>, ProductionRankedProjectionErrorV1> {
        let mut sites = [None; 2];
        let mut count = 0;
        let function = self.function;
        for block in 0..function.blocks().len() {
            let Some(row) = self.assertion_proofs.block_definitions.get(block) else {
                return Ok(None);
            };
            // Search the existing sorted per-block definition index. Only its
            // matching source blocks need statement reconciliation.
            let searches = usize::BITS as usize - row.len().leading_zeros() as usize;
            self.assertion_proofs.charge(2 + searches)?;
            if self.assertion_proofs.block_definitions[block]
                .binary_search(&local)
                .is_err()
            {
                continue;
            }
            for (statement, source) in function.blocks()[block].statements().iter().enumerate() {
                self.assertion_proofs.charge(1)?;
                let mut definitions = 0;
                let mut visits = 0;
                visit_statement_definition_places(source.kind(), &mut |place| {
                    visits += 1;
                    definitions += usize::from(local_definition_index(place) == Some(local));
                });
                self.assertion_proofs.charge(visits)?;
                if definitions != 0 {
                    if definitions != 1 || count == sites.len() {
                        return Ok(None);
                    }
                    sites[count] = Some(ScalarAssignmentSiteV1 { block, statement });
                    count += 1;
                }
            }
            self.assertion_proofs.charge(1)?;
            if let SemanticTerminatorKindV1::Call(call) =
                function.blocks()[block].terminator().kind()
            {
                if call
                    .destination()
                    .and_then(|destination| local_definition_index(destination.place()))
                    == Some(local)
                {
                    return Ok(None);
                }
            }
        }
        Ok(match sites {
            [Some(left), Some(right)] => Some([left, right]),
            _ => None,
        })
    }

    fn enum_join_guard_v1(
        &mut self,
        field: Field,
        projected: ProjectedUse,
    ) -> Result<bool, ProductionRankedProjectionErrorV1> {
        let Some(capture) = self
            .assertion_proofs
            .assignments
            .get(projected.local)
            .copied()
            .flatten()
        else {
            return Ok(false);
        };
        let SemanticTypeShapeV1::Enum {
            discriminant: discriminant_type,
            variants,
        } = self.types[field.carrier_type.index() as usize].shape()
        else {
            return Ok(false);
        };
        let Some(variant) = variants.get(field.variant as usize) else {
            return Ok(false);
        };
        let mut guard = None;
        // Inspect terminators once, then use the existing unique-definition
        // index for their scalar discriminator. No statement-body census.
        for (block, source) in self.function.blocks().iter().enumerate() {
            self.assertion_proofs.charge(1)?;
            // This theorem uses the existing normal-edge CSR. A cleanup edge
            // may introduce an additional incoming value absent from that graph.
            let unwind = match source.terminator().kind() {
                SemanticTerminatorKindV1::Call(call) => Some(call.unwind()),
                SemanticTerminatorKindV1::TailCall(call) => Some(call.unwind()),
                SemanticTerminatorKindV1::Assert { unwind, .. }
                | SemanticTerminatorKindV1::Drop { unwind, .. } => Some(*unwind),
                SemanticTerminatorKindV1::Goto(_)
                | SemanticTerminatorKindV1::SwitchInt { .. }
                | SemanticTerminatorKindV1::FalseEdge { .. }
                | SemanticTerminatorKindV1::Return
                | SemanticTerminatorKindV1::UnwindResume
                | SemanticTerminatorKindV1::UnwindTerminate
                | SemanticTerminatorKindV1::Abort
                | SemanticTerminatorKindV1::Unreachable => None,
            };
            if matches!(unwind, Some(SemanticUnwindActionV1::Cleanup(_))) {
                return Ok(false);
            }
            let SemanticTerminatorKindV1::SwitchInt {
                discriminant,
                targets,
            } = source.terminator().kind()
            else {
                continue;
            };
            self.assertion_proofs.charge(8)?;
            let Some(place) = raw_operand_place(discriminant) else {
                continue;
            };
            let local = place.local().index() as usize;
            if !place.projections().is_empty()
                || place.ty() != *discriminant_type
                || self.assertion_proofs.address_escaped.get(local).copied() != Some(false)
            {
                continue;
            }
            let Some(site) = self
                .assertion_proofs
                .assignments
                .get(local)
                .copied()
                .flatten()
            else {
                continue;
            };
            if site.block != block {
                continue;
            }
            let SemanticStatementKindV1::Assign(assignment) =
                source.statements()[site.statement].kind()
            else {
                continue;
            };
            let SemanticRvalueKindV1::Discriminant(place) = assignment.value().kind() else {
                continue;
            };
            if !place.projections().is_empty()
                || place.local().index() as usize != projected.local
                || place.ty() != field.carrier_type
                || assignment.destination().ty() != *discriminant_type
                || assignment.value().result_type() != *discriminant_type
            {
                continue;
            }
            self.assertion_proofs.charge(1 + targets.values().len())?;
            let target = targets
                .values()
                .iter()
                .find(|target| target.value() == variant.discriminant())
                .map(|target| target.edge().target())
                .unwrap_or_else(|| targets.otherwise().target());
            if guard.replace((site, target.index() as usize)).is_some() {
                return Ok(false);
            }
        }
        let Some((discriminator, selected)) = guard else {
            return Ok(false);
        };
        if !self.assertion_proofs.assignment_dominates_use(
            capture,
            discriminator.block,
            discriminator.statement,
        )? {
            return Ok(false);
        }
        let proof = &mut self.assertion_proofs;
        proof.graph.query(
            CsrWorkV1::new(&mut proof.work, MAX_PROJECTED_LOOP_GRAPH_WORK_V1),
            |mut query| {
                Ok(query.guard_authenticates_each_use(
                    (discriminator.block, selected),
                    projected.site.block,
                )? && query.stable_edge_to_site(projected.local, selected, projected.site)?)
            },
        )
    }

    fn enum_join_captures_v1(
        &mut self,
        joined: usize,
        constructor: ScalarAssignmentSiteV1,
        projected: ProjectedUse,
    ) -> Result<bool, ProductionRankedProjectionErrorV1> {
        let mut local = projected.local;
        // Replay only the already checked whole-alias chain. No allocation,
        // new source use, or unindexed historical definition search is needed.
        for _ in 0..MAX_PURE_UNIFORM_INDEX_NODES_V1 {
            self.assertion_proofs.charge(4)?;
            if local == joined {
                return Ok(true);
            }
            let Some(site) = self
                .assertion_proofs
                .assignments
                .get(local)
                .copied()
                .flatten()
            else {
                return Ok(false);
            };
            let proof = &mut self.assertion_proofs;
            if !proof.graph.query(
                CsrWorkV1::new(&mut proof.work, MAX_PROJECTED_LOOP_GRAPH_WORK_V1),
                |mut query| {
                    query.enum_capture_is_fresh(constructor.block, site.block, projected.site.block)
                },
            )? {
                return Ok(false);
            }
            let SemanticStatementKindV1::Assign(assignment) =
                self.function.blocks()[site.block].statements()[site.statement].kind()
            else {
                return Ok(false);
            };
            let SemanticRvalueKindV1::Use(operand) = assignment.value().kind() else {
                return Ok(false);
            };
            let Some(place) = raw_operand_place(operand) else {
                return Ok(false);
            };
            local = place.local().index() as usize;
        }
        Ok(false)
    }
}
