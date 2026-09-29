#[derive(Clone, Copy)]
struct OptimizedSourceScalarReadV18 {
    value: ValueId,
    original: usize,
    operation: fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1,
}

#[cfg(test)]
thread_local! {
    static OPTIMIZED_SCALAR_ATTEMPT_PROBE_V18: std::cell::Cell<Option<(usize, usize, usize, usize)>> = const { std::cell::Cell::new(None) };
}

#[cfg(test)]
fn test_optimized_scalar_attempt_header_v18(budget: &mut ArgumentBudgetV1<'_>) {
    OPTIMIZED_SCALAR_ATTEMPT_PROBE_V18.with(|probe| {
        if let Some((remaining, _, _, calls)) = probe.get() {
            assert_eq!(calls, 0, "the probe must reach one optimized inner attempt");
            let floor = budget.storage();
            let padding = budget.storage_limit() - floor - remaining;
            budget.reserve_storage(padding).unwrap();
            probe.set(Some((remaining, padding, floor, calls + 1)));
        }
    });
}

/// The original scalar names plus exact optimized read occurrences. This is not
/// a read-from/currentness proof; those obligations remain independent.
pub struct ProductionOptimizedSourceScalarLeavesV18<'scope> {
    original: &'scope ProductionSourceScalarLeavesV18<'scope>,
    optimized: &'scope ProductionOptimizedSourceCorrespondenceV18<'scope>,
    function: &'scope fe2o3_kernel_analysis::CanonicalKirFunctionRefV1<'scope>,
    reads: &'scope [OptimizedSourceScalarReadV18],
    wrapping: &'scope [SourceWrappingValueV23],
    slot: usize,
    ledger: fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
    floor: usize,
}

struct OptimizedSourceScalarNormalizationV18<'a> {
    leaves: &'a ProductionOptimizedSourceScalarLeavesV18<'a>,
    arguments: &'a SourceRootArgumentsV18<'a, 'a>,
}

/// A retained original Store paired with its actual optimized RHS use. The
/// original request still checks the same original function/place/value IDs.
pub struct ProductionOptimizedSourceScalarStoreV18<'scope> {
    original: &'scope ProductionSourceScalarStoreV18<'scope>,
    leaves: &'scope ProductionOptimizedSourceScalarLeavesV18<'scope>,
    origins: &'scope value_origin_v1::WholeValueOriginsV18<'scope>,
    inline_scalar: &'scope Gfx942InlineScalarCorrespondenceV30<'scope>,
    output: fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1,
    value: ValueId,
}

/// Every original Store has an explicit disposition. Removed source control
/// does not mint a completed memory obligation or an ordinary proof receipt.
pub enum ProductionOptimizedSourceScalarStoreDispositionV18<'scope> {
    /// The ordered Store survives with its exact output occurrence and RHS use.
    /// Expression and currentness checks remain separate obligations.
    Retained(ProductionOptimizedSourceScalarStoreV18<'scope>),
    /// The checked transition certifies this original Store is unreachable.
    /// Absence alone, a scalar rewrite, or a missing mapping cannot select it.
    RemovedUnreachable {
        /// Original input-graph occurrence, without renumbering its source site.
        input: fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1,
    },
}

fn optimized_source_scalar_reads_v18(
    original: &ProductionSourceScalarLeavesV18<'_>,
    optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<Vec<OptimizedSourceScalarReadV18>> {
    use fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1 as Definition;
    let leaves = original.leaves;
    let relation = leaves.relation;
    relation.retain_query((|| {
        optimized_source_endpoints_v18(relation, optimized, budget)?;
        let input = relation.source.root(leaves.root, budget)?.1;
        let function = relation.inventory.functions().get(input).ok_or(
            ProductionSourceOwnedViewErrorV18::Binding("optimized scalar original function"),
        )?;
        let output = optimized.output_inventory(budget)?;
        let mut rows =
            emission_vec_v1(leaves.rows.len(), budget).map_err(source_emission_error_v18)?;
        for (index, source) in leaves.rows.iter().enumerate() {
            budget.charge_work(2)?;
            let expected = NormalizedScalarExpressionV1::Symbol {
                symbol: source.symbol,
                scalar: source.scalar,
            };
            if leaves.actual_value(function.function, source.value, budget)? != Some(expected) {
                return relation
                    .source
                    .missing("optimized scalar changed original read name");
            }
            let actual = match optimized.operation(source.operation, budget)? {
                ProductionOptimizedSourceOperationV18::Retained { output, .. } => output,
                ProductionOptimizedSourceOperationV18::RemovedUnreachable { .. } => continue,
                ProductionOptimizedSourceOperationV18::Rewritten { .. } => {
                    return relation
                        .source
                        .missing("optimized scalar erased a reachable read");
                }
            };
            let operation = source_operation_row_v18(output, actual, budget)?;
            if !(if source.typed_private {
                source_scalar_read_kind_v22(&operation.operation.kind, true)
            } else {
                matches!(operation.operation.kind, OperationKind::Load { .. })
            }) {
                return relation
                    .source
                    .missing("optimized scalar source read changed operation kind");
            }
            let [result] = operation.operation.results.as_slice() else {
                return relation
                    .source
                    .missing("optimized scalar read result census");
            };
            if kir_semantic_scalar_v1(&result.ty) != Some(source.scalar) {
                return relation
                    .source
                    .missing("optimized scalar read result type changed");
            }
            let mut found = 0usize;
            for descendant in optimized.definition_descendants(
                Definition::Result {
                    operation: source.operation,
                    result: 0,
                },
                budget,
            )? {
                budget.charge_work(1)?;
                if descendant.output
                    == (Definition::Result {
                        operation: actual,
                        result: 0,
                    })
                {
                    found = found.checked_add(1).ok_or(ArgumentResourceV1::Arithmetic)?;
                }
            }
            if found != 1 {
                return relation
                    .source
                    .missing("optimized scalar read result is not its exact descendant");
            }
            if rows.len() == rows.capacity() {
                return relation.source.missing("optimized scalar read capacity");
            }
            rows.push(OptimizedSourceScalarReadV18 {
                value: result.id,
                original: index,
                operation: actual,
            });
        }
        private_array_heapsort_v1(
            &mut rows,
            |row| [row.value.0 as usize],
            &mut SourceCorrespondenceWorkV18(budget),
            || ArgumentResourceV1::Arithmetic.into(),
        )?;
        for pair in rows.windows(2) {
            budget.charge_work(1)?;
            if pair[0].value == pair[1].value {
                return relation
                    .source
                    .missing("optimized scalar merges original read occurrences");
            }
        }
        Ok(rows)
    })())
}

impl ProductionSourceCorrespondenceV18<'_> {
    /// Lends original scalar names together with their exact optimized Load
    /// occurrences for one authenticated root. The callback cannot retain the
    /// borrowed names after their paid source/output scope ends. This does not
    /// prove which Store any Load reads or discharge typed-memory obligations.
    pub fn with_optimized_scalar_leaves_v18<'work, T, E>(
        &self,
        optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
        root: usize,
        recipe: &fe2o3_pliron::ProductionRankedKernelV1,
        budget: &mut ArgumentBudgetV1<'work>,
        consume: impl for<'scope> FnOnce(
            &ProductionOptimizedSourceScalarLeavesV18<'scope>,
            &mut ArgumentBudgetV1<'work>,
        ) -> Result<T, E>,
    ) -> Result<T, E>
    where
        E: From<ProductionSourceOwnedViewErrorV18>,
    {
        self.with_optimized_scalar_leaf_namespace_v18(
            optimized,
            root,
            &SourceScalarNamespaceV18::Ranked(recipe),
            budget,
            consume,
        )
    }

    /// Opens the same checked source/output scalar scope without reserving
    /// names from a ranked recipe. All root, Load and use bindings remain
    /// mandatory; typed-memory and ranked equivalence are not granted here.
    pub fn with_optimized_source_scalar_leaves_v18<'work, T, E>(
        &self,
        optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
        root: usize,
        budget: &mut ArgumentBudgetV1<'work>,
        consume: impl for<'scope> FnOnce(
            &ProductionOptimizedSourceScalarLeavesV18<'scope>,
            &mut ArgumentBudgetV1<'work>,
        ) -> Result<T, E>,
    ) -> Result<T, E>
    where
        E: From<ProductionSourceOwnedViewErrorV18>,
    {
        self.with_optimized_scalar_leaf_namespace_v18(
            optimized,
            root,
            &SourceScalarNamespaceV18::SourceOnly,
            budget,
            consume,
        )
    }

    fn with_optimized_scalar_leaf_namespace_v18<'work, T, E>(
        &self,
        optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
        root: usize,
        namespace: &SourceScalarNamespaceV18<'_>,
        budget: &mut ArgumentBudgetV1<'work>,
        consume: impl for<'scope> FnOnce(
            &ProductionOptimizedSourceScalarLeavesV18<'scope>,
            &mut ArgumentBudgetV1<'work>,
        ) -> Result<T, E>,
    ) -> Result<T, E>
    where
        E: From<ProductionSourceOwnedViewErrorV18>,
    {
        let mut consume = SourceCallbackCustodyV29::new(consume);
        optimized_source_endpoints_v18(self, optimized, budget)?;
        self.with_scalar_leaf_namespace_v18(root, namespace, budget, move |original, budget| {
            #[cfg(test)]
            test_optimized_scalar_attempt_header_v18(budget);
            let floor = budget.storage();
            let (reads, wrapping, function, retained) = self.retain_query(
                scoped_source_attempt_v29(self.source.cleanup, budget, floor, |budget| {
                    let floor = budget.storage();
                    self.retain_query((|| {
                        budget.reserve_storage(argument_sum_v1(&[
                            size_of::<ProductionOptimizedSourceScalarLeavesV18<'_>>(),
                            size_of::<Vec<OptimizedSourceScalarReadV18>>(),
                            size_of::<Vec<SourceWrappingValueV23>>(),
                            size_of::<SourceWrappingValueV23>(),
                            size_of::<std::thread::Result<Result<T, E>>>(),
                            source_reference_cleanup_headers_v29()?,
                        ])?)?;
                        budget.charge_work(1 + SOURCE_REFERENCE_PAYLOAD_ATTEMPTS_V29)?;
                        let function =
                            optimized_source_root_function_v18(self, optimized, root, budget)?;
                        let reads = optimized_source_scalar_reads_v18(original, optimized, budget)?;
                        let wrapping =
                            optimized_source_wrapping_values_v23(original, optimized, budget)?;
                        let retained = budget
                            .storage()
                            .checked_sub(floor)
                            .ok_or(ArgumentResourceV1::Accounting)?;
                        Ok((reads, wrapping, function, retained))
                    })())
                }),
            )?;
            let leaves = ProductionOptimizedSourceScalarLeavesV18 {
                original,
                optimized,
                function,
                reads: &reads,
                wrapping: &wrapping,
                slot: std::ptr::from_ref(budget) as usize,
                ledger: budget.work_ledger_identity_v1(),
                floor: budget.storage(),
            };
            let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                consume
                    .take()
                    .expect("optimized leaf consumer is invoked once")(
                    &leaves, budget
                )
            }));
            drop(consume);
            let prior = self.source.guard.first.get();
            let postflight = if matches!(&caught, Ok(Ok(_))) {
                leaves.check(budget)
            } else {
                leaves.observe_custody(budget)
            };
            drop(leaves);
            drop(reads);
            drop(wrapping);
            let released = if self.source.cleanup.is_denied() {
                Err(ArgumentResourceV1::Accounting)
            } else {
                budget
                    .release_storage(retained)
                    .inspect_err(|_| self.source.cleanup.deny_refund())
            };
            match caught {
                Err(payload) => std::panic::resume_unwind(payload),
                Ok(Err(error)) => {
                    let selected = match prior {
                        Some(first) => {
                            source_reference_discard_v29(Err::<T, E>(error));
                            first.error().into()
                        }
                        None => error,
                    };
                    Err(selected)
                }
                Ok(Ok(value)) => {
                    match self.retain_query(postflight.and(released.map_err(Into::into))) {
                        Ok(()) => Ok(value),
                        Err(error) => {
                            source_reference_discard_v29(Ok::<T, E>(value));
                            Err(error.into())
                        }
                    }
                }
            }
        })
    }
}

impl ProductionOptimizedSourceScalarLeavesV18<'_> {
    fn observe_custody(&self, budget: &ArgumentBudgetV1<'_>) -> SourceOwnedResultV18<()> {
        if self.slot != std::ptr::from_ref(budget) as usize
            || self.ledger != budget.work_ledger_identity_v1()
            || budget.storage() < self.floor
        {
            self.original.deny_refund();
            return Err(ArgumentResourceV1::Accounting.into());
        }
        self.original.observe_custody(budget)
    }

    fn check(&self, budget: &mut ArgumentBudgetV1<'_>) -> SourceOwnedResultV18<()> {
        let relation = self.original.leaves.relation;
        relation.retain_query(self.observe_custody(budget))?;
        self.original.check(budget)?;
        optimized_source_endpoints_v18(relation, self.optimized, budget)
    }

    /// Borrows the unchanged original scalar-name view after checking this
    /// optimized view's source association, retained storage, and budget custody.
    pub fn original_leaves(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<&ProductionSourceScalarLeavesV18<'_>> {
        self.check(budget)?;
        Ok(self.original)
    }

    /// Visits each original scalar Store with an explicit retained or checked
    /// unreachable disposition and returns the full number visited. Retained
    /// requests name the actual output RHS use, not a chosen first descendant.
    /// A successful visit count is not a completed memory or ranked proof.
    pub fn visit_store_inputs<'work, E>(
        &self,
        budget: &mut ArgumentBudgetV1<'work>,
        consume: impl for<'request> FnMut(
            &ProductionOptimizedSourceScalarStoreDispositionV18<'request>,
            &mut ArgumentBudgetV1<'work>,
        ) -> Result<(), E>,
    ) -> Result<usize, E>
    where
        E: From<ProductionSourceOwnedViewErrorV18>,
    {
        let consume = SourceCallbackCustodyV29::new(consume);
        self.check(budget)?;
        let relation = self.original.leaves.relation;
        let root = self.original.leaves.root;
        let inventory = self.optimized.output_inventory(budget)?;
        let floor = budget.storage();
        let capture = std::mem::size_of_val(&consume);
        let alignment = std::mem::align_of_val(&consume);
        let prepared =
            scoped_source_attempt_v29(relation.source.cleanup, budget, floor, |budget| {
                consume.prepare(|| {
                    let headers = argument_sum_v1(&[
                        capture,
                        alignment,
                        // Both the inner result and its outer forwarding boundary
                        // can reject a consumer error, including inline=None.
                        source_callback_custody_finish_preflight_v29::<usize, E>(budget)?,
                        source_owned_finish_preflight_v26::<usize, E>(budget)?,
                        source_owned_finish_preflight_v26::<usize, E>(budget)?,
                        size_of::<ProductionOptimizedSourceScalarStoreDispositionV18<'_>>(),
                        size_of::<OptimizedSourceScalarNormalizationV18<'_>>(),
                        size_of::<Option<Gfx942InlineScalarCorrespondenceV30<'_>>>(),
                        size_of::<usize>(),
                    ])?;
                    relation.retain_query(budget.reserve_storage(headers).map_err(Into::into))?;
                    Ok::<_, ProductionSourceOwnedViewErrorV18>(headers)
                })
            });
        let (headers, mut consume) = match prepared {
            Ok(headers) => headers,
            Err(error) => {
                return Err(error.into());
            }
        };
        let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            value_origin_v1::with_optimized_whole_value_origins_v18(
                relation,
                self.optimized,
                inventory,
                self.function.coordinate,
                budget,
                |origins, budget| {
                    let mut inline = None;
                    let mut inline_storage = 0usize;
                    let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        self.original.visit_store_inputs(budget, |request, budget| {
                        self.check(budget)?;
                        request.check(budget)?;
                        let output = match self.optimized.operation(request.operation, budget)? {
                            ProductionOptimizedSourceOperationV18::Retained { output, .. } => output,
                            ProductionOptimizedSourceOperationV18::RemovedUnreachable { .. } => {
                                return consume.as_mut().expect("scalar visitor remains in custody")(&ProductionOptimizedSourceScalarStoreDispositionV18::RemovedUnreachable {
                                    input: request.operation,
                                }, budget);
                            }
                            ProductionOptimizedSourceOperationV18::Rewritten { .. } => {
                                return relation.source.missing("optimized erased reachable source Store").map_err(Into::into);
                            }
                        };
                        let (_, value) = optimized_source_actual_operand_v18(relation, self.optimized,
                            fe2o3_kernel_ir::CanonicalKirUseCoordinateV1::OperationOperand {
                                operation: request.operation, operand: 1,
                            }, output, budget)?;
                        let actual = source_operation_row_v18(inventory, output, budget)?;
                        if !matches!(&actual.operation.kind, OperationKind::Store { value: actual, .. } if *actual == value)
                            || output.block.function != self.function.coordinate
                        {
                            return relation.source.missing("optimized source Store actual RHS or root changed").map_err(Into::into);
                        }
                        if inline.is_none() {
                            let before = budget.storage();
                            inline = Some(scoped_source_attempt_v29(relation.source.cleanup, budget, before, |budget| {
                                request.inline_scalar.transport_optimized_source_v18(relation, self.optimized, root, budget)
                            })?);
                            inline_storage = relation.retain_query(budget.storage().checked_sub(before)
                                .ok_or(ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Accounting)))?;
                        }
                        let request = ProductionOptimizedSourceScalarStoreV18 {
                            original: request, leaves: self, origins,
                            inline_scalar: inline.as_ref().expect("installed optimized inline correspondence"),
                            output, value,
                        };
                        consume.as_mut().expect("scalar visitor remains in custody")(&ProductionOptimizedSourceScalarStoreDispositionV18::Retained(request), budget)
                    })
                    }));
                    let prior = relation.source.guard.first.get();
                    let postflight = self.observe_custody(budget);
                    drop(inline);
                    source_owned_finish_callback_v18(
                        caught,
                        prior,
                        postflight,
                        relation.source.cleanup,
                        budget,
                        inline_storage,
                    )
                },
            )
        }));
        let caught = consume.finish(caught);
        let prior = relation.source.guard.first.get();
        let postflight = if matches!(&caught, Ok(Ok(_))) {
            self.check(budget)
        } else {
            self.observe_custody(budget)
        };
        source_owned_finish_callback_v18(
            caught,
            prior,
            postflight,
            relation.source.cleanup,
            budget,
            headers,
        )
    }

    fn read(
        &self,
        function: &Function,
        value: ValueId,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<Option<NormalizedScalarExpressionV1>> {
        let relation = self.original.leaves.relation;
        relation.retain_query((|| {
            self.check(budget)?;
            budget.charge_work(1)?;
            if !std::ptr::eq(function, self.function.function) {
                return relation
                    .source
                    .missing("optimized scalar read foreign output function");
            }
            let mut first = 0usize;
            let mut end = self.reads.len();
            while first < end {
                budget.charge_work(2)?;
                let middle = first + (end - first) / 2;
                if self.reads[middle].value < value {
                    first = middle + 1;
                } else {
                    end = middle;
                }
            }
            budget.charge_work(2)?;
            let Some(row) = self.reads.get(first).filter(|row| row.value == value) else {
                return Ok(None);
            };
            let source = self.original.leaves.rows.get(row.original).ok_or(
                ProductionSourceOwnedViewErrorV18::Binding("optimized scalar read original row"),
            )?;
            let actual = source_operation_row_v18(
                self.optimized.output_inventory(budget)?,
                row.operation,
                budget,
            )?;
            if actual
                .operation
                .results
                .first()
                .is_none_or(|result| result.id != value)
            {
                return relation
                    .source
                    .missing("optimized scalar read actual output value changed");
            }
            Ok(Some(NormalizedScalarExpressionV1::Symbol {
                symbol: source.symbol,
                scalar: source.scalar,
            }))
        })())
    }
}

impl SourceScalarNormalizationV18 for OptimizedSourceScalarNormalizationV18<'_> {
    fn binary_overflow(
        &self,
        function: &Function,
        value: ValueId,
        overflow: ProductionOverflowContractV2,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<ProductionOverflowContractV2> {
        if overflow != ProductionOverflowContractV2::Checked
            || !self.leaves.original.leaves.ordinary_values
        {
            return Ok(overflow);
        }
        let relation = self.leaves.original.leaves.relation;
        relation.retain_query((|| {
            self.leaves.check(budget)?;
            budget.charge_work(1)?;
            if !std::ptr::eq(function, self.leaves.function.function) {
                return relation
                    .source
                    .missing("wrapping scalar substituted output function");
            }
            let Some(row) = source_wrapping_find_v23(self.leaves.wrapping, value, budget)? else {
                return Ok(overflow);
            };
            let output = self.leaves.optimized.output_inventory(budget)?;
            let actual = source_operation_row_v18(output, row.operation, budget)?;
            budget.charge_work(7)?;
            if row.operation.block.function != self.leaves.function.coordinate
                || source_wrapping_result_v23(actual.operation, row.operator, row.scalar)
                    != Some(value)
            {
                return relation
                    .source
                    .missing("wrapping scalar output occurrence differs");
            }
            Ok(ProductionOverflowContractV2::Wrapping)
        })())
    }

    fn leaf(
        &self,
        function: &Function,
        value: ValueId,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<Option<NormalizedScalarExpressionV1>> {
        self.leaves.read(function, value, budget)
    }

    fn argument(
        &self,
        function: &Function,
        argument: u32,
        scalar: ProductionSemanticScalarTypeV2,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<NormalizedScalarExpressionV1> {
        use fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1 as Definition;
        let leaves = self.leaves.original.leaves;
        let relation = leaves.relation;
        relation.retain_query((|| {
            self.leaves.check(budget)?;
            budget.charge_work(4)?;
            if !std::ptr::eq(function, self.leaves.function.function)
                || !std::ptr::eq(relation, self.arguments.relation)
                || leaves.root != self.arguments.root
            {
                return relation
                    .source
                    .missing("optimized scalar argument changed source or output function");
            }
            let slot = argument as usize;
            if function
                .signature
                .parameters
                .get(slot)
                .and_then(kir_semantic_scalar_v1)
                != Some(scalar)
            {
                return relation
                    .source
                    .missing("optimized scalar argument type changed");
            }
            let input = relation.source.root(leaves.root, budget)?.1;
            let original = relation.inventory.functions().get(input).ok_or(
                ProductionSourceOwnedViewErrorV18::Binding(
                    "optimized scalar original parameter function",
                ),
            )?;
            let source_argument =
                self.arguments
                    .original_argument(original.function, argument, budget)?;
            let expected = Definition::FunctionArgument {
                function: self.leaves.function.coordinate,
                argument,
            };
            let mut found = 0usize;
            for row in self.leaves.optimized.definition_descendants(
                Definition::FunctionArgument {
                    function: original.coordinate,
                    argument,
                },
                budget,
            )? {
                budget.charge_work(1)?;
                if row.output == expected {
                    found = found.checked_add(1).ok_or(ArgumentResourceV1::Arithmetic)?;
                }
            }
            let definition = optimized_source_definition_row_v18(
                self.leaves.optimized.output_inventory(budget)?,
                expected,
                budget,
            )?;
            if found != 1
                || function
                    .body
                    .as_ref()
                    .and_then(|body| body.parameters.get(slot))
                    .copied()
                    != definition.value
            {
                return relation
                    .source
                    .missing("optimized scalar argument changed actual output slot or value");
            }
            let symbol = PRODUCTION_KERNEL_SCALAR_SYMBOL_BASE_V2
                .checked_add(source_argument)
                .filter(|symbol| *symbol < fe2o3_pliron::PRODUCTION_SEMANTIC_LOAD_SYMBOL_BASE_V2)
                .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                    "optimized scalar argument namespace",
                ))?;
            Ok(NormalizedScalarExpressionV1::Symbol { symbol, scalar })
        })())
    }
}

impl ProductionOptimizedSourceScalarStoreV18<'_> {
    /// Returns the original invocation ID and borrowed original function, never
    /// an output-graph function substituted into the source identity namespace.
    pub fn original(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<(usize, &SemanticFunctionDeclV1)> {
        self.leaves.check(budget)?;
        self.original.original(budget)
    }
    /// Lends the retained Store's original source operand for the exact
    /// original function pointer. Same-shaped reconstructed functions refuse.
    pub fn input_for<'function>(
        &self,
        function: &'function SemanticFunctionDeclV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<ProductionSourceScalarInputV18<'function>> {
        self.leaves.check(budget)?;
        self.original.input_for(function, budget)
    }
    /// Returns the retained source operand's scalar type after custody checks;
    /// this type alone confers no value-equivalence or memory permission.
    pub fn scalar(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<ProductionSemanticScalarTypeV2> {
        self.leaves.check(budget)?;
        self.original.scalar(budget)
    }
    /// Compares a proposed scalar expression with the exact optimized Store
    /// RHS in the retained original argument/read namespace, using the shared
    /// bounded normalizer. The caller must construct that proposal from the
    /// authenticated source input; read-from and currentness remain unproved.
    pub fn check_expression(
        &self,
        expression: &ProductionSemanticExpressionV2,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<()> {
        let relation = self.original.leaves.relation;
        relation.retain_query((|| {
            self.leaves.check(budget)?;
            self.original.check(budget)?;
            if expression.scalar() != self.original.scalar {
                return relation
                    .source
                    .missing("optimized Store expression scalar type changed");
            }
            let (_, value) = optimized_source_actual_operand_v18(
                relation,
                self.leaves.optimized,
                fe2o3_kernel_ir::CanonicalKirUseCoordinateV1::OperationOperand {
                    operation: self.original.operation,
                    operand: 1,
                },
                self.output,
                budget,
            )?;
            if value != self.value {
                return relation
                    .source
                    .missing("optimized Store RHS changed after request");
            }
            let normalizer = OptimizedSourceScalarNormalizationV18 {
                leaves: self.leaves,
                arguments: self.original.arguments,
            };
            optimized_source_scalar_expression_endpoint_v18(
                self.original.leaves,
                self.leaves.optimized,
                self.leaves.optimized.output_inventory(budget)?,
                self.leaves.function.coordinate,
                &normalizer,
                self.origins,
                self.inline_scalar,
                expression,
                value,
                budget,
            )
        })())
    }
}

fn optimized_source_scalar_expression_endpoint_v18(
    leaves: &SourceScalarLeavesV18<'_, '_>,
    optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
    inventory: &fe2o3_kernel_analysis::CanonicalKirInventoryV18<'_>,
    output_function: fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1,
    normalizer: &dyn SourceScalarNormalizationV18,
    origins: &value_origin_v1::WholeValueOriginsV18<'_>,
    inline_scalar: &Gfx942InlineScalarCorrespondenceV30<'_>,
    expression: &ProductionSemanticExpressionV2,
    value: ValueId,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<()> {
    let relation = leaves.relation;
    relation.retain_query((|| {
        leaves.query(budget)?;
        optimized_source_endpoints_v18(relation, optimized, budget)?;
        let function =
            optimized_source_root_function_v18(relation, optimized, leaves.root, budget)?;
        if !std::ptr::eq(inventory, optimized.output_inventory(budget)?)
            || function.coordinate != output_function
        {
            return relation
                .source
                .missing("optimized scalar normalization exact output root");
        }
        let work_limit = relation
            .source
            .limits(budget)?
            .max_operations
            .checked_mul(UNSUPPORTED_INDEX_CORRELATION_STEPS_PER_OPERATION_V1)
            .ok_or(ArgumentResourceV1::Arithmetic)?;
        let run = |budget: &mut ArgumentBudgetV1<'_>| {
            budget.charge_work(MAX_PRODUCTION_SEMANTIC_EXPRESSION_DEPTH_V2 + 1)?;
            let ledger = CorrelationLedgerV18::new(budget, relation.source.cleanup);
            let graph = InventoryCorrelationV18 {
                origins: Some(origins),
                inventory,
                function: output_function,
                ledger: &ledger,
                inline_scalar,
                scalar_source: Some(normalizer),
            };
            let expected_leaves = SourceExpressionLeavesV18 {
                leaves,
                ledger: &ledger,
                remaining: std::cell::Cell::new(
                    fe2o3_pliron::MAX_PRODUCTION_SEMANTIC_EXPRESSION_NODES_V2,
                ),
            };
            let mut charge = SourceTranslationChargeV18::new(&ledger, work_limit)?;
            let mut visiting = SourceScalarVisitingV18 {
                rows: [None; MAX_PRODUCTION_SEMANTIC_EXPRESSION_DEPTH_V2 + 1],
                length: 0,
            };
            let no_ranked_reads = BTreeMap::new();
            let mut expected = charge.begin_normalized_tree().and_then(|()| {
                normalize_semantic_expression_v18(expression, &expected_leaves, 0, &mut charge)
            });
            let expected_nodes = charge.remaining_nodes.and_then(|remaining| {
                fe2o3_pliron::MAX_PRODUCTION_SEMANTIC_EXPRESSION_NODES_V2.checked_sub(remaining)
            });
            let mut actual = if expected.is_some() && charge.begin_normalized_tree().is_some() {
                native_helper_value_expansion_v1::with_source_value_expansion_v18(
                    &ledger,
                    |helpers| {
                        normalize_kir_expression_with_visiting_v18(
                            function.function,
                            &graph,
                            &no_ranked_reads,
                            value,
                            0,
                            &mut visiting,
                            &mut charge,
                            helpers,
                        )
                        .ok_or(ProductionMirPlironTranslationErrorV1::ResourceLimit)
                    },
                )
                .ok()
            } else {
                None
            };
            let constants = expected
                .as_mut()
                .zip(actual.as_mut())
                .and_then(|(expected, actual)| {
                    source_scalar_constant_fold_v18(expected, 0, &mut charge)?;
                    source_scalar_constant_fold_v18(actual, 0, &mut charge)?;
                    if leaves.ordinary_values {
                        source_private_integer_identity_v22(expected, 0, &mut charge)?;
                        source_private_integer_identity_v22(actual, 0, &mut charge)?;
                    }
                    Some(())
                })
                .is_some();
            let equality_work =
                expected_nodes
                    .zip(charge.remaining_nodes)
                    .and_then(|(expected, remaining)| {
                        fe2o3_pliron::MAX_PRODUCTION_SEMANTIC_EXPRESSION_NODES_V2
                            .checked_sub(remaining)?
                            .checked_add(expected)
                    });
            let equality_paid = constants
                && equality_work
                    .and_then(|work| charge.charge_many(work))
                    .is_some();
            let equal = equality_paid && expected == actual && visiting.length == 0;
            // The pair can share input DAG nodes, but never heap ownership.
            // Credits remain reserved until both expanded trees are gone.
            drop((actual, expected));
            let finished = charge.finish_normalized_pair().is_some();
            let resource = ledger.failure.get();
            let refused = ledger.inconsistent_inventory.get() || charge.charge.finite_denied;
            let error = charge.error.take();
            drop((no_ranked_reads, visiting, charge, expected_leaves, graph));
            drop(ledger);
            if let Some(error) = resource {
                return Err(error.into());
            }
            if let Some(error) = error {
                return Err(error);
            }
            if refused || !finished || !equal {
                return relation.source.missing(
                    "actual optimized scalar expression differs from its original source value",
                );
            }
            optimized_source_endpoints_v18(relation, optimized, budget)
        };
        let storage = argument_sum_v1(&[
            size_of::<CorrelationLedgerV18<'_, '_, '_>>(),
            size_of::<SourceScalarVisitingV18>(),
            size_of::<SourceExpressionLeavesV18<'_, '_, '_, '_, '_, '_>>(),
            size_of::<InventoryCorrelationV18<'_, '_, '_, '_, '_, '_>>(),
            size_of::<BTreeMap<(FunctionOperationLocation, u32), SemanticAccessSiteV1>>(),
            source_scalar_constant_fold_headers_v18()?,
            source_scalar_overflow_query_headers_v23()?,
        ])?;
        source_scalar_normalization_scratch_v18(relation.source.cleanup, budget, storage, run)
    })())
}
