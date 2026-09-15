impl<'a> CapabilitySsaGraphV1<'a> {
    pub(super) fn with_definition_source(
        mut self,
        query: &fe2o3_pliron::ProductionSemanticSsaSourceQueryV1<'a>,
    ) -> Result<Self, ProductionSemanticKirErrorV1> {
        self.charge(3)?;
        if !std::ptr::eq(self.body, query.function())
            || !std::ptr::eq(self.ssa, query.plan().plan())
            || self.reuse.definition_source.is_some()
        {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
        // The reference header was charged by Graph::new. This borrows the
        // existing replay-checked index; it creates no inventory or authority.
        self.reuse.definition_source = Some(*query);
        Ok(self)
    }

    fn definition_from_index(
        &mut self,
        source: &fe2o3_pliron::ProductionSemanticSsaSourceQueryV1<'a>,
        value: SsaValueV1,
    ) -> Result<CapabilityDefinitionSiteV1, ProductionSemanticKirErrorV1> {
        use fe2o3_pliron::{
            ProductionSemanticSsaSourceQueryErrorV1 as QueryError,
            ProductionSemanticSsaValueOriginV1 as Origin,
        };
        let missing = || {
            reject("capability authority originates from an entry parameter or missing definition")
        };
        let SsaValueV1::Definition(id) = value else {
            return Err(missing());
        };
        let mut failure = None;
        let found = source.definition_origin(id, &mut || match self.charge(1) {
            Ok(()) => true,
            Err(error) => {
                failure = Some(error);
                false
            }
        });
        if let Some(error) = failure {
            return Err(error);
        }
        let (variable, origin) = found.map_err(|error| match error {
            QueryError::MissingDefinition => missing(),
            _ => ProductionSemanticKirErrorV1::CorrespondenceMismatch,
        })?;
        match origin {
            Origin::Entry { .. } | Origin::BlockArgument(_) => Err(missing()),
            Origin::Event { block, .. } => {
                let body = self
                    .body
                    .blocks()
                    .get(block.get() as usize)
                    .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
                let events = self
                    .ssa
                    .resolved_events(block)
                    .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
                self.charge(events.len())?;
                self.charge(body.statements().len())?;
                if events
                    .iter()
                    .filter(|(_, event)| {
                        matches!(event,
                    SsaResolvedEventV1::Define { variable: local, .. } if *local == variable)
                    })
                    .count()
                    != 1
                {
                    return Err(reject(
                        "capability definition has multiple source assignments in one block",
                    ));
                }
                let mut site = None;
                for (index, statement) in body.statements().iter().enumerate() {
                    if let SemanticStatementKindV1::Assign(assignment) = statement.kind()
                        && assignment.destination().local().index() == variable.get()
                        && (!assignment.destination().projections().is_empty()
                            || site.replace(index as u32).is_some())
                    {
                        return Err(reject("capability definition is projected or overwritten"));
                    }
                }
                let statement = site.ok_or_else(|| {
                    reject("capability definition has no checked source assignment")
                })?;
                Ok(CapabilityDefinitionSiteV1 {
                    block: block.get(),
                    statement: Some(statement),
                    local: variable.get(),
                })
            }
            Origin::Edge { edge, .. } => self.definition_call_source(edge.id(), variable.get()),
        }
    }

    fn definition_call_source(
        &mut self,
        edge: SsaEdgeIdV1,
        local: u32,
    ) -> Result<CapabilityDefinitionSiteV1, ProductionSemanticKirErrorV1> {
        self.charge(1)?;
        let block = edge.source().get();
        let body = self
            .body
            .blocks()
            .get(block as usize)
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        let SemanticTerminatorKindV1::Call(call) = body.terminator().kind() else {
            return Err(reject("capability issuer is not a source call"));
        };
        if edge.ordinal() != 0
            || !call.destination().is_some_and(|destination| {
                destination.place().local().index() == local
                    && destination.place().projections().is_empty()
            })
        {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
        Ok(CapabilityDefinitionSiteV1 {
            block,
            statement: None,
            local,
        })
    }
}
