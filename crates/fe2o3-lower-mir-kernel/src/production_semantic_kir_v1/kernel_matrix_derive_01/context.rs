// Typed Context origin resolution uses the shared SSA and storage-loan engine.
struct ContextResolver<'a, 'g> {
    owner: &'a ProductionSemanticSsaOwnerV1,
    context: &'a RootKernelContextLoweringV1,
    graph: &'g mut CapabilitySsaGraphV1<'a>,
    references: &'g mut BTreeMap<Site, Reference>,
}

impl ContextResolver<'_, '_> {
    fn step(
        &mut self,
        value: SsaValueV1,
        depth: usize,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        self.graph.charge(1)?;
        if depth > 128 || matches!(value, SsaValueV1::BlockArgument { .. }) {
            return Err(rejected(
                "Matrix Context custody requires a bounded unique SSA origin",
            ));
        }
        Ok(())
    }
    fn reference(
        &mut self,
        value: SsaValueV1,
        reference: SemanticTypeIdV1,
        depth: usize,
    ) -> Result<(SsaValueV1, BTreeSet<CapabilityLoanV1>), ProductionSemanticKirErrorV1> {
        self.step(value, depth)?;
        if !numerical_policy_math_shared_type_v1(
            self.owner.source_semantic().types(),
            reference,
            self.context.semantic_type,
        ) {
            return Err(rejected(
                "Matrix Context receiver lost its exact shared reference type",
            ));
        }
        let site = self.graph.definition(value)?;
        let statement = site
            .statement
            .ok_or_else(|| rejected("unmodeled reference-returning Matrix Context terminal"))?;
        let a = assignment_at(
            self.graph.body,
            (SemanticBlockIdV1::from_index(site.block), statement),
        )?
        .clone();
        if a.destination().ty() != reference || a.value().result_type() != reference {
            return Err(mismatch());
        }
        let (place, borrow) = match a.value().kind() {
            SemanticRvalueKindV1::Use(
                SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place),
            ) if place.ty() == reference && place.projections().is_empty() => (place, false),
            SemanticRvalueKindV1::Borrow {
                kind: SemanticBorrowKindV1::Shared,
                place,
            } if place.ty() == self.context.semantic_type => (place, true),
            _ => {
                return Err(rejected(
                    "Matrix receiver requires exact borrow or reference transfer",
                ));
            }
        };
        let local = place.local().index();
        let source = self.graph.use_value(site.block, local)?;
        let (issuer, loans) = if !borrow {
            self.reference(source, reference, depth + 1)?
        } else {
            match place.projections() {
                [] => {
                    let issuer = self.owned(source, depth + 1)?;
                    (
                        issuer,
                        BTreeSet::from([CapabilityLoanV1 {
                            borrow: site,
                            owner_local: local,
                            owner_value: source,
                        }]),
                    )
                }
                [projection] if projection.kind() == SemanticProjectionKindV1::Dereference => {
                    let actual = self
                        .graph
                        .body
                        .locals()
                        .get(local as usize)
                        .ok_or_else(mismatch)?
                        .ty();
                    self.reference(source, actual, depth + 1)?
                }
                _ => {
                    return Err(rejected(
                        "Matrix Context reborrow has an unauthenticated field projection",
                    ));
                }
            }
        };
        let key = (SemanticBlockIdV1::from_index(site.block), statement);
        let flow = Reference {
            assignment: a,
            local,
            value: source,
            issuer,
        };
        if let Some(previous) = self.references.get(&key) {
            if previous != &flow {
                return Err(mismatch());
            }
        } else {
            self.graph.charge(32)?;
            self.references.insert(key, flow);
        }
        Ok((issuer, loans))
    }
    fn owned(
        &mut self,
        value: SsaValueV1,
        depth: usize,
    ) -> Result<SsaValueV1, ProductionSemanticKirErrorV1> {
        self.step(value, depth)?;
        let site = self.graph.definition(value)?;
        if let Some(statement) = site.statement {
            if let Some(entry) = self.context.entry_transfer.as_ref()
                && value == entry.parameter
            {
                if entry.context != self.context.semantic_type
                    || entry.block.index() != site.block
                    || entry.statement != statement
                    || entry.destination.index() != site.local
                    || entry.issuer == value
                {
                    return Err(mismatch());
                }
                return self.owned(entry.issuer, depth + 1);
            }
            let a = assignment_at(
                self.graph.body,
                (SemanticBlockIdV1::from_index(site.block), statement),
            )?;
            if a.destination().ty() != self.context.semantic_type {
                return Err(mismatch());
            }
            let SemanticRvalueKindV1::Use(
                SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place),
            ) = a.value().kind()
            else {
                return Err(rejected(
                    "Matrix cannot reconstruct Context from a constant or aggregate",
                ));
            };
            if place.ty() != self.context.semantic_type || !place.projections().is_empty() {
                return Err(rejected(
                    "Matrix Context value has a substituted type or projected origin",
                ));
            }
            let source = self.graph.use_value(site.block, place.local().index())?;
            return self.owned(source, depth + 1);
        }
        let view = self
            .owner
            .execution_view_for_root(self.context.selected_root)
            .ok_or_else(mismatch)?;
        let origin = &view.block_origins()[site.block as usize];
        let SemanticTerminatorKindV1::Call(call) = self.graph.body.blocks()[site.block as usize]
            .terminator()
            .kind()
        else {
            return Err(mismatch());
        };
        if origin.terminator() != SemanticExpandedTerminatorOriginV1::Source
            || !call.arguments().is_empty()
            || !call.destination().is_some_and(|d| {
                d.place().local().index() == site.local
                    && d.place().projections().is_empty()
                    && d.place().ty() == self.context.semantic_type
            })
            || *self.owner.source_semantic().functions()[origin.function().index() as usize]
                .identity()
                .as_bytes()
                != self.context.source.function()
            || !matches!(self.owner.source_semantic().callables().get(call.callee().index() as usize),
                Some(SemanticCallableDeclV1::CompilerIntrinsic {
                    operation: SemanticCompilerIntrinsicOperationV1::KernelContextIssue { context }, ..
                }) if *context == self.context.semantic_type)
        {
            return Err(rejected(
                "Matrix requires the exact authenticated root Context issuer",
            ));
        }
        Ok(value)
    }
}
