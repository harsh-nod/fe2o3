//! Exact scalar payloads of dominating enum constructions. This does not turn
//! an enum discriminant or a type-compatible join into an invocation origin.
use super::*;

mod join_v1;

#[derive(Clone, Copy)]
struct Field {
    carrier_type: SemanticTypeIdV1,
    variant: u32,
    field: u32,
    scalar_type: SemanticTypeIdV1,
}

#[derive(Clone, Copy)]
struct ProjectedUse {
    local: usize,
    site: ScalarAssignmentSiteV1,
}

impl<'model, 'state, 'proof> TotalUnsignedIndexProjectorV1<'model, 'state, 'proof> {
    pub(super) fn with_exclusive_enum_conditions_v1(
        mut self,
        conditions: Option<&'state global_enum_transport_v1::Conditions<'model>>,
    ) -> Result<Self, ProductionRankedProjectionErrorV1> {
        if self.node_work != 0
            || !self.states.is_empty()
            || conditions.is_some_and(|conditions| !conditions.matches(self.types, self.function))
        {
            return Err(ProductionRankedProjectionErrorV1::Unsupported(
                "exclusive enum conditions do not belong to this source query",
            ));
        }
        self.exclusive_enum_conditions = conditions;
        Ok(self)
    }
}

impl TotalUnsignedIndexProjectorV1<'_, '_, '_> {
    pub(super) fn resolve_exclusive_enum_index_v1(
        &mut self,
        operand: &SemanticOperandV1,
        block: usize,
        statement: usize,
    ) -> Result<Option<TotalUnsignedIndexValueV1>, ProductionRankedProjectionErrorV1> {
        // Keep optional comparison/uniform-bound precision unchanged. This
        // source relation is consumed only by the mandatory exclusive query.
        if !self.exclusive_source_arguments
            || self.optional_source.is_some()
            || !matches!(self.roots, TotalUnsignedIndexRootsV1::Invocation { .. })
        {
            return Ok(None);
        }
        let Some(place) = raw_operand_place(operand) else {
            return Ok(None);
        };
        let [downcast, field] = place.projections() else {
            return Ok(None);
        };
        let (
            SemanticProjectionKindV1::Downcast(variant),
            SemanticProjectionKindV1::Field(field_id),
        ) = (downcast.kind(), field.kind())
        else {
            return Ok(None);
        };
        self.assertion_proofs.charge(8)?;
        if !self.exact_enum_index_use_v1(operand, block, statement)? {
            return Ok(None);
        }
        let local = place.local().index() as usize;
        let Some(declaration) = self.function.locals().get(local) else {
            return Ok(None);
        };
        if downcast.result_type() != declaration.ty()
            || field.result_type() != place.ty()
            || self.unsigned_maximum(place.ty()).is_none()
        {
            return Ok(None);
        }
        self.resolve_exact_enum_field_v1(
            local,
            Field {
                carrier_type: declaration.ty(),
                variant,
                field: field_id,
                scalar_type: place.ty(),
            },
            ScalarAssignmentSiteV1 { block, statement },
            ProjectedUse {
                local,
                site: ScalarAssignmentSiteV1 { block, statement },
            },
        )
    }

    fn exact_enum_index_use_v1(
        &mut self,
        operand: &SemanticOperandV1,
        block: usize,
        statement: usize,
    ) -> Result<bool, ProductionRankedProjectionErrorV1> {
        let function = self.function;
        let Some(block) = function.blocks().get(block) else {
            return Ok(false);
        };
        let mut found = false;
        if let Some(statement) = block.statements().get(statement) {
            if let SemanticStatementKindV1::Assign(assignment) = statement.kind() {
                assignment
                    .value()
                    .kind()
                    .try_visit_operands::<ProductionRankedProjectionErrorV1>(|source| {
                        self.assertion_proofs.charge(1)?;
                        found |= source == operand;
                        Ok(())
                    })?;
            }
        } else if statement == block.statements().len() {
            if let SemanticTerminatorKindV1::Call(call) = block.terminator().kind() {
                for source in call.arguments() {
                    self.assertion_proofs.charge(1)?;
                    found |= source == operand;
                }
            }
        }
        Ok(found)
    }

    fn resolve_exact_enum_field_v1(
        &mut self,
        local: usize,
        field: Field,
        use_site: ScalarAssignmentSiteV1,
        projected: ProjectedUse,
    ) -> Result<Option<TotalUnsignedIndexValueV1>, ProductionRankedProjectionErrorV1> {
        if !self.charge_node()? {
            return Ok(None);
        }
        // No new heap owner: the existing node-bounded states table holds only
        // an in-progress marker. Charge this bounded continuation's coordinates.
        self.assertion_proofs.charge(
            12 + std::mem::size_of::<(usize, Field, ScalarAssignmentSiteV1, ProjectedUse)>()
                .div_ceil(std::mem::size_of::<usize>()),
        )?;
        let function = self.function;
        let Some(declaration) = function.locals().get(local) else {
            return Ok(None);
        };
        if declaration.ty() != field.carrier_type
            || self.address_escaped().get(local).copied() != Some(false)
            || self.assertion_proofs.address_escaped.get(local).copied() != Some(false)
            || self.states.contains_key(&local)
        {
            return Ok(None);
        }
        let count = self.assertion_proofs.definition_counts.get(local).copied();
        if self.local_definitions.get(local).copied() != count {
            return Ok(None);
        }
        if count == Some(2) {
            return self.resolve_joined_enum_field_v1(local, field, use_site, projected);
        }
        if count != Some(1) {
            return Ok(None);
        }
        let Some(definition) = self.definitions().get(local).copied().flatten() else {
            return Ok(None);
        };
        if !self
            .assertion_proofs
            .assignments
            .get(local)
            .copied()
            .flatten()
            .is_some_and(|site| {
                site.block == definition.block && site.statement == definition.statement
            })
            || function
                .blocks()
                .get(use_site.block)
                .is_none_or(|block| use_site.statement > block.statements().len())
            || (definition.block == use_site.block && definition.statement >= use_site.statement)
            || !self
                .assertion_proofs
                .block_dominates(definition.block, use_site.block)?
        {
            return Ok(None);
        }
        let Some(SemanticStatementKindV1::Assign(assignment)) = function
            .blocks()
            .get(definition.block)
            .and_then(|block| block.statements().get(definition.statement))
            .map(|statement| statement.kind())
        else {
            return Ok(None);
        };
        if !assignment.destination().projections().is_empty()
            || assignment.destination().local().index() as usize != local
            || assignment.destination().ty() != field.carrier_type
            || assignment.value().result_type() != field.carrier_type
        {
            return Ok(None);
        }
        self.states
            .insert(local, TotalUnsignedIndexStateV1::Visiting);
        let result = (|| {
            match assignment.value().kind() {
                SemanticRvalueKindV1::Use(operand) => {
                    let Some(source) = raw_operand_place(operand) else {
                        return Ok(None);
                    };
                    if !source.projections().is_empty() || source.ty() != field.carrier_type {
                        return Ok(None);
                    }
                    // Move/Copy remain original source uses at this assignment,
                    // not manufactured loads or new mutable-reference authority.
                    self.resolve_exact_enum_field_v1(
                        source.local().index() as usize,
                        field,
                        definition,
                        projected,
                    )
                }
                SemanticRvalueKindV1::Aggregate(aggregate) => {
                    let SemanticAggregateKindV1::EnumVariant(actual) = aggregate.kind() else {
                        return Ok(None);
                    };
                    if *actual != field.variant {
                        return Ok(None);
                    }
                    let Some(SemanticTypeShapeV1::Enum { variants, .. }) = self
                        .types
                        .get(field.carrier_type.index() as usize)
                        .map(|ty| ty.shape())
                    else {
                        return Ok(None);
                    };
                    let Some(variant) = variants.get(field.variant as usize) else {
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
                        || fields.get(field.field as usize) != Some(&field.scalar_type)
                    {
                        return Ok(None);
                    }
                    let Some(payload) = aggregate.operands().get(field.field as usize) else {
                        return Ok(None);
                    };
                    // The constructor fixes the active variant and dominates
                    // every forwarding use. Arithmetic is still proved at the
                    // exact original payload use, including overflow evidence.
                    self.resolve_operand(payload, definition.block, definition.statement)
                }
                _ => Ok(None),
            }
        })();
        // Never memoize a field result by carrier local: another field, variant
        // or use site must authenticate its own complete source chain again.
        self.states.remove(&local);
        result
    }
}
