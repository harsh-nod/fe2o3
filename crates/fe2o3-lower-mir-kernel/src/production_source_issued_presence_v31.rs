// These names denote checked issuer predicates, not arbitrary enum values or
// permission to dereference an issued pointer. The original issuer roster and
// its output replay remain the authority for each physical predicate.
#[derive(Clone, Copy)]
struct SourceIssuedPresenceV31 {
    issuer: usize,
    instance: usize,
    definition: EntryValueV20,
    operation: fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1,
    value: ValueId,
    symbol: u32,
}

struct SourceIssuedPresencesV31 {
    rows: Vec<SourceIssuedPresenceV31>,
    values: Vec<(ValueId, usize)>,
    definitions: Vec<(usize, EntryValueV20, usize)>,
}

impl SourceIssuedPresencesV31 {
    fn empty() -> Self {
        Self {
            rows: Vec::new(),
            values: Vec::new(),
            definitions: Vec::new(),
        }
    }
}

#[derive(Clone, Copy)]
struct OptimizedIssuedPresenceV31 {
    original: usize,
    operation: fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1,
    value: ValueId,
}

type OptimizedScalarPreparedV31<'a> = (
    Vec<OptimizedSourceScalarReadV18>,
    Vec<SourceWrappingValueV23>,
    OptimizedSourceScalarBoundariesV31,
    Vec<OptimizedIssuedPresenceV31>,
    &'a fe2o3_kernel_analysis::CanonicalKirFunctionRefV1<'a>,
    usize,
);
type OptimizedScalarCatchFrameV31<'a> = (
    &'a ProductionOptimizedSourceScalarLeavesV18<'a>,
    &'a mut (),
    &'a mut ArgumentBudgetV1<'a>,
);

fn issued_discriminant_query_headers_v31() -> Result<usize, ArgumentResourceV1> {
    type Arguments<'a> = (
        &'a OriginalEntryIndexV20<'a, 'a>,
        &'a ProductionSourceScalarLeavesV18<'a>,
        usize,
        SemanticFunctionIdV1,
        &'a SemanticFunctionDeclV1,
        EntrySiteV20,
        &'a PrivateRvalueV22,
        &'a SemanticPlaceV1,
        SemanticTypeIdV1,
        ProductionSemanticScalarTypeV2,
        usize,
        &'a mut usize,
        &'a mut ArgumentBudgetV1<'a>,
    );
    argument_sum_v1(&[
        size_of::<Arguments<'_>>(),
        std::mem::align_of::<Arguments<'_>>(),
        size_of::<OriginalEntryDefinitionRowV20>(),
        size_of::<EntryValueV20>(),
        size_of::<ProductionSemanticExpressionV2>(),
        argument_product_v1(
            2,
            size_of::<SourceOwnedResultV18<ProductionSemanticExpressionV2>>(),
        )?,
        size_of::<SourceOwnedResultV18<()>>(),
        argument_product_v1(8, size_of::<&()>())?,
    ])
}

fn issued_discriminant_encoding_v31(
    types: &[SemanticTypeDeclV1],
    option: SemanticTypeIdV1,
    ty: SemanticTypeIdV1,
    scalar: ProductionSemanticScalarTypeV2,
) -> SourceOwnedResultV18<()> {
    let (discriminant, _) =
        semantic_enum_shape(types, option).map_err(source_emission_error_v18)?;
    if discriminant != ty
        || option_payload_type_v1(types, option).is_none()
        || kir_semantic_scalar_v1(&lower_scalar_type(types, ty).map_err(source_emission_error_v18)?)
            != Some(scalar)
        || !matches!(
            scalar,
            ProductionSemanticScalarTypeV2::Bool
                | ProductionSemanticScalarTypeV2::Integer {
                    bits: 8 | 16 | 32 | 64,
                    ..
                }
        )
    {
        return Err(ProductionSourceOwnedViewErrorV18::Binding(
            "issued discriminant declared encoding or type is unsupported",
        ));
    }
    Ok(())
}

fn source_presence_headers_v31() -> Result<usize, ArgumentResourceV1> {
    fn h<T>() -> Result<usize, ArgumentResourceV1> {
        argument_sum_v1(&[
            size_of::<T>(),
            argument_product_v1(2, size_of::<SourceOwnedResultV18<T>>())?,
        ])
    }
    argument_sum_v1(&[
        scoped_raw_admission_v29::source_issuer_query_headers_v31()?,
        h::<Vec<SourceIssuedPresenceV31>>()?,
        h::<SourceIssuedPresencesV31>()?,
        h::<Vec<(ValueId, usize)>>()?,
        h::<Vec<(usize, EntryValueV20, usize)>>()?,
        h::<SourceIssuedPresenceV31>()?,
        h::<Option<&SourceIssuedPresenceV31>>()?,
        h::<&PendingSourceIssuedIssuerV29>()?,
        h::<&[PendingSourceIssuedIssuerV29]>()?,
        h::<ProductionSemanticExpressionV2>()?,
        h::<OriginalEntryDefinitionRowV20>()?,
        h::<EntryValueV20>()?,
        h::<(SemanticTypeIdV1, &[SemanticEnumVariantV1])>()?,
        h::<(ValueId, usize)>()?,
        h::<(usize, EntryValueV20, usize)>()?,
        size_of::<std::slice::Iter<'_, (ValueId, usize)>>(),
        size_of::<std::slice::Iter<'_, (usize, EntryValueV20, usize)>>(),
        size_of::<std::slice::Iter<'_, SourceIssuedPresenceV31>>(),
        size_of::<std::slice::Iter<'_, PendingSourceIssuedIssuerV29>>(),
        argument_product_v1(12, size_of::<usize>())?,
        argument_product_v1(12, size_of::<&()>())?,
    ])
}

fn source_issued_presences_v31(
    relation: &ProductionSourceCorrespondenceV18<'_>,
    root: usize,
    memory: &[SourceScalarLeafRowV18],
    boundaries: &SourceScalarBoundariesV31,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<SourceIssuedPresencesV31> {
    relation.query(budget)?;
    budget.reserve_storage(source_presence_headers_v31()?)?;
    let issuers = scoped_raw_admission_v29::checked_source_issuers_v31(relation, root, budget)?;
    let mut rows = emission_vec_v1(issuers.len(), budget).map_err(source_emission_error_v18)?;
    let mut next = 0u32;
    for symbol in memory
        .iter()
        .map(|row| row.symbol)
        .chain(boundaries.rows.iter().map(|row| row.symbol))
    {
        budget.charge_work(1)?;
        next = next.max(
            symbol
                .checked_add(1)
                .ok_or(ArgumentResourceV1::Arithmetic)?,
        );
    }
    for (issuer, row) in issuers.iter().enumerate() {
        budget.charge_work(4)?;
        if next >= PRODUCTION_KERNEL_SCALAR_SYMBOL_BASE_V2 {
            return relation
                .source
                .missing("issued presence exhausts source scalar names");
        }
        let [_, operation, _, _] = scoped_raw_admission_v29::source_issued_tail_locations_v18(
            relation, root, row, budget,
        )?;
        if rows.len() == rows.capacity() {
            return Err(ArgumentResourceV1::Accounting.into());
        }
        rows.push(SourceIssuedPresenceV31 {
            issuer,
            instance: row.instance.index(),
            definition: row.definition,
            operation,
            value: row.present,
            symbol: next,
        });
        next = next.checked_add(1).ok_or(ArgumentResourceV1::Arithmetic)?;
    }
    let mut values = emission_vec_v1(rows.len(), budget).map_err(source_emission_error_v18)?;
    let mut definitions = emission_vec_v1(rows.len(), budget).map_err(source_emission_error_v18)?;
    for (index, row) in rows.iter().enumerate() {
        budget.charge_work(2)?;
        if values.len() == values.capacity() || definitions.len() == definitions.capacity() {
            return Err(ArgumentResourceV1::Accounting.into());
        }
        values.push((row.value, index));
        definitions.push((row.instance, row.definition, index));
    }
    call_splice_sort_work_v1(values.len(), budget)
        .map_err(source_address_call_error_v29)
        .map_err(source_emission_error_v18)?;
    values.sort_unstable_by_key(|row| row.0);
    call_splice_sort_work_v1(definitions.len(), budget)
        .map_err(source_address_call_error_v29)
        .map_err(source_emission_error_v18)?;
    definitions.sort_unstable_by_key(|row| (row.0, row.1));
    for pair in values.windows(2) {
        budget.charge_work(1)?;
        if pair[0].0 == pair[1].0 {
            return relation
                .source
                .missing("issued presence value is ambiguous");
        }
    }
    for pair in definitions.windows(2) {
        budget.charge_work(1)?;
        if (pair[0].0, pair[0].1) == (pair[1].0, pair[1].1) {
            return relation
                .source
                .missing("issued presence definition is ambiguous");
        }
    }
    Ok(SourceIssuedPresencesV31 {
        rows,
        values,
        definitions,
    })
}

impl SourceScalarLeavesV18<'_, '_> {
    fn presence_row_v31(
        &self,
        row: &SourceIssuedPresenceV31,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<()> {
        self.query(budget)?;
        let issuer = self
            .relation
            .source
            .root_row(self.root)?
            .source_slots
            .pending_memory
            .as_ref()
            .and_then(|pending| {
                scoped_raw_admission_v29::retained_source_issuer_v31(pending, row.issuer)
            })
            .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                "issued presence original issuer absent",
            ))?;
        budget.charge_work(7)?;
        if issuer.instance.index() != row.instance
            || issuer.definition != row.definition
            || issuer.present != row.value
        {
            return self
                .relation
                .source
                .missing("issued presence original issuer changed");
        }
        let [_, operation, _, _] = scoped_raw_admission_v29::source_issued_tail_locations_v18(
            self.relation,
            self.root,
            issuer,
            budget,
        )?;
        let actual =
            source_operation_row_v18(self.relation.inventory, operation, budget)?.operation;
        if operation != row.operation
            || !matches!(actual.results.as_slice(), [result] if result.id == row.value && result.ty == Type::BOOL)
            || !matches!(actual.kind, OperationKind::Compare { predicate: ComparePredicate::LessThan, lhs, rhs }
                if lhs == issuer.index && rhs == issuer.length)
        {
            return self
                .relation
                .source
                .missing("issued presence original predicate changed");
        }
        Ok(())
    }

    fn presence_symbol_v31(
        &self,
        symbol: u32,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<Option<&SourceIssuedPresenceV31>> {
        self.query(budget)?;
        budget.charge_work(2)?;
        let Some(first) = self.presences.rows.first() else {
            return Ok(None);
        };
        let Some(index) = symbol.checked_sub(first.symbol) else {
            return Ok(None);
        };
        let Some(row) = self.presences.rows.get(index as usize) else {
            return Ok(None);
        };
        if row.symbol != symbol {
            return self
                .relation
                .source
                .missing("issued presence symbol order changed");
        }
        self.presence_row_v31(row, budget)?;
        Ok(Some(row))
    }

    fn presence_value_v31(
        &self,
        value: ValueId,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<Option<&SourceIssuedPresenceV31>> {
        self.query(budget)?;
        charge_execution_cfg_lookup_v29(self.presences.values.len(), budget)
            .map_err(source_emission_error_v18)?;
        let Ok(index) = self
            .presences
            .values
            .binary_search_by_key(&value, |row| row.0)
        else {
            return Ok(None);
        };
        let row = self
            .presences
            .rows
            .get(self.presences.values[index].1)
            .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                "issued presence value index changed",
            ))?;
        if row.value != value {
            return self
                .relation
                .source
                .missing("issued presence value index changed");
        }
        self.presence_row_v31(row, budget)?;
        Ok(Some(row))
    }

    fn presence_definition_v31(
        &self,
        instance: usize,
        definition: EntryValueV20,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<Option<&SourceIssuedPresenceV31>> {
        self.query(budget)?;
        charge_execution_cfg_lookup_v29(self.presences.definitions.len(), budget)
            .map_err(source_emission_error_v18)?;
        let Ok(index) = self
            .presences
            .definitions
            .binary_search_by_key(&(instance, definition), |row| (row.0, row.1))
        else {
            return Ok(None);
        };
        let row = self
            .presences
            .rows
            .get(self.presences.definitions[index].2)
            .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                "issued presence definition index changed",
            ))?;
        if (row.instance, row.definition) != (instance, definition) {
            return self
                .relation
                .source
                .missing("issued presence definition index changed");
        }
        self.presence_row_v31(row, budget)?;
        Ok(Some(row))
    }
}

impl OriginalEntryIndexV20<'_, '_> {
    fn issued_discriminant_v31(
        &self,
        leaves: &ProductionSourceScalarLeavesV18<'_>,
        instance: usize,
        function_id: SemanticFunctionIdV1,
        function: &SemanticFunctionDeclV1,
        site: EntrySiteV20,
        value: &PrivateRvalueV22,
        place: &SemanticPlaceV1,
        ty: SemanticTypeIdV1,
        scalar: ProductionSemanticScalarTypeV2,
        mut depth: usize,
        remaining: &mut usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<ProductionSemanticExpressionV2> {
        self.check(budget)?;
        leaves.leaves.query(budget)?;
        let source = self.source.source.source_semantic(budget)?;
        budget.charge_work(8)?;
        let expected_function = self
            .source
            .source
            .instance(leaves.leaves.root, instance, budget)?
            .0;
        if !std::ptr::eq(self.source, leaves.leaves.relation)
            || function_id != expected_function
            || !source
                .functions()
                .get(function_id.index() as usize)
                .is_some_and(|original| std::ptr::eq(original, function))
        {
            return self
                .source
                .source
                .missing("issued discriminant original function or owner differs");
        }
        let Some(SemanticStatementKindV1::Assign(assignment)) =
            scoped_source_statement_v29(function, site)
        else {
            return self
                .source
                .source
                .missing("issued discriminant original assignment absent");
        };
        if !std::ptr::eq(assignment.value(), value)
            || !matches!(value.kind(), SemanticRvalueKindV1::Discriminant(original) if std::ptr::eq(original, place))
            || !place.projections().is_empty()
            || function
                .locals()
                .get(place.local().index() as usize)
                .map(|local| local.ty())
                != Some(place.ty())
        {
            return self
                .source
                .source
                .missing("issued discriminant original place differs");
        }
        issued_discriminant_encoding_v31(source.types(), place.ty(), ty, scalar)?;
        let mut definition = self.promoted_use(
            function_id,
            site,
            EntryOperandV20::RvaluePlace,
            place.local().index(),
            budget,
        )?;
        loop {
            if depth > MAX_PRODUCTION_SEMANTIC_EXPRESSION_DEPTH_V2 || *remaining == 0 {
                return self
                    .source
                    .source
                    .missing("issued discriminant source forwarding exceeds expression bounds");
            }
            *remaining -= 1;
            depth += 1;
            if let Some(row) = leaves
                .leaves
                .presence_definition_v31(instance, definition, budget)?
            {
                let pending = self
                    .source
                    .source
                    .root_row(leaves.leaves.root)?
                    .source_slots
                    .pending_memory
                    .as_ref()
                    .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                        "issued discriminant issuer owner absent",
                    ))?;
                let issuer =
                    scoped_raw_admission_v29::retained_source_issuer_v31(pending, row.issuer)
                        .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                            "issued discriminant original issuer absent",
                        ))?;
                let Some(SemanticTerminatorKindV1::Call(call)) = function
                    .blocks()
                    .get(issuer.block.index() as usize)
                    .map(|block| block.terminator().kind())
                else {
                    return self
                        .source
                        .source
                        .missing("issued discriminant original call absent");
                };
                if call
                    .destination()
                    .map(|destination| destination.place().ty())
                    != Some(place.ty())
                {
                    return self
                        .source
                        .source
                        .missing("issued discriminant option type differs from issuer");
                }
                let predicate = ProductionSemanticExpressionV2::Symbol {
                    symbol: row.symbol,
                    scalar: ProductionSemanticScalarTypeV2::Bool,
                };
                if scalar == ProductionSemanticScalarTypeV2::Bool {
                    return Ok(predicate);
                }
                budget.reserve_storage(size_of::<ProductionSemanticExpressionV2>())?;
                return Ok(ProductionSemanticExpressionV2::Cast {
                    kind: fe2o3_pliron::ProductionSemanticCastV2::Integer,
                    source: ProductionSemanticScalarTypeV2::Bool,
                    target: scalar,
                    operand: Box::new(predicate),
                });
            }
            // Whole-value original copies may forward an issued option. No
            // physical value, guessed predecessor, or general enum is a source.
            let origin = self.definition(function_id, definition, budget)?;
            let OriginalEntryDefinitionV20::Assignment { block, statement } = origin.origin else {
                return self
                    .source
                    .source
                    .missing("issued discriminant has no original issued source");
            };
            let Some(SemanticStatementKindV1::Assign(assignment)) = function
                .blocks()
                .get(block as usize)
                .and_then(|block| block.statements().get(statement as usize))
                .map(|statement| statement.kind())
            else {
                return self
                    .source
                    .source
                    .missing("issued discriminant forwarding assignment absent");
            };
            let SemanticRvalueKindV1::Use(
                SemanticOperandV1::Copy(input) | SemanticOperandV1::Move(input),
            ) = assignment.value().kind()
            else {
                return self
                    .source
                    .source
                    .missing("issued discriminant forwarding is unsupported");
            };
            if assignment.destination().local().index() != origin.local
                || !assignment.destination().projections().is_empty()
                || assignment.destination().ty() != place.ty()
                || assignment.value().result_type() != place.ty()
                || !input.projections().is_empty()
                || input.ty() != place.ty()
            {
                return self
                    .source
                    .source
                    .missing("issued discriminant forwarding type or place differs");
            }
            definition = self.promoted_use(
                function_id,
                EntrySiteV20::Statement {
                    block: fe2o3_mir_model::SsaBlockIdV1::new(block),
                    statement,
                },
                EntryOperandV20::RvalueOperand(0),
                input.local().index(),
                budget,
            )?;
        }
    }
}

impl ProductionOptimizedSourceScalarLeavesV18<'_> {
    fn presence_value_v31(
        &self,
        value: ValueId,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<Option<&SourceIssuedPresenceV31>> {
        self.check(budget)?;
        let mut low = 0;
        let mut high = self.presences.len();
        while low < high {
            budget.charge_work(1)?;
            let middle = low + (high - low) / 2;
            if self.presences[middle].value < value {
                low = middle + 1;
            } else {
                high = middle;
            }
        }
        budget.charge_work(4)?;
        let Some(row) = self.presences.get(low).filter(|row| row.value == value) else {
            return Ok(None);
        };
        let source = self
            .original
            .leaves
            .presences
            .rows
            .get(row.original)
            .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                "optimized issued presence source row absent",
            ))?;
        self.original.leaves.presence_row_v31(source, budget)?;
        let inventory = self.optimized.output_inventory(budget)?;
        let actual = source_operation_row_v18(inventory, row.operation, budget)?.operation;
        if row.operation.block.function != self.function.coordinate
            || !matches!(self.optimized.operation(source.operation, budget)?,
                ProductionOptimizedSourceOperationV18::Retained { output, .. } if output == row.operation)
            || !matches!(actual.results.as_slice(), [result] if result.id == value && result.ty == Type::BOOL)
            || !matches!(
                actual.kind,
                OperationKind::Compare {
                    predicate: ComparePredicate::LessThan,
                    ..
                }
            )
        {
            return self
                .original
                .leaves
                .relation
                .source
                .missing("optimized issued presence actual predicate changed");
        }
        Ok(Some(source))
    }
}
