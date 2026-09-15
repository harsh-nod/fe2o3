// Exclude an enum alternative only after the original tag edge proves the
// requested variant of this exact SSA value at the original projection use.
use super::*;
use fe2o3_mir_model::SsaEdgeIdV1;

impl Resolver<'_, '_> {
    pub(super) fn guarded_payload(
        &mut self,
        value: SsaValueV1,
        enum_type: SemanticTypeIdV1,
        variant: u32,
        field: u32,
        remaining: &[SemanticProjectionKindV1],
        use_block: u32,
    ) -> Result<Option<SsaValueV1>, ProductionSemanticKirErrorV1> {
        if !self.graph.selected_variant(self.types, value, enum_type, variant, use_block)? {
            return Ok(None);
        }
        self.result_value(value, enum_type, variant, field, remaining)?
            .map(Some)
            .ok_or_else(|| reject("global BF16 guarded Result has no matching payload origin"))
    }

    fn result_value(
        &mut self,
        value: SsaValueV1,
        enum_type: SemanticTypeIdV1,
        variant: u32,
        field: u32,
        remaining: &[SemanticProjectionKindV1],
    ) -> Result<Option<SsaValueV1>, ProductionSemanticKirErrorV1> {
        self.graph.charge(1)?;
        if self.active.len() >= 128 || !self.active.insert(value) {
            return Err(reject(
                "global BF16 capture has cyclic or excessive SSA custody",
            ));
        }
        let result = self.result_definition(value, enum_type, variant, field, remaining);
        self.active.remove(&value);
        result
    }

    fn result_definition(
        &mut self,
        value: SsaValueV1,
        enum_type: SemanticTypeIdV1,
        variant: u32,
        field: u32,
        remaining: &[SemanticProjectionKindV1],
    ) -> Result<Option<SsaValueV1>, ProductionSemanticKirErrorV1> {
        if let SsaValueV1::BlockArgument { block, variable } = value {
            // Stream the same complete reachable predecessor/edge roster as
            // Graph::incoming, without a retained allocation per nested Phi.
            let body = self.graph.body;
            let ssa = self.graph.ssa;
            let mut found_edge = false;
            let mut issuer = None;
            for &predecessor in ssa.reverse_postorder() {
                self.graph.charge(1)?;
                let mut ordinal = 0u32;
                body.blocks()[predecessor.get() as usize]
                    .terminator()
                    .kind()
                    .try_for_each_edge::<ProductionSemanticKirErrorV1>(|edge| {
                        self.graph.charge(1)?;
                        let edge_id = SsaEdgeIdV1::new(predecessor, ordinal);
                        ordinal = ordinal
                            .checked_add(1)
                            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
                        if edge.target().index() != block.get() {
                            return Ok(());
                        }
                        found_edge = true;
                        let arguments = ssa
                            .edge_arguments(edge_id)
                            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
                        self.graph.charge(arguments.len())?;
                        let mut incoming = None;
                        for argument in arguments {
                            if argument.variable() == variable
                                && incoming.replace(argument.value()).is_some()
                            {
                                return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
                            }
                        }
                        let incoming = incoming.ok_or_else(|| {
                            reject("capability SSA merge lacks its exact edge value")
                        })?;
                        if let Some(candidate) =
                            self.result_value(incoming, enum_type, variant, field, remaining)?
                            && issuer
                                .replace(candidate)
                                .is_some_and(|previous| previous != candidate)
                        {
                            return Err(reject(
                                "global BF16 capture merges different Global issuers",
                            ));
                        }
                        Ok(())
                    })?;
            }
            if !found_edge {
                return Err(reject("capability SSA merge has no incoming owner"));
            }
            return Ok(issuer);
        }
        let site = self.graph.definition(value)?;
        let body = self.graph.body;
        let statement = site
            .statement
            .ok_or_else(|| reject("global BF16 guarded Result is not a retained assignment"))?;
        let SemanticStatementKindV1::Assign(assignment) =
            body.blocks()[site.block as usize].statements()[statement as usize].kind()
        else {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        };
        if assignment.destination().ty() != enum_type
            || assignment.value().result_type() != enum_type
        {
            return Err(reject(
                "global BF16 guarded Result changes its exact enum type",
            ));
        }
        match assignment.value().kind() {
            SemanticRvalueKindV1::Use(
                SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place),
            ) if place.ty() == enum_type && place.projections().is_empty() => {
                let input = self.graph.use_value(site.block, place.local().index())?;
                self.result_value(input, enum_type, variant, field, remaining)
            }
            SemanticRvalueKindV1::Aggregate(aggregate) => {
                let SemanticAggregateKindV1::EnumVariant(actual) = aggregate.kind() else {
                    return Err(reject(
                        "global BF16 guarded Result is not an enum construction",
                    ));
                };
                let Some(SemanticTypeShapeV1::Enum { variants, .. }) = self
                    .types
                    .get(enum_type.index() as usize)
                    .map(SemanticTypeDeclV1::shape)
                else {
                    return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
                };
                let actual_variant = variants
                    .get(*actual as usize)
                    .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
                if actual_variant.is_uninhabited() {
                    return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
                }
                let fields = actual_variant.fields().fields();
                self.graph.charge(fields.len())?;
                if fields.len() != aggregate.operands().len()
                    || fields
                        .iter()
                        .zip(aggregate.operands())
                        .any(|(ty, operand)| *ty != operand.ty())
                {
                    return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
                }
                if *actual != variant {
                    // This whole enum value contributes to the guarded SSA join.
                    // The original selecting edge excludes only this variant.
                    return Ok(None);
                }
                let operand = aggregate
                    .operands()
                    .get(field as usize)
                    .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
                self.operand(site.block, operand, remaining).map(Some)
            }
            _ => Err(reject(
                "global BF16 guarded Result has unsupported payload transport",
            )),
        }
    }
}
