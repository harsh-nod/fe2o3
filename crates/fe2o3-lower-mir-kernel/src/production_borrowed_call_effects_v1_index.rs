// Join the captured current source use to an exact native defining statement.
// Constant payload equality checks a recipe; it never identifies an SSA origin.
impl<'a> SliceQuery<'a, '_> {
    fn borrowed_index_use_v1(
        &self,
        site: fe2o3_pliron::ProductionSemanticSsaOccurrenceSiteV1,
        operand: fe2o3_pliron::ProductionSemanticSsaOperandRoleV1,
        role: fe2o3_pliron::ProductionSemanticSsaEventRoleV1,
        local: SemanticLocalIdV1,
        budget: &mut SliceBudget<'_>,
    ) -> SliceResult<fe2o3_mir_model::SsaValueV1> {
        use fe2o3_mir_model::{SsaEventV1, SsaResolvedEventV1};
        let mismatch = || ProductionSemanticKirErrorV1::CorrespondenceMismatch;
        let occurrences = self
            .subject
            .semantic_ssa
            .occurrences_v1()
            .ok_or_else(mismatch)?;
        let occurrences = occurrences
            .function(self.site.function)
            .ok_or_else(mismatch)?;
        let mut selected = None;
        for event in occurrences.events() {
            budget.charge_work(7)?;
            if event.site() != site || event.operand() != operand || event.role() != role {
                continue;
            }
            let Some(SsaResolvedEventV1::Use { variable, value }) = event.resolved() else {
                return Err(mismatch());
            };
            if !event.is_reachable()
                || !event.is_promoted()
                || variable.get() != local.index()
                || event.event() != SsaEventV1::Use(variable)
                || selected.replace(value).is_some()
            {
                return Err(mismatch());
            }
        }
        selected.ok_or_else(mismatch)
    }

    fn borrowed_index_definition_v1(
        &self,
        mut value: fe2o3_mir_model::SsaValueV1,
        budget: &mut SliceBudget<'_>,
    ) -> SliceResult<SliceDefinition> {
        use fe2o3_mir_model::{SsaEventV1, SsaResolvedEventV1};
        use fe2o3_pliron::{
            ProductionSemanticSsaEventRoleV1 as Role,
            ProductionSemanticSsaOccurrenceSiteV1 as Site,
            ProductionSemanticSsaOperandRoleV1 as Operand,
        };
        let mismatch = || ProductionSemanticKirErrorV1::CorrespondenceMismatch;
        let occurrences = self
            .subject
            .semantic_ssa
            .occurrences_v1()
            .ok_or_else(mismatch)?;
        let occurrences = occurrences
            .function(self.site.function)
            .ok_or_else(mismatch)?;
        let semantic = self.subject.semantic_ssa.source_semantic();
        let source = semantic
            .functions()
            .get(self.site.function.index() as usize)
            .ok_or_else(mismatch)?;
        for _ in 0..occurrences.events().len() {
            budget.charge_work(2)?;
            let mut selected = None;
            for event in occurrences.events() {
                budget.charge_work(3)?;
                if matches!(event.resolved(), Some(SsaResolvedEventV1::Define { value: defined, .. }) if defined == value)
                {
                    if selected.replace(event).is_some() {
                        return Err(mismatch());
                    }
                }
            }
            let event = selected.ok_or_else(|| {
                self.site
                    .unsupported("helper slice index requires an exact source assignment origin")
            })?;
            let Some(SsaResolvedEventV1::Define { variable, .. }) = event.resolved() else {
                return Err(mismatch());
            };
            let Site::Statement { block, statement } = event.site() else {
                return Err(mismatch());
            };
            if !event.is_reachable()
                || !event.is_promoted()
                || event.operand() != Operand::Destination
                || event.role() != Role::DestinationDefine
                || event.event() != SsaEventV1::Define(variable)
            {
                return Err(mismatch());
            }
            let statement_source = source
                .blocks()
                .get(block.get() as usize)
                .and_then(|block| block.statements().get(statement as usize))
                .ok_or_else(mismatch)?;
            let SemanticStatementKindV1::Assign(assignment) = statement_source.kind() else {
                return Err(mismatch());
            };
            if assignment.destination().local().index() != variable.get()
                || !assignment.destination().projections().is_empty()
            {
                return Err(mismatch());
            }
            budget.charge_work(4)?;
            let source_type = semantic
                .types()
                .get(assignment.value().result_type().index() as usize)
                .ok_or_else(mismatch)?;
            if !matches!(
                source_type.shape(),
                SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                    signed: false,
                    bits: 64
                })
            ) {
                return Err(self
                    .site
                    .unsupported("helper slice source index assignment requires u64"));
            }
            let ty = Type::Scalar(ScalarType::U64);
            let defining_query = SliceQuery {
                subject: self.subject,
                assertions: self.assertions,
                inventory: self.inventory,
                function: self.function,
                site: ProductionSliceAccessSiteV1 {
                    block: SemanticBlockIdV1::from_index(block.get()),
                    statement: Some(statement),
                    ..self.site
                },
                origins: self.origins,
            };
            let (native_block, first, count) = defining_query.source_span(budget)?;
            if native_block != BlockId(block.get()) {
                return Err(mismatch());
            }
            let native_block = self
                .inventory
                .block_for_id(self.function.coordinate, native_block, budget)
                .map_err(slice_inventory_error)?
                .ok_or_else(mismatch)?;
            let end = first
                .checked_add(count)
                .ok_or(ArgumentResourceV1::Arithmetic)?;
            let block_operations = self
                .inventory
                .operations()
                .get(native_block.operations.clone())
                .ok_or_else(mismatch)?;
            if self.borrowed_index_prefix_v1(block, statement, block_operations, budget)?
                != first as usize
            {
                return Err(mismatch());
            }
            let operations = block_operations
                .get(first as usize..end as usize)
                .ok_or_else(mismatch)?;
            budget.charge_work(operations.len())?;
            match assignment.value().kind() {
                SemanticRvalueKindV1::Use(SemanticOperandV1::Constant(constant)) => {
                    let SemanticConstantValueV1::Scalar(bits) = constant.value() else {
                        return Err(mismatch());
                    };
                    let [operation] = operations else {
                        return Err(mismatch());
                    };
                    let [result] = operation.operation.results.as_slice() else {
                        return Err(mismatch());
                    };
                    if bits.size_bytes() != 8
                        || result.ty != ty
                        || operation.operation.kind
                            != OperationKind::Constant(Constant::U64(bits.bits() as u64))
                    {
                        return Err(mismatch());
                    }
                    return Ok(SliceDefinition::Result {
                        operation: operation.coordinate,
                        result: 0,
                    });
                }
                SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place))
                    if place.projections().is_empty() && operations.is_empty() =>
                {
                    value = self.borrowed_index_use_v1(
                        event.site(),
                        Operand::RvalueOperand(0),
                        Role::BaseUse,
                        place.local(),
                        budget,
                    )?;
                }
                _ => {
                    return Err(self
                        .site
                        .unsupported("helper slice source index recipe is unsupported"));
                }
            }
        }
        Err(self
            .site
            .unsupported("helper slice source index transport is cyclic"))
    }

    fn borrowed_index_prefix_v1(
        &self,
        block: fe2o3_mir_model::SsaBlockIdV1,
        statement: u32,
        operations: &[CanonicalKirOperationRefV1<'_>],
        budget: &mut SliceBudget<'_>,
    ) -> SliceResult<usize> {
        use fe2o3_pliron::{
            ProductionSemanticSsaOccurrenceSiteV1 as Site,
            ProductionSemanticSsaOperandRoleV1 as Operand,
        };
        let mismatch = || ProductionSemanticKirErrorV1::CorrespondenceMismatch;
        let semantic = self.subject.semantic_ssa.source_semantic();
        let statements = semantic
            .functions()
            .get(self.site.function.index() as usize)
            .and_then(|function| function.blocks().get(block.get() as usize))
            .and_then(|block| block.statements().get(..statement as usize))
            .ok_or_else(mismatch)?;
        let mut next = 0_usize;
        for (ordinal, statement) in statements.iter().enumerate() {
            budget.charge_work(12)?;
            let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
                return Err(self
                    .site
                    .unsupported("helper slice index prefix recipe is unsupported"));
            };
            if !assignment.destination().projections().is_empty()
                || !matches!(
                    semantic
                        .types()
                        .get(assignment.value().result_type().index() as usize)
                        .map(SemanticTypeDeclV1::shape),
                    Some(SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                        signed: false,
                        bits: 64
                    }))
                )
            {
                return Err(self
                    .site
                    .unsupported("helper slice index prefix requires whole u64 assignments"));
            }
            match assignment.value().kind() {
                SemanticRvalueKindV1::Use(SemanticOperandV1::Constant(constant)) => {
                    let SemanticConstantValueV1::Scalar(bits) = constant.value() else {
                        return Err(mismatch());
                    };
                    let operation = operations.get(next).ok_or_else(mismatch)?.operation;
                    if bits.size_bytes() != 8
                        || operation.kind
                            != OperationKind::Constant(Constant::U64(bits.bits() as u64))
                        || !matches!(operation.results.as_slice(), [result] if result.ty == Type::Scalar(ScalarType::U64))
                    {
                        return Err(mismatch());
                    }
                    next = argument_sum_v1(&[next, 1])?;
                }
                SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place))
                    if place.projections().is_empty() => {}
                SemanticRvalueKindV1::Unary {
                    operation: SemanticUnaryOpV1::PointerMetadata,
                    operand: SemanticOperandV1::Copy(place),
                } if place.projections().is_empty() => {
                    let length = operations.get(next).ok_or_else(mismatch)?.operation;
                    let OperationKind::SliceLength { slice } = length.kind else {
                        return Err(mismatch());
                    };
                    let [result] = length.results.as_slice() else {
                        return Err(mismatch());
                    };
                    if result.ty != Type::INDEX {
                        return Err(mismatch());
                    }
                    let input = self.origin(slice, budget)?;
                    let definition = self
                        .inventory
                        .definition_for_value(self.function.coordinate, slice, budget)
                        .map_err(slice_inventory_error)?
                        .ok_or_else(mismatch)?;
                    if !matches!(definition.ty, Type::Slice(slice) if slice.address_space == AddressSpace::Global && slice.access == AccessMode::ReadOnly)
                    {
                        return Err(mismatch());
                    }
                    checked_borrowed_slice_source_input_v1(
                        self.subject,
                        self.site.root,
                        self.site.function,
                        input,
                        place.local(),
                        Site::Statement {
                            block,
                            statement: u32::try_from(ordinal)
                                .map_err(|_| ArgumentResourceV1::Arithmetic)?,
                        },
                        Operand::RvalueOperand(0),
                        budget,
                    )?;
                    next = argument_sum_v1(&[next, 1])?;
                }
                _ => {
                    return Err(self
                        .site
                        .unsupported("helper slice index prefix recipe is unsupported"));
                }
            }
        }
        Ok(next)
    }
}
