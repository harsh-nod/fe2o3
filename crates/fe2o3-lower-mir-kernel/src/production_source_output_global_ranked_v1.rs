type GlobalRankedSiteRowV1 = (SemanticAccessSiteV1, IndexedRankedAccessSourceV1);
type GlobalRankedLocationRowV1 = ((u32, u32), SemanticAccessSiteV1);
type GlobalOriginalSiteRowV1 = (
    fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1,
    SemanticAccessSiteV1,
);
type GlobalRankedExpressionRowV1<'a> = (
    ProductionRankedValueIdV1,
    &'a ProductionSemanticExpressionV2,
    ProductionNumericalContractV2,
);

struct SourceOutputScalarNormalizationV1<'a, 'kir, 'ranked, 'ledger, 'limit> {
    inventory: &'a fe2o3_kernel_analysis::CanonicalKirInventoryV1<'kir>,
    function: &'a fe2o3_kernel_analysis::CanonicalKirFunctionRefV1<'kir>,
    lowering: &'ranked ProductionRankedKernelLoweringInputV1,
    sources: Vec<GlobalRankedSiteRowV1>,
    locations: Vec<GlobalRankedLocationRowV1>,
    sites: Vec<GlobalOriginalSiteRowV1>,
    views: Vec<(ProductionRankedValueIdV1, RankedViewDefinitionV1)>,
    expressions: Vec<GlobalRankedExpressionRowV1<'ranked>>,
    nodes: Vec<NormalizedScalarNodeV1<usize>>,
    comparison: [(usize, usize); 2 * MAX_PRODUCTION_SEMANTIC_EXPRESSION_DEPTH_V2 + 1],
    visiting: [Option<ValueId>; MAX_PRODUCTION_SEMANTIC_EXPRESSION_DEPTH_V2 + 1],
    visiting_len: usize,
    normalization_steps: usize,
    ssa: SourceOutputAllocationScratchV1,
    error: Option<ProductionSourceOutputErrorV1>,
    budget: &'ledger mut AssertOriginBudgetV1<'limit>,
}

impl SourceOutputScalarNormalizationV1<'_, '_, '_, '_, '_> {
    fn remember<T>(&mut self, result: Result<T, ProductionSourceOutputErrorV1>) -> Option<T> {
        match result {
            Ok(value) => Some(value),
            Err(error) => {
                self.error.get_or_insert(error);
                None
            }
        }
    }
    fn paid(&mut self, work: usize) -> Option<()> {
        let result = self
            .budget
            .charge_work(work)
            .map_err(ProductionSourceOutputErrorV1::Resource);
        self.remember(result)
    }
    fn failure(&mut self) -> ProductionSourceOutputErrorV1 {
        self.error
            .take()
            .unwrap_or(ProductionSourceOutputErrorV1::Invalid(
                "global ranked value expression mismatch",
            ))
    }
    fn equivalent(&mut self, left: usize, right: usize) -> Option<bool> {
        self.paid(2)?;
        self.comparison[0] = (left, right);
        let mut pending = 1;
        while pending != 0 {
            self.paid(9)?;
            pending -= 1;
            let (left, right) = self.comparison[pending];
            let (left_shape, left_children) = normalized_scalar_shape_v1(self.nodes.get(left)?);
            let (right_shape, right_children) = normalized_scalar_shape_v1(self.nodes.get(right)?);
            if left_shape != right_shape {
                return Some(false);
            }
            for child in (0..3).rev() {
                self.paid(1)?;
                match (left_children[child], right_children[child]) {
                    (Some(lhs), Some(rhs)) => {
                        self.paid(2)?;
                        if lhs >= left || rhs >= right {
                            return None;
                        }
                        *self.comparison.get_mut(pending)? = (lhs, rhs);
                        pending += 1;
                    }
                    (None, None) => {}
                    _ => return Some(false),
                }
            }
        }
        Some(true)
    }
}

// This is representation traversal only. Scalar operator meaning remains in
// the shared normalizer; children are visited in exact original operand order.
fn normalized_scalar_shape_v1(
    node: &NormalizedScalarNodeV1<usize>,
) -> (NormalizedScalarNodeV1<()>, [Option<usize>; 3]) {
    use NormalizedScalarNodeV1 as Node;
    match node {
        Node::Symbol { symbol, scalar } => (
            Node::Symbol {
                symbol: *symbol,
                scalar: *scalar,
            },
            [None; 3],
        ),
        Node::Constant { scalar, bits } => (
            Node::Constant {
                scalar: *scalar,
                bits: *bits,
            },
            [None; 3],
        ),
        Node::Load { site, scalar } => (
            Node::Load {
                site: *site,
                scalar: *scalar,
            },
            [None; 3],
        ),
        Node::Unary {
            operation,
            scalar,
            operand,
        } => (
            Node::Unary {
                operation: *operation,
                scalar: *scalar,
                operand: (),
            },
            [Some(*operand), None, None],
        ),
        Node::Binary {
            operation,
            scalar,
            overflow,
            lhs,
            rhs,
        } => (
            Node::Binary {
                operation: *operation,
                scalar: *scalar,
                overflow: *overflow,
                lhs: (),
                rhs: (),
            },
            [Some(*lhs), Some(*rhs), None],
        ),
        Node::Compare {
            operation,
            operand_scalar,
            lhs,
            rhs,
        } => (
            Node::Compare {
                operation: *operation,
                operand_scalar: *operand_scalar,
                lhs: (),
                rhs: (),
            },
            [Some(*lhs), Some(*rhs), None],
        ),
        Node::Select {
            scalar,
            condition,
            when_true,
            when_false,
        } => (
            Node::Select {
                scalar: *scalar,
                condition: (),
                when_true: (),
                when_false: (),
            },
            [Some(*condition), Some(*when_true), Some(*when_false)],
        ),
        Node::Cast {
            kind,
            source,
            target,
            operand,
        } => (
            Node::Cast {
                kind: *kind,
                source: *source,
                target: *target,
                operand: (),
            },
            [Some(*operand), None, None],
        ),
    }
}

impl<'kir, 'ranked> ScalarNormalizationContextV1<'kir, 'ranked>
    for SourceOutputScalarNormalizationV1<'_, 'kir, 'ranked, '_, '_>
{
    type Node = usize;
    fn charge(&mut self) -> Option<()> {
        self.paid(2)?;
        self.normalization_steps = self.normalization_steps.checked_add(1)?;
        Some(())
    }
    fn enter(&mut self, value: ValueId) -> Option<bool> {
        self.paid(self.visiting_len.checked_add(2)?)?;
        if self.visiting[..self.visiting_len].contains(&Some(value)) {
            return Some(false);
        }
        *self.visiting.get_mut(self.visiting_len)? = Some(value);
        self.visiting_len += 1;
        Some(true)
    }
    fn leave(&mut self, value: ValueId) {
        // Its matching enter prepaid this fixed slot removal, including failure unwind.
        if self.visiting_len > 0 && self.visiting[self.visiting_len - 1] == Some(value) {
            self.visiting_len -= 1;
            self.visiting[self.visiting_len] = None;
        }
    }
    fn emit(&mut self, node: NormalizedScalarNodeV1<usize>) -> Option<usize> {
        // Append-only postorder storage has amortized constant append work.
        let index = self.nodes.len();
        let result = assert_origin_push_v1(&mut self.nodes, node, self.budget)
            .map_err(ProductionSourceOutputErrorV1::SourceOrigin);
        self.remember(result)?;
        Some(index)
    }
    fn unique_origin(&mut self, value: ValueId) -> Option<ValueId> {
        self.paid(3)?;
        self.ssa.generation = self.ssa.generation.checked_add(1)?;
        self.ssa.pending.clear();
        let mut worklist = SourceOutputScalarSsaOriginV1(SourceOutputAllocationWorklistV1 {
            inventory: self.inventory,
            function: self.function,
            scratch: &mut self.ssa,
            current: None,
            budget: self.budget,
        });
        let result = worklist
            .0
            .push(value)
            .and_then(|()| scalar_ssa_origin_worklist_v1(&mut worklist));
        self.remember(result).flatten()
    }
    fn parameter(
        &mut self,
        value: ValueId,
    ) -> Option<Option<(u32, ProductionSemanticScalarTypeV2)>> {
        let result = self
            .inventory
            .definition_for_value(self.function.coordinate, value, self.budget)
            .map_err(ProductionSourceOutputErrorV1::Inventory);
        let definition = self.remember(result)??;
        self.paid(2)?;
        if let fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1::FunctionArgument {
            argument,
            ..
        } = definition.coordinate
        {
            Some(Some((argument, kir_semantic_scalar_v1(definition.ty)?)))
        } else {
            Some(None)
        }
    }
    fn operation(&mut self, value: ValueId) -> Option<&'kir Operation> {
        let result = self
            .inventory
            .definition_for_value(self.function.coordinate, value, self.budget)
            .map_err(ProductionSourceOutputErrorV1::Inventory);
        let definition = self.remember(result)??;
        self.paid(1)?;
        let fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1::Result { operation, .. } =
            definition.coordinate
        else {
            return None;
        };
        let result = source_output_operation_v1(self.inventory.owner(), operation, self.budget);
        let operation = self.remember(result)?;
        self.paid(operation.results.len())?;
        Some(operation)
    }
    fn scalar(&mut self, value: ValueId) -> Option<ProductionSemanticScalarTypeV2> {
        let result = self
            .inventory
            .definition_for_value(self.function.coordinate, value, self.budget)
            .map_err(ProductionSourceOutputErrorV1::Inventory);
        let definition = self.remember(result)??;
        self.paid(1)?;
        kir_semantic_scalar_v1(definition.ty)
    }
    fn load_site(&mut self, value: ValueId) -> Option<SemanticAccessSiteV1> {
        let result = self
            .inventory
            .definition_for_value(self.function.coordinate, value, self.budget)
            .map_err(ProductionSourceOutputErrorV1::Inventory);
        let definition = self.remember(result)??;
        self.paid(1)?;
        let fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1::Result { operation, .. } =
            definition.coordinate
        else {
            return None;
        };
        let result = assert_origin_find_v1(&self.sites, self.budget, |row, budget| {
            budget.charge_work(4)?;
            Ok(row.0.cmp(&operation))
        })
        .map_err(ProductionSourceOutputErrorV1::SourceOrigin);
        let index = self.remember(result)??;
        self.paid(1)?;
        Some(self.sites[index].1)
    }
    fn ranked_site(&mut self, location: (u32, u32)) -> Option<SemanticAccessSiteV1> {
        let result = assert_origin_find_v1(&self.locations, self.budget, |row, budget| {
            budget.charge_work(2)?;
            Ok(row.0.cmp(&location))
        })
        .map_err(ProductionSourceOutputErrorV1::SourceOrigin);
        let index = self.remember(result)??;
        self.paid(1)?;
        Some(self.locations[index].1)
    }
    fn ranked_source(&mut self, site: SemanticAccessSiteV1) -> Option<IndexedRankedAccessSourceV1> {
        let result = assert_origin_find_v1(&self.sources, self.budget, |row, budget| {
            budget.charge_work(3)?;
            Ok(row.0.cmp(&site))
        })
        .map_err(ProductionSourceOutputErrorV1::SourceOrigin);
        let index = self.remember(result)??;
        self.paid(1)?;
        Some(self.sources[index].1)
    }
    fn ranked_view(&mut self, value: ProductionRankedValueIdV1) -> Option<RankedViewDefinitionV1> {
        let result = assert_origin_find_v1(&self.views, self.budget, |row, budget| {
            budget.charge_work(1)?;
            Ok(row.0.cmp(&value))
        })
        .map_err(ProductionSourceOutputErrorV1::SourceOrigin);
        let index = self.remember(result)??;
        self.paid(1)?;
        Some(self.views[index].1)
    }
    fn ranked_operation(
        &mut self,
        location: (u32, u32),
    ) -> Option<&'ranked ProductionRankedOperationV1> {
        self.paid(3)?;
        let operation = self
            .lowering
            .kernel()
            .blocks()
            .get(location.0 as usize)?
            .operations()
            .get(location.1 as usize)?;
        // The common Load check compares the exact borrowed index lists.
        let indices = match operation {
            ProductionRankedOperationV1::Access { indices, .. }
            | ProductionRankedOperationV1::ValueAccess { indices, .. }
            | ProductionRankedOperationV1::AtomicAccess { indices, .. }
            | ProductionRankedOperationV1::AtomicValueAccess { indices, .. } => indices.len(),
            _ => 0,
        };
        self.paid(indices.checked_add(1)?)?;
        Some(operation)
    }
}

fn source_output_global_scratch_scope_v1<T>(
    budget: &mut AssertOriginBudgetV1<'_>,
    body: impl FnOnce(&mut AssertOriginBudgetV1<'_>) -> Result<T, ProductionSourceOutputErrorV1>,
) -> Result<T, ProductionSourceOutputErrorV1> {
    use ProductionSourceOutputErrorV1 as Error;
    let floor = budget.storage();
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| body(budget)));
    let cleanup = budget
        .storage()
        .checked_sub(floor)
        .ok_or(Error::Resource(AssertOriginResourceV1::Accounting))
        .and_then(|retained| budget.release_storage(retained).map_err(Error::Resource));
    match outcome {
        Ok(Err(error)) => Err(error),
        Ok(Ok(value)) => cleanup.map(|()| value),
        Err(payload) => {
            let _ = cleanup;
            std::panic::resume_unwind(payload)
        }
    }
}

fn source_output_ranked_sort_unique_v1<T>(
    values: &mut [T],
    budget: &mut AssertOriginBudgetV1<'_>,
    compare: impl Fn(&T, &T) -> std::cmp::Ordering,
) -> Result<(), ProductionSourceOutputErrorV1> {
    use ProductionSourceOutputErrorV1 as Error;
    assert_origin_sort_v1(values, budget, |left, right, budget| {
        budget.charge_work(4)?;
        Ok(compare(left, right))
    })
    .map_err(Error::SourceOrigin)?;
    for pair in values.windows(2) {
        budget.charge_work(4).map_err(Error::Resource)?;
        if compare(&pair[0], &pair[1]).is_eq() {
            return Err(Error::Invalid("duplicate global ranked correlation key"));
        }
    }
    Ok(())
}

impl ProductionSourceOutputOccurrencesV1<'_, '_> {
    /// Checks ordinary Global allocation and general write expressions for one
    /// freshly constructed ranked root. Original expressions are normalized at
    /// N; this view independently checked the actual O Store operand ancestry.
    /// The result is a narrow diagnostic, not whole-root validation, a formal
    /// discharge, a mandatory report, or final physical attachment authority.
    pub fn check_ranked_global_allocation_values(
        &self,
        owner: SemanticFunctionIdV1,
        function: SemanticFunctionIdV1,
        lowering: &ProductionRankedKernelLoweringInputV1,
        access_sources: &[ProductionRankedAccessSourceV1],
        budget: &mut AssertOriginBudgetV1<'_>,
    ) -> Result<(), ProductionSourceOutputErrorV1> {
        use ProductionSourceOutputErrorV1 as Error;
        budget.charge_work(4).map_err(Error::Resource)?;
        let minimum = self
            .source
            .executable_storage()
            .retained_storage()
            .checked_add(self.source.assert_origin_storage().payload_storage())
            .and_then(|n| n.checked_add(self.checked_output.storage().retained_storage()))
            .and_then(|n| n.checked_add(self.storage.retained_storage()))
            .ok_or(Error::Resource(AssertOriginResourceV1::Arithmetic))?;
        if budget.storage() < minimum {
            return Err(Error::Resource(AssertOriginResourceV1::Accounting));
        }
        source_output_global_scratch_scope_v1(budget, |budget| {
            budget.charge_work(4).map_err(Error::Resource)?;
            let declaration = self
                .source
                .semantic_ssa
                .source_semantic()
                .functions()
                .get(owner.index() as usize)
                .and_then(SemanticFunctionDeclV1::kernel_entry)
                .ok_or(Error::Invalid("global ranked source entry absent"))?;
            if !private_array_equal_bytes_v1(
                declaration.export_symbol().as_bytes(),
                lowering.kernel().function_name().as_bytes(),
                &mut PrivateArrayQueryWorkV1 { budget },
            )
            .map_err(Error::PrivateArray)?
            {
                return Err(Error::Invalid("global ranked source name changed"));
            }
            let origins = self.source.assert_origins();
            let alias = assert_origin_find_v1(&origins.origins.functions, budget, |row, budget| {
                budget.charge_work(2)?;
                Ok((row.owner, row.function).cmp(&(owner, function)))
            })
            .map_err(Error::SourceOrigin)?
            .ok_or(Error::Invalid("global ranked source function absent"))?;
            budget.charge_work(2).map_err(Error::Resource)?;
            let coordinate = origins.origins.functions[alias].canonical;
            let (inventory, storage) = fe2o3_kernel_analysis::CanonicalKirInventoryV1::derive(
                self.source.executable(),
                budget,
            )
            .map_err(Error::Inventory)?;
            budget
                .reserve_storage(storage.retained_storage())
                .map_err(Error::Resource)?;
            budget.charge_work(2).map_err(Error::Resource)?;
            let original_function = inventory
                .functions()
                .get(coordinate.0 as usize)
                .ok_or(Error::Invalid("global ranked original function absent"))?;
            if original_function.function.role != fe2o3_kernel_ir::FunctionRole::KernelEntry {
                return Err(Error::Invalid(
                    "global ranked nonentry function unsupported",
                ));
            }
            // All scratch lives inside this closure and drops before its floor
            // is released, including partial index construction and failed growth.
            let header =
                std::mem::size_of::<SourceOutputScalarNormalizationV1<'_, '_, '_, '_, '_>>()
                    .checked_sub(std::mem::size_of::<SourceOutputAllocationScratchV1>())
                    .ok_or(Error::Resource(AssertOriginResourceV1::Arithmetic))?;
            budget.reserve_storage(header).map_err(Error::Resource)?;
            let ssa = source_output_allocation_scratch_v1(&inventory, budget)?;
            let mut context = SourceOutputScalarNormalizationV1 {
                inventory: &inventory,
                function: original_function,
                lowering,
                sources: Vec::new(),
                locations: Vec::new(),
                sites: Vec::new(),
                views: Vec::new(),
                expressions: Vec::new(),
                nodes: Vec::new(),
                comparison: [(0, 0); 2 * MAX_PRODUCTION_SEMANTIC_EXPRESSION_DEPTH_V2 + 1],
                visiting: [None; MAX_PRODUCTION_SEMANTIC_EXPRESSION_DEPTH_V2 + 1],
                visiting_len: 0,
                normalization_steps: 0,
                ssa,
                error: None,
                budget,
            };
            source_output_ranked_global_index_v1(
                self,
                owner,
                function,
                access_sources,
                &mut context,
            )?;
            source_output_ranked_global_check_v1(self, owner, function, &mut context)
        })
    }
}

fn source_output_ranked_global_index_v1(
    view: &ProductionSourceOutputOccurrencesV1<'_, '_>,
    owner: SemanticFunctionIdV1,
    function: SemanticFunctionIdV1,
    access_sources: &[ProductionRankedAccessSourceV1],
    context: &mut SourceOutputScalarNormalizationV1<'_, '_, '_, '_, '_>,
) -> Result<(), ProductionSourceOutputErrorV1> {
    use ProductionSourceOutputErrorV1 as Error;
    for block in context.lowering.kernel().blocks() {
        context.budget.charge_work(1).map_err(Error::Resource)?;
        for operation in block.operations() {
            context.budget.charge_work(2).map_err(Error::Resource)?;
            if let Some(definition) = ranked_view_definition_v1(operation) {
                assert_origin_push_v1(&mut context.views, definition, context.budget)
                    .map_err(Error::SourceOrigin)?;
            }
            if let ProductionRankedOperationV1::SemanticExpression {
                result,
                expression,
                numerical_contract,
            } = operation
            {
                assert_origin_push_v1(
                    &mut context.expressions,
                    (*result, expression, *numerical_contract),
                    context.budget,
                )
                .map_err(Error::SourceOrigin)?;
            }
        }
    }
    for source in access_sources {
        context.budget.charge_work(6).map_err(Error::Resource)?;
        let operation = context
            .lowering
            .kernel()
            .blocks()
            .get(source.ranked_block as usize)
            .and_then(|block| block.operations().get(source.ranked_operation as usize))
            .ok_or(Error::Invalid("global ranked access location absent"))?;
        let (access, allocation, value, atomic) = ranked_access_descriptor_v1(operation)
            .ok_or(Error::Invalid("global ranked access descriptor absent"))?;
        let site = SemanticAccessSiteV1 {
            block: source.semantic_block,
            statement: source.semantic_statement,
            ordinal: source.semantic_access_ordinal,
        };
        let indexed = IndexedRankedAccessSourceV1 {
            ranked_block: source.ranked_block,
            ranked_operation: source.ranked_operation,
            access,
            allocation,
            value,
            atomic,
        };
        assert_origin_push_v1(&mut context.sources, (site, indexed), context.budget)
            .map_err(Error::SourceOrigin)?;
        assert_origin_push_v1(
            &mut context.locations,
            ((source.ranked_block, source.ranked_operation), site),
            context.budget,
        )
        .map_err(Error::SourceOrigin)?;
    }
    for span in &view.global_spans {
        context.budget.charge_work(3).map_err(Error::Resource)?;
        if span.key[0] != owner.index()
            || span.key[1] != function.index()
            || span.unsupported.is_some()
        {
            continue;
        }
        for ordinal in 0..span.count {
            context.budget.charge_work(5).map_err(Error::Resource)?;
            let row = view
                .global_accesses
                .get(
                    span.first
                        .checked_add(ordinal)
                        .ok_or(Error::Resource(AssertOriginResourceV1::Arithmetic))?,
                )
                .ok_or(Error::Invalid("global original site row absent"))?;
            let original = match row.placement {
                ProductionSourceOutputGlobalAccessV1::Retained { original, .. }
                | ProductionSourceOutputGlobalAccessV1::OmittedUnreachable { original } => original,
                ProductionSourceOutputGlobalAccessV1::Unsupported(_) => {
                    return Err(Error::Invalid("global original site unsupported"));
                }
            };
            let site = SemanticAccessSiteV1 {
                block: span.key[2],
                statement: (span.key[3] == 0).then_some(span.key[4]),
                ordinal: u32::try_from(ordinal)
                    .map_err(|_| Error::Resource(AssertOriginResourceV1::Arithmetic))?,
            };
            assert_origin_push_v1(&mut context.sites, (original, site), context.budget)
                .map_err(Error::SourceOrigin)?;
        }
    }
    source_output_ranked_sort_unique_v1(&mut context.sources, context.budget, |a, b| {
        a.0.cmp(&b.0)
    })?;
    source_output_ranked_sort_unique_v1(&mut context.locations, context.budget, |a, b| {
        a.0.cmp(&b.0)
    })?;
    source_output_ranked_sort_unique_v1(&mut context.sites, context.budget, |a, b| a.0.cmp(&b.0))?;
    source_output_ranked_sort_unique_v1(&mut context.views, context.budget, |a, b| a.0.cmp(&b.0))?;
    source_output_ranked_sort_unique_v1(&mut context.expressions, context.budget, |a, b| {
        a.0.cmp(&b.0)
    })?;
    let mut previous: Option<SemanticAccessSiteV1> = None;
    for (site, _) in &context.sources {
        context.budget.charge_work(4).map_err(Error::Resource)?;
        let expected = match previous {
            Some(last) if (last.block, last.statement) == (site.block, site.statement) => {
                last.ordinal.checked_add(1)
            }
            _ => Some(0),
        };
        if expected != Some(site.ordinal) {
            return Err(Error::Invalid(
                "global ranked source ordinals are not dense",
            ));
        }
        previous = Some(*site);
    }
    Ok(())
}

fn source_output_ranked_global_check_v1(
    view: &ProductionSourceOutputOccurrencesV1<'_, '_>,
    owner: SemanticFunctionIdV1,
    function: SemanticFunctionIdV1,
    context: &mut SourceOutputScalarNormalizationV1<'_, '_, '_, '_, '_>,
) -> Result<(), ProductionSourceOutputErrorV1> {
    use ProductionSourceOutputErrorV1 as Error;
    let mut ranked_global = 0usize;
    for ordinal in 0..context.sources.len() {
        context.budget.charge_work(3).map_err(Error::Resource)?;
        let (_, source) = context.sources[ordinal];
        let definition = match source.allocation {
            IndexedRankedAllocationV1::View(ProductionRankedValueV1::Local(value)) => context
                .ranked_view(value)
                .ok_or_else(|| context.failure())?,
            IndexedRankedAllocationV1::Direct(definition) => definition,
            _ => {
                return Err(Error::Invalid(
                    "global ranked allocation descriptor unsupported",
                ));
            }
        };
        if definition.memory_space != dialect_kernel::MemorySpaceAttr::Global {
            continue;
        }
        if !matches!(source.allocation, IndexedRankedAllocationV1::View(_)) {
            return Err(Error::Invalid(
                "global ranked direct allocation unsupported",
            ));
        }
        ranked_global = ranked_global
            .checked_add(1)
            .ok_or(Error::Resource(AssertOriginResourceV1::Arithmetic))?;
    }
    let mut checked_global = 0usize;
    // The sealed N/O Global rows are indexed once. A live row must have one
    // ranked claim; a wholly omitted row must have none. The final count also
    // rejects extra ranked Global accesses without another root-wide scan.
    for ordinal in 0..context.sites.len() {
        context.budget.charge_work(2).map_err(Error::Resource)?;
        let (expected_original, site) = context.sites[ordinal];
        let placement = view.global_access(
            owner,
            function,
            site.block,
            site.statement,
            site.ordinal,
            context.budget,
        )?;
        let source_index =
            assert_origin_find_v1(&context.sources, context.budget, |row, budget| {
                budget.charge_work(3)?;
                Ok(row.0.cmp(&site))
            })
            .map_err(Error::SourceOrigin)?;
        context.budget.charge_work(2).map_err(Error::Resource)?;
        if let ProductionSourceOutputGlobalAccessV1::OmittedUnreachable { original } = placement {
            if original != expected_original || source_index.is_some() {
                return Err(Error::Invalid(
                    "global ranked omitted source access claimed",
                ));
            }
            continue;
        }
        let source = context.sources
            [source_index.ok_or(Error::Invalid("global ranked live source access absent"))?]
        .1;
        let IndexedRankedAllocationV1::View(ProductionRankedValueV1::Local(value)) =
            source.allocation
        else {
            return Err(Error::Invalid(
                "global ranked allocation descriptor unsupported",
            ));
        };
        let definition = context
            .ranked_view(value)
            .ok_or_else(|| context.failure())?;
        if definition.memory_space != dialect_kernel::MemorySpaceAttr::Global {
            return Err(Error::Invalid("global ranked memory space changed"));
        }
        context.budget.charge_work(7).map_err(Error::Resource)?;
        let ProductionSourceOutputGlobalAccessV1::Retained {
            original,
            operation,
            source_argument,
            value: output_value,
            executable: true,
            ..
        } = placement
        else {
            return Err(Error::Invalid(
                "global ranked correlation requires an executable retained access",
            ));
        };
        if original != expected_original
            || original.block.function != context.function.coordinate
            || definition.allocation_origin != u64::from(source_argument) + 1
        {
            return Err(Error::Invalid("global ranked allocation origin mismatch"));
        }
        let output = source_output_operation_v1(view.output(), operation, context.budget)?;
        let original =
            source_output_operation_v1(context.inventory.owner(), original, context.budget)?;
        context.budget.charge_work(5).map_err(Error::Resource)?;
        match (
            &original.kind,
            &output.kind,
            source.access,
            source.value,
            output_value,
        ) {
            (
                OperationKind::Load { .. },
                OperationKind::Load { .. },
                dialect_kernel::AccessKindAttr::Read,
                None,
                None,
            ) if source.atomic.is_none() => {}
            (
                OperationKind::Store { value, .. },
                OperationKind::Store { .. },
                dialect_kernel::AccessKindAttr::Write,
                Some(ProductionRankedValueV1::Local(ranked_value)),
                Some(_),
            ) if source.atomic.is_none() => {
                // Capacity is retained between stores, but tree IDs never cross
                // a pair. Shared SSA subexpressions keep legacy expansion rules.
                context.budget.charge_work(1).map_err(Error::Resource)?;
                context.nodes.clear();
                let expression =
                    assert_origin_find_v1(&context.expressions, context.budget, |row, budget| {
                        budget.charge_work(1)?;
                        Ok(row.0.cmp(&ranked_value))
                    })
                    .map_err(Error::SourceOrigin)?
                    .ok_or(Error::Invalid("global ranked write expression absent"))?;
                context.budget.charge_work(2).map_err(Error::Resource)?;
                let (_, expression, contract) = context.expressions[expression];
                let start = context.normalization_steps;
                let expected = normalize_ranked_expression_core_v1(expression, 0, context)
                    .ok_or_else(|| context.failure())?;
                let steps = context
                    .normalization_steps
                    .checked_sub(start)
                    .ok_or(Error::Resource(AssertOriginResourceV1::Accounting))?;
                // Successful normalization has already bounded every expression
                // node and its depth. Pay for the existing contract traversal.
                context
                    .budget
                    .charge_work(
                        steps
                            .checked_add(1)
                            .ok_or(Error::Resource(AssertOriginResourceV1::Arithmetic))?,
                    )
                    .map_err(Error::Resource)?;
                if contract != ProductionNumericalContractV2::exact_for_expression(expression) {
                    return Err(Error::Invalid("global ranked numerical contract mismatch"));
                }
                let actual = normalize_kir_expression_core_v1(*value, 0, context)
                    .ok_or_else(|| context.failure())?;
                if !context
                    .equivalent(actual, expected)
                    .ok_or_else(|| context.failure())?
                {
                    return Err(Error::Invalid("global ranked write value mismatch"));
                }
            }
            _ => {
                return Err(Error::Invalid(
                    "global ranked access or write value contract mismatch",
                ));
            }
        }
        checked_global = checked_global
            .checked_add(1)
            .ok_or(Error::Resource(AssertOriginResourceV1::Arithmetic))?;
    }
    // This is completeness only for the already checked ordinary Global rows,
    // not a census of unrelated O effects or whole-root coverage authority.
    context.budget.charge_work(1).map_err(Error::Resource)?;
    if checked_global != ranked_global {
        return Err(Error::Invalid("global ranked extra access unsupported"));
    }
    Ok(())
}
