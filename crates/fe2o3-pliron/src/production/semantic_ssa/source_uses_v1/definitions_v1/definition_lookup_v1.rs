impl<'a> ProductionSemanticSsaSourceQueryV1<'a> {
    /// Inert coordinates in this exact retained plan. A raw definition ID does
    /// not mint a source-use/value token or prove an issuer, read or lifetime.
    /// Consumers must bind this plan to their actual body and retain replay.
    pub fn definition_origin(
        &self,
        definition: fe2o3_mir_model::SsaDefinitionIdV1,
        charge: &mut impl FnMut() -> bool,
    ) -> Result<(SsaVariableIdV1, ProductionSemanticSsaValueOriginV1<'a>), QueryError> {
        step(charge)?;
        definition_origin(self.plan, definition, None, charge)
    }
}

impl ProductionSemanticSsaFunctionPlanV1 {
    pub(in super::super) fn source_event_site(
        &self,
        block: SsaBlockIdV1,
        event: u32,
        charge: &mut impl FnMut() -> bool,
    ) -> Result<Site, QueryError> {
        step(charge)?;
        let events = self
            .plan
            .resolved_events(block)
            .ok_or(QueryError::InvalidSite)?;
        let found = partition(events, |(at, _)| *at < event, charge)?;
        if events.get(found).is_none_or(|(at, _)| *at != event) {
            return Err(QueryError::MissingEventOrigin);
        }
        let ends = self
            .event_origins
            .block_ends(block.get())
            .ok_or(QueryError::MissingEventOrigin)?;
        let index = partition(ends, |end| *end <= event, charge)?;
        ends.get(index).ok_or(QueryError::MissingEventOrigin)?;
        let statement = if index + 1 == ends.len() {
            None
        } else {
            Some(u32::try_from(index).map_err(|_| QueryError::MissingEventOrigin)?)
        };
        Ok(Site::new(
            SemanticBlockIdV1::from_index(block.get()),
            statement,
        ))
    }
}

// The token-based public query keeps its original variable check and charged
// steps. The raw-ID entry returns only this existing inert origin enum.
fn definition_origin<'a>(
    plan: &'a ProductionSemanticSsaFunctionPlanV1,
    id: fe2o3_mir_model::SsaDefinitionIdV1,
    expected: Option<SsaVariableIdV1>,
    charge: &mut impl FnMut() -> bool,
) -> Result<(SsaVariableIdV1, ProductionSemanticSsaValueOriginV1<'a>), QueryError> {
    let origins = &plan.value_origins;
    let matches = |variable, value| {
        value == SsaValueV1::Definition(id) && expected.is_none_or(|expected| variable == expected)
    };
    match origins
        .definitions
        .get(id.get() as usize)
        .ok_or(QueryError::MissingDefinition)?
        .origin()
    {
        DefinitionOrigin::Entry { argument } => {
            let row = plan
                .plan
                .entry_definitions()
                .get(argument)
                .filter(|row| matches(row.variable(), row.value()))
                .ok_or(QueryError::MissingDefinition)?;
            Ok((
                row.variable(),
                ProductionSemanticSsaValueOriginV1::Entry { argument },
            ))
        }
        DefinitionOrigin::Event { block, event } => {
            let events = plan
                .plan
                .resolved_events(block)
                .ok_or(QueryError::MissingDefinition)?;
            let position = partition(events, |(at, _)| *at < event, charge)?;
            let Some((at, SsaResolvedEventV1::Define { variable, value })) = events.get(position)
            else {
                return Err(QueryError::MissingDefinition);
            };
            if *at != event || !matches(*variable, *value) {
                return Err(QueryError::MissingDefinition);
            }
            let site = plan.source_event_site(block, event, charge)?;
            Ok((
                *variable,
                ProductionSemanticSsaValueOriginV1::Event { block, event, site },
            ))
        }
        DefinitionOrigin::Edge {
            incoming,
            definition,
        } => {
            let row = *origins
                .incoming
                .get(incoming)
                .ok_or(QueryError::MissingDefinition)?;
            let argument = plan
                .plan
                .edge_definitions(row.edge())
                .and_then(|arguments| arguments.get(definition))
                .filter(|argument| matches(argument.variable(), argument.value()))
                .ok_or(QueryError::MissingDefinition)?;
            Ok((
                argument.variable(),
                ProductionSemanticSsaValueOriginV1::Edge {
                    edge: ProductionSemanticSsaIncomingEdgeV1 { row },
                    definition,
                },
            ))
        }
        _ => Err(QueryError::MissingDefinition),
    }
}
