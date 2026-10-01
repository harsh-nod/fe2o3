// Physical memory occurrences remain instance-qualified until the original
// source relation decides their ranked counterpart or checked pending obligation.
// This census alone grants neither value equivalence nor memory safety.
struct SourceMemoryEffectV18 {
    coordinate: fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1,
    location: FunctionOperationLocation,
    ordinal: u32,
    pointer: ValueId,
    access: dialect_kernel::AccessKindAttr,
    static_space: AddressSpace,
    concrete_space: Option<AddressSpace>,
    atomic: Option<NormalizedAtomicContractV1>,
    storage_operation: bool,
}

#[derive(Clone, Copy, Default)]
struct SourceMemoryBindingV18 {
    instance: Option<usize>,
    span: Option<usize>,
    lifecycle: Option<usize>,
    failure: Option<usize>,
    terminal: Option<usize>,
}

#[derive(Clone, Copy)]
struct SourceRootParameterV18 {
    slot: usize,
    value: ValueId,
    source_argument: u32,
}

struct SourceRootArgumentsV18<'relation, 'source> {
    relation: &'relation ProductionSourceCorrespondenceV18<'source>,
    root: usize,
    parameters: Vec<Option<SourceRootParameterV18>>,
}

impl<'relation, 'source> SourceRootArgumentsV18<'relation, 'source> {
    fn build(
        relation: &'relation ProductionSourceCorrespondenceV18<'source>,
        root: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<Self> {
        relation.retain_query((|| {
            relation.query(budget)?;
            let physical = relation.source.root(root, budget)?.1;
            let function = relation.inventory.functions().get(physical).ok_or(
                ProductionSourceOwnedViewErrorV18::Binding("source argument physical root"),
            )?;
            let count = function.function.signature.parameters.len();
            budget.reserve_storage(size_of::<Self>())?;
            let mut parameters =
                emission_vec_v1(count, budget).map_err(source_emission_error_v18)?;
            budget.charge_work(count)?;
            parameters.resize(count, None);
            // The returned rows are allocated before argument scratch begins.
            // Its existing cleanup therefore cannot refund these live rows.
            relation.with_root_argument_data_v18(root, budget, |data, budget| {
                data.visit_nodes_scoped(budget, |node, budget| {
                    let ProductionArgumentCoverageV1::Parameter(parameter) = node.coverage() else {
                        return Ok(());
                    };
                    budget.charge_work(4)?;
                    let actual = data
                        .physical(parameter.slot(), budget)?
                        .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
                    if actual.value() != parameter.value() || actual.ty() != parameter.ty() {
                        return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
                    }
                    let row = parameters
                        .get_mut(parameter.slot())
                        .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
                    if row
                        .replace(SourceRootParameterV18 {
                            slot: parameter.slot(),
                            value: parameter.value(),
                            source_argument: node.source_argument(),
                        })
                        .is_some()
                    {
                        return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
                    }
                    Ok(())
                })?;
                for (slot, row) in parameters.iter().enumerate() {
                    budget.charge_work(2)?;
                    if !matches!(row, Some(row) if row.slot == slot) {
                        return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
                    }
                }
                Ok(())
            })?;
            Ok(Self {
                relation,
                root,
                parameters,
            })
        })())
    }

    fn original_argument(
        &self,
        function: &Function,
        parameter: u32,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<u32> {
        self.relation.retain_query((|| {
            self.relation.query(budget)?;
            let physical = self.relation.source.root(self.root, budget)?.1;
            let original = self.relation.inventory.functions().get(physical).ok_or(
                ProductionSourceOwnedViewErrorV18::Binding("source argument original root"),
            )?;
            budget.charge_work(4)?;
            let parameter =
                usize::try_from(parameter).map_err(|_| ArgumentResourceV1::Arithmetic)?;
            let row = self
                .parameters
                .get(parameter)
                .and_then(Option::as_ref)
                .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                    "source argument original physical slot",
                ))?;
            if !std::ptr::eq(original.function, function)
                || row.slot != parameter
                || function
                    .body
                    .as_ref()
                    .and_then(|body| body.parameters.get(parameter))
                    != Some(&row.value)
            {
                return self
                    .relation
                    .source
                    .missing("source argument substituted original function or physical value");
            }
            Ok(row.source_argument)
        })())
    }
}

// Value transport from the exact source operand's actual Load. This does not
// assert which physical Store the Load reads, nor the pointee's lifetime.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SourceScalarReadTransportV18 {
    instance: usize,
    operation: fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1,
    value: ValueId,
    read: ScopedMemoryReadV29,
}

#[derive(Clone, Copy)]
struct SourceScalarLeafRowV18 {
    instance: usize,
    anchor: usize,
    place: *const SemanticPlaceV1,
    read: ScopedMemoryReadV29,
    operation: fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1,
    value: ValueId,
    scalar: ProductionSemanticScalarTypeV2,
    symbol: u32,
    typed_private: bool,
}

#[derive(Clone, Copy)]
struct SourceScalarLeafLookupV18 {
    key: [usize; 3],
    row: usize,
}

// One occurrence table and one tagged lookup index. A leaf names an actual
// read, not its memory version, allocation generation or reaching writer.
struct SourceScalarLeavesV18<'relation, 'source> {
    relation: &'relation ProductionSourceCorrespondenceV18<'source>,
    root: usize,
    rows: Vec<SourceScalarLeafRowV18>,
    lookup: Vec<SourceScalarLeafLookupV18>,
    wrapping: Vec<SourceWrappingValueV23>,
    boundaries: SourceScalarBoundariesV31,
    presences: SourceIssuedPresencesV31,
    lengths: slice_view_v1::SourceDescriptorLengthsV40,
    floor: usize,
    ordinary_values: bool,
}

struct SourceScalarNormalizationInputV18<'a, 'relation, 'source> {
    leaves: &'a SourceScalarLeavesV18<'relation, 'source>,
    arguments: &'a SourceRootArgumentsV18<'relation, 'source>,
}

struct SourceExpressionLeavesV18<'a, 'relation, 'source, 'b, 'w, 'c> {
    leaves: &'a SourceScalarLeavesV18<'relation, 'source>,
    ledger: &'a CorrelationLedgerV18<'b, 'w, 'c>,
    remaining: std::cell::Cell<usize>,
}

impl SemanticExpressionLeavesV18 for SourceExpressionLeavesV18<'_, '_, '_, '_, '_, '_> {
    fn global_invocation_1d(
        &self,
        scalar: ProductionSemanticScalarTypeV2,
        _: &mut dyn CorrelationChargeV18,
    ) -> Option<NormalizedScalarExpressionV1> {
        let result = self.ledger.with_budget(|budget| {
            Ok(self.leaves.relation.retain_query((|| {
                self.leaves.query(budget)?;
                budget.charge_work(1)?;
                if !matches!(
                    scalar,
                    ProductionSemanticScalarTypeV2::Integer {
                        signed: false,
                        bits: 32 | 64
                    }
                ) {
                    return self
                        .leaves
                        .relation
                        .source
                        .missing("source launch expression width differs");
                }
                Ok(NormalizedScalarExpressionV1::GlobalInvocation1d { scalar })
            })()))
        });
        match result {
            Ok(Ok(value)) => Some(value),
            Ok(Err(ProductionSourceOwnedViewErrorV18::Resource(error))) => {
                self.ledger.fail(error);
                None
            }
            Ok(Err(_)) => {
                self.ledger.inconsistent_inventory.set(true);
                None
            }
            Err(_) => None,
        }
    }

    fn begin_node(&self) -> Option<()> {
        self.remaining.set(self.remaining.get().checked_sub(1)?);
        Some(())
    }

    fn symbol(
        &self,
        symbol: u32,
        scalar: ProductionSemanticScalarTypeV2,
        _: &mut dyn CorrelationChargeV18,
    ) -> Option<NormalizedScalarExpressionV1> {
        let result = self.ledger.with_budget(|budget| {
            Ok(self.leaves.relation.retain_query((|| {
                self.leaves.query(budget)?;
                let relation = self.leaves.relation;
                if symbol < PRODUCTION_KERNEL_SCALAR_SYMBOL_BASE_V2 {
                    if !self.leaves.lengths.is_empty()
                        && self
                            .leaves
                            .descriptor_length_symbol_v40(symbol, scalar, budget)?
                    {
                        return Ok(NormalizedScalarExpressionV1::Symbol { symbol, scalar });
                    }
                    if !self.leaves.presences.rows.is_empty()
                        && self.leaves.presence_symbol_v31(symbol, budget)?.is_some()
                    {
                        if scalar != ProductionSemanticScalarTypeV2::Bool {
                            return relation
                                .source
                                .missing("issued presence symbol type differs");
                        }
                        return Ok(NormalizedScalarExpressionV1::Symbol { symbol, scalar });
                    }
                    if let Some(row) = self
                        .leaves
                        .boundary_find_v31([2, symbol as usize, 0, 0], budget)?
                    {
                        if row.scalar != scalar {
                            return relation
                                .source
                                .missing("source SSA boundary symbol type differs");
                        }
                        return Ok(NormalizedScalarExpressionV1::Symbol { symbol, scalar });
                    }
                    let row = self.leaves.find([2, symbol as usize, 0], budget)?.ok_or(
                        ProductionSourceOwnedViewErrorV18::Binding(
                            "source expression private read name is absent",
                        ),
                    )?;
                    if row.scalar != scalar || row.symbol != symbol {
                        return relation
                            .source
                            .missing("source expression private read type or name differs");
                    }
                } else {
                    let argument = symbol
                        .checked_sub(PRODUCTION_KERNEL_SCALAR_SYMBOL_BASE_V2)
                        .filter(|_| symbol < fe2o3_pliron::PRODUCTION_SEMANTIC_LOAD_SYMBOL_BASE_V2)
                        .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                            "source expression entered a ranked symbol namespace",
                        ))?;
                    let source = relation.source.source_semantic(budget)?;
                    let original = relation.source.root(self.leaves.root, budget)?.0;
                    let function = source.functions().get(original.index() as usize).ok_or(
                        ProductionSourceOwnedViewErrorV18::Binding(
                            "source expression original argument function",
                        ),
                    )?;
                    let mut found = false;
                    for local in function.locals() {
                        budget.charge_work(1)?;
                        if local.role() != SemanticLocalRoleV1::Argument(argument) {
                            continue;
                        }
                        if found
                            || kir_semantic_scalar_v1(
                                &lower_scalar_type(source.types(), local.ty())
                                    .map_err(source_emission_error_v18)?,
                            ) != Some(scalar)
                        {
                            return relation
                                .source
                                .missing("source expression original scalar argument changed");
                        }
                        found = true;
                    }
                    if !found {
                        return relation
                            .source
                            .missing("source expression names no original scalar argument");
                    }
                }
                Ok(NormalizedScalarExpressionV1::Symbol { symbol, scalar })
            })()))
        });
        match result {
            Ok(Ok(value)) => Some(value),
            Ok(Err(error)) => {
                if let ProductionSourceOwnedViewErrorV18::Resource(error) = error {
                    self.ledger.fail(error);
                } else {
                    self.ledger.inconsistent_inventory.set(true);
                }
                None
            }
            Err(_) => None,
        }
    }

    fn load(
        &self,
        _: &fe2o3_pliron::ProductionSemanticLoadV2,
        _: &mut dyn CorrelationChargeV18,
    ) -> Option<NormalizedScalarExpressionV1> {
        // Source reads are named only by the exact occurrence table. A ranked
        // Load descriptor cannot be used as a substitute for that association.
        self.ledger.inconsistent_inventory.set(true);
        None
    }
}

struct SourceScalarVisitingV18 {
    rows: [Option<ValueId>; MAX_PRODUCTION_SEMANTIC_EXPRESSION_DEPTH_V2 + 1],
    length: usize,
}

impl ScalarValueVisitingV18 for SourceScalarVisitingV18 {
    fn insert_value(
        &mut self,
        value: ValueId,
        budget: &mut dyn CorrelationChargeV18,
    ) -> Option<bool> {
        for current in &self.rows[..self.length] {
            budget.charge()?;
            if *current == Some(value) {
                return Some(false);
            }
        }
        budget.charge()?;
        *self.rows.get_mut(self.length)? = Some(value);
        self.length = self.length.checked_add(1)?;
        Some(true)
    }

    fn remove_value(
        &mut self,
        value: ValueId,
        budget: &mut dyn CorrelationChargeV18,
    ) -> Option<()> {
        budget.charge()?;
        let index = self.length.checked_sub(1)?;
        if self.rows.get(index)? != &Some(value) {
            return None;
        }
        self.rows[index] = None;
        self.length = index;
        Some(())
    }
}

include!("production_source_scalar_constant_fold_v18.rs");

// Equality is relative to unresolved original read occurrences. This returns
// no ranked, memory or executable owner; the containing consumer must still
// discharge source effects/control/calls and physical read-from/epoch checks.
fn source_scalar_expression_value_v18(
    leaves: &SourceScalarLeavesV18<'_, '_>,
    arguments: &SourceRootArgumentsV18<'_, '_>,
    origins: &value_origin_v1::WholeValueOriginsV18<'_>,
    inline_scalar: &Gfx942InlineScalarCorrespondenceV30<'_>,
    expression: &ProductionSemanticExpressionV2,
    value: ValueId,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<()> {
    let normalizer = SourceScalarNormalizationInputV18 { leaves, arguments };
    source_scalar_expression_endpoint_v18(
        leaves,
        &normalizer,
        origins,
        inline_scalar,
        expression,
        value,
        budget,
    )
}

fn source_scalar_expression_endpoint_v18(
    leaves: &SourceScalarLeavesV18<'_, '_>,
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
        let physical = relation.source.root(leaves.root, budget)?.1;
        let inventory = relation.inventory;
        let function = inventory.functions().get(physical).ok_or(
            ProductionSourceOwnedViewErrorV18::Binding("source scalar normalization root"),
        )?;
        let work_limit = relation
            .source
            .limits(budget)?
            .max_operations
            .checked_mul(UNSUPPORTED_INDEX_CORRELATION_STEPS_PER_OPERATION_V1)
            .ok_or(ArgumentResourceV1::Arithmetic)?;
        let storage = argument_sum_v1(&[
            size_of::<CorrelationLedgerV18<'_, '_, '_>>(),
            size_of::<SourceCorrelationChargeV18<'_, '_, '_, '_>>(),
            size_of::<SourceScalarVisitingV18>(),
            size_of::<SourceScalarNormalizationInputV18<'_, '_, '_>>(),
            size_of::<SourceExpressionLeavesV18<'_, '_, '_, '_, '_, '_>>(),
            size_of::<InventoryCorrelationV18<'_, '_, '_, '_, '_, '_>>(),
            size_of::<BTreeMap<(FunctionOperationLocation, u32), SemanticAccessSiteV1>>(),
            source_scalar_constant_fold_headers_v18()?,
            source_scalar_overflow_query_headers_v23()?,
            argument_product_v1(
                fe2o3_pliron::MAX_PRODUCTION_SEMANTIC_EXPRESSION_NODES_V2,
                argument_product_v1(2, size_of::<NormalizedScalarExpressionV1>())?,
            )?,
        ])?;
        source_scalar_normalization_scratch_v18(
            relation.source.cleanup,
            budget,
            storage,
            |budget| {
                budget.charge_work(MAX_PRODUCTION_SEMANTIC_EXPRESSION_DEPTH_V2 + 1)?;
                let ledger = CorrelationLedgerV18::new(budget, relation.source.cleanup);
                let graph = InventoryCorrelationV18 {
                    origins: Some(origins),
                    inventory,
                    function: function.coordinate,
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
                let mut charge = SourceCorrelationChargeV18 {
                    ledger: &ledger,
                    finite: UnsupportedIndexCorrelationBudgetV1 {
                        remaining: work_limit,
                    },
                    finite_denied: false,
                };
                let mut visiting = SourceScalarVisitingV18 {
                    rows: [None; MAX_PRODUCTION_SEMANTIC_EXPRESSION_DEPTH_V2 + 1],
                    length: 0,
                };
                let no_ranked_reads = BTreeMap::new();
                let mut expected =
                    normalize_semantic_expression_v18(expression, &expected_leaves, 0, &mut charge);
                let mut actual = if expected.is_some() {
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
                let resource = ledger.failure.get();
                let refused = ledger.inconsistent_inventory.get() || charge.finite_denied;
                let equal = constants && expected == actual && visiting.length == 0;
                drop((
                    actual,
                    expected,
                    no_ranked_reads,
                    visiting,
                    charge,
                    expected_leaves,
                    graph,
                ));
                drop(ledger);
                if let Some(error) = resource {
                    return Err(error.into());
                }
                if refused || !equal {
                    return relation.source.missing(
                        "actual scalar expression differs from its original source value",
                    );
                }
                Ok(())
            },
        )
    })())
}

// Only a unit result escapes this boundary. All trees, helper expansions and
// ledger borrows are dropped before either attempt can refund their credits.
fn source_scalar_normalization_scratch_v18<'work>(
    cleanup: &ScopedSourceCleanupV29,
    budget: &mut ArgumentBudgetV1<'work>,
    storage: usize,
    run: impl FnOnce(&mut ArgumentBudgetV1<'work>) -> SourceOwnedResultV18<()>,
) -> SourceOwnedResultV18<()> {
    if cleanup.is_denied() {
        return Err(ArgumentResourceV1::Accounting.into());
    }
    let floor = budget.storage();
    let attempt = |budget: &mut ArgumentBudgetV1<'work>| {
        budget.reserve_storage(storage)?;
        // This inner floor includes the still-live fixed scratch, so Err and
        // unwind cannot hide a local undercut above the caller's outer floor.
        let retained = budget.storage();
        scoped_source_attempt_v29(cleanup, budget, retained, run)
    };
    type Result = SourceOwnedResultV18<()>;
    budget.reserve_storage(argument_sum_v1(&[
        std::mem::size_of_val(&attempt),
        argument_product_v1(
            2,
            argument_sum_v1(&[
                size_of::<Result>(),
                size_of::<std::thread::Result<Result>>(),
                size_of::<std::panic::AssertUnwindSafe<Result>>(),
            ])?,
        )?,
    ])?)?;
    scoped_source_attempt_v29(cleanup, budget, floor, attempt)?;
    budget.release_storage(
        budget
            .storage()
            .checked_sub(floor)
            .ok_or(ArgumentResourceV1::Accounting)?,
    )?;
    Ok(())
}

impl SourceScalarNormalizationV18 for SourceScalarNormalizationInputV18<'_, '_, '_> {
    fn binary_overflow(
        &self,
        function: &Function,
        value: ValueId,
        overflow: ProductionOverflowContractV2,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<ProductionOverflowContractV2> {
        self.leaves
            .wrapping_value_v23(function, value, overflow, budget)
    }

    fn leaf(
        &self,
        function: &Function,
        value: ValueId,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<Option<NormalizedScalarExpressionV1>> {
        self.leaves.actual_value(function, value, budget)
    }

    fn argument(
        &self,
        function: &Function,
        argument: u32,
        scalar: ProductionSemanticScalarTypeV2,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<NormalizedScalarExpressionV1> {
        self.leaves.relation.retain_query((|| {
            self.leaves.query(budget)?;
            budget.charge_work(3)?;
            if self.leaves.root != self.arguments.root
                || !std::ptr::eq(self.leaves.relation, self.arguments.relation)
            {
                return self
                    .leaves
                    .relation
                    .source
                    .missing("scalar normalization changed original argument owner");
            }
            let parameter =
                usize::try_from(argument).map_err(|_| ArgumentResourceV1::Arithmetic)?;
            if function
                .signature
                .parameters
                .get(parameter)
                .and_then(kir_semantic_scalar_v1)
                != Some(scalar)
            {
                return self
                    .leaves
                    .relation
                    .source
                    .missing("scalar normalization argument type changed");
            }
            let original = self
                .arguments
                .original_argument(function, argument, budget)?;
            let symbol = PRODUCTION_KERNEL_SCALAR_SYMBOL_BASE_V2
                .checked_add(original)
                .filter(|symbol| *symbol < fe2o3_pliron::PRODUCTION_SEMANTIC_LOAD_SYMBOL_BASE_V2)
                .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                    "source scalar argument namespace exhausted",
                ))?;
            Ok(NormalizedScalarExpressionV1::Symbol { symbol, scalar })
        })())
    }
}

/// Borrowed expression inputs for one exact original-source root.
/// These names identify scalar reads only; they are not memory, ranked or
/// execution proofs and must never enter a public ranked proof transcript.
pub struct ProductionSourceScalarLeavesV18<'scope> {
    leaves: &'scope SourceScalarLeavesV18<'scope, 'scope>,
}

/// An original unchanged argument's source expression boundary.
/// The caller descriptor is not a certificate of physical argument transport.
pub enum ProductionSourceScalarArgumentV18<'scope> {
    /// A scalar argument of the exact original root, before physical splitting.
    Root {
        /// Original logical source argument, not a physical parameter ordinal.
        argument: u32,
    },
    /// The original operand at this specific helper invocation.
    Caller {
        /// Original caller instance in the same retained root.
        instance: usize,
        /// Original caller declaration, borrowed without reconstruction.
        function: &'scope SemanticFunctionDeclV1,
        /// Original source call block; arguments are evaluated at its terminator.
        block: SemanticBlockIdV1,
        /// Original scalar operand at the selected logical argument position.
        operand: &'scope SemanticOperandV1,
    },
}

/// Original source value selected by one captured scalar Store.
/// This borrowed descriptor grants no memory or ranked proof authority.
#[derive(Clone, Copy)]
pub enum ProductionSourceScalarInputV18<'scope> {
    /// An original operand evaluated at its exact source position.
    Operand {
        /// Original source block.
        block: SemanticBlockIdV1,
        /// Original statement ordinal, or the terminator when absent.
        statement: Option<u32>,
        /// Borrowed original operand, not a reconstructed value.
        operand: &'scope SemanticOperandV1,
    },
    /// The successful original scalar assignment result.
    Assignment {
        /// Original source block.
        block: SemanticBlockIdV1,
        /// Original assignment ordinal.
        statement: u32,
        /// Borrowed original statement.
        assignment: &'scope SemanticStatementKindV1,
    },
    /// The result of one original direct call at its normal return.
    CallResult {
        /// Original call block.
        block: SemanticBlockIdV1,
        /// Borrowed original call.
        call: &'scope SemanticDirectCallV1,
    },
    /// One original logical function argument, before physical splitting.
    EntryArgument {
        /// Original logical argument position, not a physical slot ordinal.
        argument: u32,
    },
}

include!("production_source_entry_rhs_v18.rs");

/// One scoped original Store input. The actual Store is retained privately;
/// callers cannot substitute its graph value or source occurrence.
pub struct ProductionSourceScalarStoreV18<'scope> {
    leaves: &'scope SourceScalarLeavesV18<'scope, 'scope>,
    arguments: &'scope SourceRootArgumentsV18<'scope, 'scope>,
    origins: &'scope value_origin_v1::WholeValueOriginsV18<'scope>,
    inline_scalar: &'scope Gfx942InlineScalarCorrespondenceV30<'scope>,
    instance: usize,
    function: &'scope SemanticFunctionDeclV1,
    row: usize,
    anchor: &'scope ScopedMemoryAnchorV29,
    operation: fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1,
    source: ScopedMemoryStoreSourceV29,
    scalar: ProductionSemanticScalarTypeV2,
    value: ValueId,
    floor: usize,
}

impl ProductionSourceScalarStoreV18<'_> {
    fn check(&self, budget: &mut ArgumentBudgetV1<'_>) -> SourceOwnedResultV18<()> {
        let relation = self.leaves.relation;
        relation.retain_query((|| {
            if budget.storage() < self.floor {
                relation.source.cleanup.deny_refund();
                return Err(ArgumentResourceV1::Accounting.into());
            }
            self.leaves.query(budget)?;
            let source = relation.source.source_semantic(budget)?;
            let original = relation
                .source
                .instance(self.leaves.root, self.instance, budget)?
                .0;
            budget.charge_work(5)?;
            if !source
                .functions()
                .get(original.index() as usize)
                .is_some_and(|function| std::ptr::eq(function, self.function))
            {
                return relation
                    .source
                    .missing("scalar Store request changed original instance or function");
            }
            let access = SourcePhysicalAccessV18 {
                instance: self.instance,
                row: self.row,
                anchor: self.anchor,
            };
            let payload = relation
                .retained_scalar_payload_v18(self.leaves.root, self.operation, &access, budget)?
                .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                    "scalar Store request payload absent",
                ))?;
            if payload.value != self.value
                || !matches!(payload.source,
                ScopedMemoryPayloadV29::Store { source, .. } if *source == self.source)
            {
                return relation
                    .source
                    .missing("scalar Store request changed original payload or physical value");
            }
            let ty = match self.source {
                ScopedMemoryStoreSourceV29::Operand { ty, .. }
                | ScopedMemoryStoreSourceV29::Assignment { ty, .. }
                | ScopedMemoryStoreSourceV29::AssignmentComponent { ty, .. }
                | ScopedMemoryStoreSourceV29::CallResult { ty, .. }
                | ScopedMemoryStoreSourceV29::EntryArgument { ty, .. } => ty,
            };
            if kir_semantic_scalar_v1(
                &lower_scalar_type(source.types(), ty).map_err(source_emission_error_v18)?,
            ) != Some(self.scalar)
            {
                return relation
                    .source
                    .missing("scalar Store request changed original scalar type");
            }
            Ok(())
        })())
    }

    /// Returns the exact original invocation and declaration under the same ledger.
    pub fn original(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<(usize, &SemanticFunctionDeclV1)> {
        self.check(budget)?;
        Ok((self.instance, self.function))
    }

    /// Borrows the original input; its expression still needs independent resolution.
    pub fn input(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<ProductionSourceScalarInputV18<'_>> {
        self.input_for(self.function, budget)
    }

    /// Reborrows the same original input through an independently retained
    /// declaration. The declaration must be the exact original instance's
    /// function, not a same-shaped copy or another helper invocation.
    pub fn input_for<'function>(
        &self,
        function: &'function SemanticFunctionDeclV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<ProductionSourceScalarInputV18<'function>> {
        self.leaves.relation.retain_query((|| {
            self.check(budget)?;
            budget.charge_work(4)?;
            if !std::ptr::eq(function, self.function) {
                return self
                    .leaves
                    .relation
                    .source
                    .missing("scalar Store resolver substituted original declaration");
            }
            Ok(match self.source {
                ScopedMemoryStoreSourceV29::Operand { site, role, .. } => {
                    let operand = scoped_source_operand_v29(function, site, role).ok_or(
                        ProductionSourceOwnedViewErrorV18::Binding(
                            "scalar Store resolver operand absent",
                        ),
                    )?;
                    let (block, statement) = match site {
                        ExecutionSiteV29::Statement { block, statement } => {
                            (block, Some(statement))
                        }
                        ExecutionSiteV29::Terminator { block } => (block, None),
                    };
                    ProductionSourceScalarInputV18::Operand {
                        block: SemanticBlockIdV1::from_index(block.get()),
                        statement,
                        operand,
                    }
                }
                ScopedMemoryStoreSourceV29::Assignment { site, .. } => {
                    let ExecutionSiteV29::Statement { block, statement } = site else {
                        return self
                            .leaves
                            .relation
                            .source
                            .missing("scalar Store resolver assignment absent");
                    };
                    let assignment = scoped_source_statement_v29(function, site).ok_or(
                        ProductionSourceOwnedViewErrorV18::Binding(
                            "scalar Store resolver assignment absent",
                        ),
                    )?;
                    ProductionSourceScalarInputV18::Assignment {
                        block: SemanticBlockIdV1::from_index(block.get()),
                        statement,
                        assignment,
                    }
                }
                ScopedMemoryStoreSourceV29::CallResult {
                    site: ExecutionSiteV29::Terminator { block },
                    ..
                } => {
                    let Some(SemanticTerminatorKindV1::Call(call)) = function
                        .blocks()
                        .get(block.get() as usize)
                        .map(|row| row.terminator().kind())
                    else {
                        return self
                            .leaves
                            .relation
                            .source
                            .missing("scalar Store resolver call absent");
                    };
                    ProductionSourceScalarInputV18::CallResult {
                        block: SemanticBlockIdV1::from_index(block.get()),
                        call,
                    }
                }
                ScopedMemoryStoreSourceV29::EntryArgument { local, .. } => {
                    let Some(SemanticLocalRoleV1::Argument(argument)) = function
                        .locals()
                        .get(local.index() as usize)
                        .map(|row| row.role())
                    else {
                        return self
                            .leaves
                            .relation
                            .source
                            .missing("scalar Store resolver original argument absent");
                    };
                    ProductionSourceScalarInputV18::EntryArgument { argument }
                }
                _ => {
                    return self
                        .leaves
                        .relation
                        .source
                        .missing("scalar Store resolver input kind changed");
                }
            })
        })())
    }

    /// Returns the admitted scalar type of this exact original Store input.
    pub fn scalar(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<ProductionSemanticScalarTypeV2> {
        self.check(budget)?;
        Ok(self.scalar)
    }

    /// Compares an independently reconstructed expression with the exact
    /// captured physical Store RHS. Equality is relative to unresolved Load
    /// occurrences and grants no source, ranked, epoch or memory capability.
    pub fn check_expression(
        &self,
        expression: &ProductionSemanticExpressionV2,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<()> {
        self.leaves.relation.retain_query((|| {
            self.check(budget)?;
            if expression.scalar() != self.scalar {
                return self
                    .leaves
                    .relation
                    .source
                    .missing("scalar Store expression type changed");
            }
            source_scalar_expression_value_v18(
                self.leaves,
                self.arguments,
                self.origins,
                self.inline_scalar,
                expression,
                self.value,
                budget,
            )
        })())
    }
}

fn source_scalar_input_v18<'a>(
    leaves: &'a SourceScalarLeavesV18<'_, '_>,
    arguments: &'a SourceRootArgumentsV18<'_, '_>,
    origins: &'a value_origin_v1::WholeValueOriginsV18<'_>,
    inline_scalar: &'a Gfx942InlineScalarCorrespondenceV30<'_>,
    access: &SourcePhysicalAccessV18<'a>,
    operation: fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1,
    value: ValueId,
    source: ScopedMemoryStoreSourceV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<Option<ProductionSourceScalarStoreV18<'a>>> {
    let relation = leaves.relation;
    relation.retain_query((|| {
        leaves.query(budget)?;
        let semantic = relation.source.source_semantic(budget)?;
        let id = relation.source.instance(leaves.root, access.instance, budget)?.0;
        let function = semantic.functions().get(id.index() as usize)
            .ok_or(ProductionSourceOwnedViewErrorV18::Binding("scalar Store original function"))?;
        let ty = match source {
            ScopedMemoryStoreSourceV29::Operand { ty, .. }
            | ScopedMemoryStoreSourceV29::Assignment { ty, .. }
            | ScopedMemoryStoreSourceV29::AssignmentComponent { ty, .. }
            | ScopedMemoryStoreSourceV29::CallResult { ty, .. }
            | ScopedMemoryStoreSourceV29::EntryArgument { ty, .. } => ty,
        };
        budget.charge_work(2)?;
        if !matches!(semantic.types().get(ty.index() as usize).map(SemanticTypeDeclV1::shape),
            Some(SemanticTypeShapeV1::Scalar(_))) { return Ok(None); }
        let scalar = kir_semantic_scalar_v1(&lower_scalar_type(semantic.types(), ty).map_err(source_emission_error_v18)?)
            .ok_or(ProductionSourceOwnedViewErrorV18::Binding("scalar Store original scalar type"))?;
        let _input = match source {
            ScopedMemoryStoreSourceV29::Operand { site, role, source, .. } => {
                let operand = scoped_source_operand_v29(function, site, role)
                    .ok_or(ProductionSourceOwnedViewErrorV18::Binding("scalar Store original operand"))?;
                if operand.ty() != ty { return relation.source.missing("scalar Store operand type changed"); }
                match (source, operand) {
                    (ScopedMemoryOperandSourceV29::Constant, SemanticOperandV1::Constant(_)) => {}
                    (ScopedMemoryOperandSourceV29::Place(occurrence @ ScopedMemoryOccurrenceV29::Promoted { .. })
                        | ScopedMemoryOperandSourceV29::Memory { occurrence, .. },
                        SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place)) => {
                        let occurrences = relation.source.source_ssa(budget)?.occurrences_v1()
                            .and_then(|rows| rows.function(id))
                            .ok_or(ProductionSourceOwnedViewErrorV18::Binding("scalar Store original occurrence census"))?;
                        check_scoped_payload_occurrence_v29(&occurrences, site, role, place, occurrence, budget)
                            .map_err(source_emission_error_v18)?;
                    }
                    _ => return relation.source.missing("scalar Store input has no completed original operand transport"),
                }
                let (block, statement) = match site {
                    ExecutionSiteV29::Statement { block, statement } => (block, Some(statement)),
                    ExecutionSiteV29::Terminator { block } => (block, None),
                };
                ProductionSourceScalarInputV18::Operand {
                    block: SemanticBlockIdV1::from_index(block.get()), statement, operand,
                }
            }
            ScopedMemoryStoreSourceV29::Assignment { site, .. } => {
                let ExecutionSiteV29::Statement { block, statement } = site else {
                    return relation.source.missing("scalar assignment Store input has no original statement");
                };
                let original = scoped_source_statement_v29(function, site)
                    .ok_or(ProductionSourceOwnedViewErrorV18::Binding("scalar assignment Store source absent"))?;
                if !matches!(original, SemanticStatementKindV1::Assign(assignment)
                    if assignment.value().result_type() == ty && assignment.destination().ty() == ty)
                { return relation.source.missing("scalar assignment Store result type changed"); }
                ProductionSourceScalarInputV18::Assignment {
                    block: SemanticBlockIdV1::from_index(block.get()), statement, assignment: original,
                }
            }
            ScopedMemoryStoreSourceV29::AssignmentComponent { .. } => {
                return relation.source.missing("computed result requires its typed object write relation");
            }
            ScopedMemoryStoreSourceV29::CallResult { site, .. } => {
                let ExecutionSiteV29::Terminator { block } = site else {
                    return relation.source.missing("scalar call-result Store input is not a terminator");
                };
                let Some(SemanticTerminatorKindV1::Call(call)) = function.blocks().get(block.get() as usize)
                    .map(|block| block.terminator().kind()) else {
                        return relation.source.missing("scalar call-result Store original call absent");
                    };
                if call.destination().is_none_or(|destination| destination.place().ty() != ty) {
                    return relation.source.missing("scalar call-result Store type changed");
                }
                ProductionSourceScalarInputV18::CallResult {
                    block: SemanticBlockIdV1::from_index(block.get()), call,
                }
            }
            ScopedMemoryStoreSourceV29::EntryArgument { local, .. } => {
                let declaration = function.locals().get(local.index() as usize)
                    .filter(|declaration| declaration.ty() == ty)
                    .ok_or(ProductionSourceOwnedViewErrorV18::Binding("scalar entry Store original local changed"))?;
                let SemanticLocalRoleV1::Argument(argument) = declaration.role() else {
                    return relation.source.missing("scalar entry Store does not name an original argument");
                };
                ProductionSourceScalarInputV18::EntryArgument { argument }
            }
        };
        Ok(Some(ProductionSourceScalarStoreV18 { leaves, arguments, origins, inline_scalar, instance: access.instance,
            function, row: access.row, anchor: access.anchor, operation, source, scalar, value,
            floor: budget.storage() }))
    })())
}

impl ProductionSourceScalarLeavesV18<'_> {
    /// Visits original captured scalar Store inputs without exporting their
    /// physical value IDs. The containing consumer must separately census all
    /// effects and discharge memory/epoch obligations; this returns no proof owner.
    pub fn visit_store_inputs<'work, E>(
        &self,
        budget: &mut ArgumentBudgetV1<'work>,
        consume: impl for<'request> FnMut(
            &ProductionSourceScalarStoreV18<'request>,
            &mut ArgumentBudgetV1<'work>,
        ) -> Result<(), E>,
    ) -> Result<usize, E>
    where
        E: From<ProductionSourceOwnedViewErrorV18>,
    {
        let consume = SourceCallbackCustodyV29::new(consume);
        let leaves = self.leaves;
        let relation = leaves.relation;
        leaves.query(budget)?;
        let floor = budget.storage();
        let capture = std::mem::size_of_val(&consume);
        let alignment = std::mem::align_of_val(&consume);
        let prepared =
            scoped_source_attempt_v29(relation.source.cleanup, budget, floor, |budget| {
                consume.prepare(|| {
                    let floor = budget.storage();
                    relation.retain_query((|| {
                        let headers = argument_sum_v1(&[
                            capture,
                            alignment,
                            size_of::<ProductionSourceScalarStoreV18<'_>>(),
                            source_callback_custody_finish_preflight_v29::<usize, E>(budget)?,
                            source_owned_finish_preflight_v26::<usize, E>(budget)?,
                        ])?;
                        budget.reserve_storage(headers)?;
                        let arguments =
                            SourceRootArgumentsV18::build(relation, leaves.root, budget)?;
                        let inline_scalar = Gfx942InlineScalarCorrespondenceV30::build_source_v18(
                            relation,
                            leaves.root,
                            budget,
                        )?;
                        let physical = relation.source.root(leaves.root, budget)?.1;
                        let function = relation
                            .inventory
                            .functions()
                            .get(physical)
                            .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                                "scalar Store origin root",
                            ))?
                            .coordinate;
                        let retained = budget
                            .storage()
                            .checked_sub(floor)
                            .ok_or(ArgumentResourceV1::Accounting)?;
                        Ok((arguments, inline_scalar, function, retained))
                    })())
                })
            });
        let ((arguments, inline_scalar, function, retained), mut consume) = match prepared {
            Ok(prepared) => prepared,
            Err(error) => {
                return Err(error.into());
            }
        };
        let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            value_origin_v1::with_whole_value_origins_v18(
                relation,
                function,
                budget,
                |origins, budget| {
                    let retained_floor = budget.storage();
                    let root = relation.retain_query(relation.source.root_row(leaves.root))?;
                    let mut count = 0usize;
                    for original in &root.coordinates.sources.rows {
                        let active = relation.retain_query((|| {
                            budget.charge_work(1)?;
                            let instance = original.instance.index();
                            let Some(sidecar) =
                                relation
                                    .source
                                    .optional_sidecar(leaves.root, instance, budget)?
                            else {
                                return Ok(None);
                            };
                            let anchors = sidecar.scoped_memory_anchors.as_ref().ok_or(
                                ProductionSourceOwnedViewErrorV18::Binding(
                                    "scalar Store source census absent",
                                ),
                            )?;
                            Ok(Some((instance, anchors)))
                        })())?;
                        let Some((instance, anchors)) = active else {
                            continue;
                        };
                        for (row, anchor) in anchors.rows.iter().enumerate() {
                            let request = relation.retain_query((|| {
                                budget.charge_work(1)?;
                                let ScopedMemoryAnchorKindV29::Access {
                                    payload: Some(ScopedMemoryPayloadV29::Store { source, .. }),
                                    ..
                                } = &anchor.kind
                                else {
                                    return Ok(None);
                                };
                                let key = TileAttachmentKeyV29 {
                                    root: leaves.root,
                                    family: TileAttachmentFamilyV29::MemoryAnchor,
                                    instance,
                                    row,
                                    field: TileAttachmentFieldV29::MemoryPosition,
                                    component: 0,
                                    part: 0,
                                };
                                let [position] = relation.attachment_range(key, budget)? else {
                                    return Err(ProductionSourceOwnedViewErrorV18::Binding(
                                        "scalar Store position census",
                                    )
                                    .into());
                                };
                                let ProductionSourceOperationV18::Operation(operation) =
                                    relation.mapped_source_operation(position.location, budget)?
                                else {
                                    return Err(ProductionSourceOwnedViewErrorV18::Binding(
                                        "scalar Store has no physical operation",
                                    )
                                    .into());
                                };
                                let access = SourcePhysicalAccessV18 {
                                    instance,
                                    row,
                                    anchor,
                                };
                                let payload = relation
                                    .retained_scalar_payload_v18(
                                        leaves.root,
                                        operation,
                                        &access,
                                        budget,
                                    )?
                                    .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                                        "scalar Store lost payload",
                                    ))?;
                                let Some(request) = source_scalar_input_v18(
                                    leaves,
                                    &arguments,
                                    origins,
                                    &inline_scalar,
                                    &access,
                                    operation,
                                    payload.value,
                                    *source,
                                    budget,
                                )?
                                else {
                                    return Ok(None);
                                };
                                if matches!(
                                    source,
                                    ScopedMemoryStoreSourceV29::Operand {
                                        source: ScopedMemoryOperandSourceV29::Memory { .. },
                                        ..
                                    }
                                ) && source_scalar_read_store_v18(
                                    relation,
                                    leaves.root,
                                    &access,
                                    &payload,
                                    budget,
                                )?
                                .is_none()
                                {
                                    return Err(ProductionSourceOwnedViewErrorV18::Binding(
                                        "scalar Store prior Load transport absent",
                                    )
                                    .into());
                                }
                                Ok(Some(request))
                            })(
                            ))?;
                            let Some(request) = request else {
                                continue;
                            };
                            consume
                                .as_mut()
                                .expect("scalar store visitor remains in custody")(
                                &request, budget,
                            )?;
                            if budget.storage() < retained_floor {
                                relation.source.cleanup.deny_refund();
                                return relation
                                    .retain_query(Err(ArgumentResourceV1::Accounting.into()))
                                    .map_err(Into::into);
                            }
                            leaves.query(budget)?;
                            count = relation.retain_query(count.checked_add(1).ok_or(
                                ProductionSourceOwnedViewErrorV18::Resource(
                                    ArgumentResourceV1::Arithmetic,
                                ),
                            ))?;
                        }
                    }
                    Ok::<_, E>(count)
                },
            )
        }));
        let caught = consume.finish(caught);
        let prior = relation.source.guard.first.get();
        let postflight = leaves.observe_custody(budget);
        drop((arguments, inline_scalar));
        source_owned_finish_callback_v18(
            caught,
            prior,
            postflight,
            relation.source.cleanup,
            budget,
            retained,
        )
    }

    /// Propagates a containing consumer's cleanup veto. This cannot restore
    /// custody, authorize a refund or alter an already selected diagnostic.
    pub fn deny_refund(&self) {
        self.leaves.relation.source.cleanup.deny_refund();
    }

    /// Observes original ledger/floor custody without charging work or
    /// inventing an earlier source-query diagnostic during error cleanup.
    pub fn observe_custody(&self, budget: &ArgumentBudgetV1<'_>) -> SourceOwnedResultV18<()> {
        self.leaves.observe_custody(budget)
    }

    /// Checks the original ledger and the complete retained leaf-table floor.
    pub fn check(&self, budget: &mut ArgumentBudgetV1<'_>) -> SourceOwnedResultV18<()> {
        self.leaves.query(budget)
    }

    /// Borrows the exact original declaration for this retained invocation.
    pub fn original_function(
        &self,
        instance: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<&SemanticFunctionDeclV1> {
        self.leaves.relation.retain_query((|| {
            self.leaves.query(budget)?;
            let source = self.leaves.relation.source.source_semantic(budget)?;
            let (function, _) =
                self.leaves
                    .relation
                    .source
                    .instance(self.leaves.root, instance, budget)?;
            source.functions().get(function.index() as usize).ok_or(
                ProductionSourceOwnedViewErrorV18::Binding("scalar leaf original function"),
            )
        })())
    }

    /// Resolves only an exact captured whole scalar read in this source instance.
    /// Absence leaves the existing source resolver responsible for the value.
    pub fn original_place(
        &self,
        instance: usize,
        function: &SemanticFunctionDeclV1,
        place: &SemanticPlaceV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<Option<ProductionSemanticExpressionV2>> {
        self.leaves
            .original_place(instance, function, place, budget)
    }

    /// Returns the original logical argument boundary, retaining invocation
    /// identity. No helper argument is relabeled as a root parameter.
    pub fn original_argument(
        &self,
        instance: usize,
        function: &SemanticFunctionDeclV1,
        argument: u32,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<ProductionSourceScalarArgumentV18<'_>> {
        let relation = self.leaves.relation;
        relation.retain_query((|| {
            self.leaves.query(budget)?;
            let source = relation.source.source_semantic(budget)?;
            let (original, incoming) =
                relation
                    .source
                    .instance(self.leaves.root, instance, budget)?;
            budget.charge_work(2)?;
            if !source
                .functions()
                .get(original.index() as usize)
                .is_some_and(|declaration| std::ptr::eq(declaration, function))
            {
                return relation
                    .source
                    .missing("scalar argument substituted original function");
            }
            let mut declared = None;
            for local in function.locals() {
                budget.charge_work(1)?;
                if local.role() == SemanticLocalRoleV1::Argument(argument) {
                    if declared.replace(local.ty()).is_some() {
                        return relation
                            .source
                            .missing("scalar argument source declaration is ambiguous");
                    }
                }
            }
            let ty = declared.ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                "scalar argument original declaration",
            ))?;
            if !matches!(
                source
                    .types()
                    .get(ty.index() as usize)
                    .map(SemanticTypeDeclV1::shape),
                Some(SemanticTypeShapeV1::Scalar(_))
            ) {
                return relation
                    .source
                    .missing("scalar argument is not a whole scalar source value");
            }
            let Some((caller, block)) = incoming else {
                if relation.source.root(self.leaves.root, budget)?.0 != original {
                    return relation
                        .source
                        .missing("scalar argument root association changed");
                }
                return Ok(ProductionSourceScalarArgumentV18::Root { argument });
            };
            let (caller_function, _) =
                relation.source.instance(self.leaves.root, caller, budget)?;
            let declaration = source
                .functions()
                .get(caller_function.index() as usize)
                .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                    "scalar argument original caller",
                ))?;
            budget.charge_work(5)?;
            let SemanticTerminatorKindV1::Call(call) = declaration
                .blocks()
                .get(block.index() as usize)
                .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                    "scalar argument original call block",
                ))?
                .terminator()
                .kind()
            else {
                return relation
                    .source
                    .missing("scalar argument has no original caller terminator");
            };
            if !matches!(source.callables().get(call.callee().index() as usize),
                Some(SemanticCallableDeclV1::Defined { function }) if *function == original)
            {
                return relation
                    .source
                    .missing("scalar argument original callee differs");
            }
            let operand = call.arguments().get(argument as usize).ok_or(
                ProductionSourceOwnedViewErrorV18::Binding("scalar argument original call operand"),
            )?;
            if semantic_operand_type(operand) != ty {
                return relation
                    .source
                    .missing("scalar argument original call type differs");
            }
            Ok(ProductionSourceScalarArgumentV18::Caller {
                instance: caller,
                function: declaration,
                block,
                operand,
            })
        })())
    }
}

enum SourceScalarNamespaceV18<'a> {
    Ranked(&'a fe2o3_pliron::ProductionRankedKernelV1),
    SourceOnly,
    PrivateSourceWritesV22,
    OriginalSourceExpressionsV23,
}

include!("production_source_private_scalar_leaves_v22.rs");

impl ProductionSourceCorrespondenceV18<'_> {
    /// Gives the shared source resolver private read names over this exact
    /// source and canonical graph. The recipe is used solely to exclude its
    /// existing symbols; this method grants no recipe/source equivalence.
    pub fn with_scalar_leaves_v18<'work, T, E>(
        &self,
        root: usize,
        recipe: &fe2o3_pliron::ProductionRankedKernelV1,
        budget: &mut ArgumentBudgetV1<'work>,
        consume: impl for<'scope> FnOnce(
            &ProductionSourceScalarLeavesV18<'scope>,
            &mut ArgumentBudgetV1<'work>,
        ) -> Result<T, E>,
    ) -> Result<T, E>
    where
        E: From<ProductionSourceOwnedViewErrorV18>,
    {
        self.with_scalar_leaf_namespace_v18(
            root,
            &SourceScalarNamespaceV18::Ranked(recipe),
            budget,
            consume,
        )
    }

    /// Lends private scalar names without a ranked recipe. The root, argument
    /// symbols and complete original Load census still come from this exact
    /// source/canonical relation. This grants no ranked or memory equivalence.
    pub fn with_source_scalar_leaves_v18<'work, T, E>(
        &self,
        root: usize,
        budget: &mut ArgumentBudgetV1<'work>,
        consume: impl for<'scope> FnOnce(
            &ProductionSourceScalarLeavesV18<'scope>,
            &mut ArgumentBudgetV1<'work>,
        ) -> Result<T, E>,
    ) -> Result<T, E>
    where
        E: From<ProductionSourceOwnedViewErrorV18>,
    {
        self.with_scalar_leaf_namespace_v18(
            root,
            &SourceScalarNamespaceV18::SourceOnly,
            budget,
            consume,
        )
    }

    fn with_scalar_leaf_namespace_v18<'work, T, E>(
        &self,
        root: usize,
        namespace: &SourceScalarNamespaceV18<'_>,
        budget: &mut ArgumentBudgetV1<'work>,
        consume: impl for<'scope> FnOnce(
            &ProductionSourceScalarLeavesV18<'scope>,
            &mut ArgumentBudgetV1<'work>,
        ) -> Result<T, E>,
    ) -> Result<T, E>
    where
        E: From<ProductionSourceOwnedViewErrorV18>,
    {
        let consume = SourceCallbackCustodyV29::new(consume);
        self.query(budget)?;
        let floor = budget.storage();
        let capture = std::mem::size_of_val(&consume);
        let alignment = std::mem::align_of_val(&consume);
        // Attempt-header refusal happens before the constructor callback can
        // retain it. It is still the first failure of this source query.
        let prepared = self.retain_query(scoped_source_attempt_v29(
            self.source.cleanup,
            budget,
            floor,
            |budget| {
                consume.prepare(|| {
                    let floor = budget.storage();
                    self.retain_query((|| {
                        let headers = argument_sum_v1(&[
                            capture,
                            alignment,
                            source_owned_finish_preflight_v26::<T, E>(budget)?,
                            size_of::<ProductionSourceScalarLeavesV18<'_>>(),
                            size_of::<SourceScalarNamespaceV18<'_>>(),
                            size_of::<&SourceScalarNamespaceV18<'_>>(),
                            size_of::<std::thread::Result<Result<T, E>>>(),
                        ])?;
                        budget.reserve_storage(headers)?;
                        let leaves = SourceScalarLeavesV18::build(self, root, namespace, budget)?;
                        let retained = budget
                            .storage()
                            .checked_sub(floor)
                            .ok_or(ArgumentResourceV1::Accounting)?;
                        Ok((leaves, retained))
                    })())
                })
            },
        ));
        let ((mut leaves, retained), mut consume) = match prepared {
            Ok(prepared) => prepared,
            Err(error) => {
                return Err(error.into());
            }
        };
        // The constructor's own callback header has now been settled. Only
        // the still-live leaf scope contributes to the returned custody floor.
        leaves.floor = budget.storage();
        let view = ProductionSourceScalarLeavesV18 { leaves: &leaves };
        let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            consume
                .take()
                .expect("scalar leaf consumer is invoked once")(&view, budget)
        }));
        drop(consume);
        let prior = self.source.guard.first.get();
        let postflight = if matches!(&caught, Ok(Ok(_))) {
            leaves.query(budget)
        } else {
            leaves.observe_custody(budget)
        };
        drop(view);
        drop(leaves);
        source_owned_finish_callback_v18(
            caught,
            prior,
            postflight,
            self.source.cleanup,
            budget,
            retained,
        )
    }
}

fn source_leaf_event_v18(read: ScopedMemoryReadV29) -> usize {
    match read.occurrence {
        ScopedMemoryOccurrenceV29::Promoted { event, .. }
        | ScopedMemoryOccurrenceV29::Retained { event } => event,
    }
}

fn source_leaf_original_order_v18(row: &SourceScalarLeafRowV18) -> [usize; 3] {
    [
        row.instance,
        source_leaf_event_v18(row.read),
        row.read.prefix as usize,
    ]
}

fn visit_source_expression_symbols_v18(
    expression: &ProductionSemanticExpressionV2,
    budget: &mut ArgumentBudgetV1<'_>,
    visit: &mut impl FnMut(u32, &mut ArgumentBudgetV1<'_>) -> SourceOwnedResultV18<()>,
) -> SourceOwnedResultV18<()> {
    use ProductionSemanticExpressionV2 as E;
    const STACK: usize = MAX_PRODUCTION_SEMANTIC_EXPRESSION_DEPTH_V2 * 2 + 1;
    // The caller prepays this fixed scratch. Three-child DFS retains at most
    // two siblings per depth; original grammar limits still apply per tree.
    budget.charge_work(STACK)?;
    let mut stack = [None; STACK];
    stack[0] = Some((expression, 0usize));
    let mut length = 1usize;
    let mut nodes = 0usize;
    while length != 0 {
        budget.charge_work(1)?;
        length -= 1;
        let (expression, depth) = stack[length].take().ok_or(ArgumentResourceV1::Accounting)?;
        nodes = nodes.checked_add(1).ok_or(ArgumentResourceV1::Arithmetic)?;
        if depth > MAX_PRODUCTION_SEMANTIC_EXPRESSION_DEPTH_V2
            || nodes > fe2o3_pliron::MAX_PRODUCTION_SEMANTIC_EXPRESSION_NODES_V2
        {
            return Err(ProductionSourceOwnedViewErrorV18::Binding(
                "source expression symbol census exceeds original grammar",
            ));
        }
        let next = depth.checked_add(1).ok_or(ArgumentResourceV1::Arithmetic)?;
        let mut push = |child| -> SourceOwnedResultV18<()> {
            budget.charge_work(1)?;
            let slot = stack
                .get_mut(length)
                .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                    "source expression symbol stack exceeds original grammar",
                ))?;
            *slot = Some((child, next));
            length = length
                .checked_add(1)
                .ok_or(ArgumentResourceV1::Arithmetic)?;
            Ok(())
        };
        match expression {
            E::Symbol { symbol, .. } => visit(*symbol, budget)?,
            E::Constant { .. } | E::GlobalInvocation1d { .. } | E::Load(_) => {}
            E::Unary { operand, .. } | E::Cast { operand, .. } => push(operand)?,
            E::Binary { lhs, rhs, .. } | E::Compare { lhs, rhs, .. } => {
                push(rhs)?;
                push(lhs)?;
            }
            E::Select {
                condition,
                when_true,
                when_false,
                ..
            } => {
                push(when_false)?;
                push(when_true)?;
                push(condition)?;
            }
        }
    }
    Ok(())
}

fn visit_source_reserved_symbols_v18(
    declaration: &SemanticFunctionDeclV1,
    namespace: &SourceScalarNamespaceV18<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
    visit: &mut impl FnMut(u32, &mut ArgumentBudgetV1<'_>) -> SourceOwnedResultV18<()>,
) -> SourceOwnedResultV18<()> {
    for local in declaration.locals() {
        budget.charge_work(1)?;
        if let SemanticLocalRoleV1::Argument(argument) = local.role() {
            let symbol = PRODUCTION_KERNEL_SCALAR_SYMBOL_BASE_V2
                .checked_add(argument)
                .filter(|symbol| *symbol < fe2o3_pliron::PRODUCTION_SEMANTIC_LOAD_SYMBOL_BASE_V2)
                .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                    "source scalar argument namespace exhausted",
                ))?;
            visit(symbol, budget)?;
        }
    }
    let SourceScalarNamespaceV18::Ranked(recipe) = namespace else {
        return Ok(());
    };
    for block in recipe.blocks() {
        budget.charge_work(1)?;
        for operation in block.operations() {
            budget.charge_work(1)?;
            match operation {
                ProductionRankedOperationV1::SemanticSymbol { symbol, .. } => {
                    visit(*symbol, budget)?
                }
                ProductionRankedOperationV1::SemanticExpression { expression, .. } => {
                    visit_source_expression_symbols_v18(expression, budget, visit)?;
                }
                _ => {}
            }
        }
    }
    Ok(())
}

// The low lane is disjoint from every root-argument and ranked-load symbol.
// Existing public expression symbols in that lane are excluded by the exact
// recipe census. No caller supplies a start ID or a leaf-to-symbol mapping.
fn assign_source_leaf_symbols_v18(
    rows: &mut [SourceScalarLeafRowV18],
    reserved: &[u32],
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<()> {
    let mut next = 0u32;
    let mut cursor = 0usize;
    for row in rows {
        loop {
            budget.charge_work(2)?;
            if next >= PRODUCTION_KERNEL_SCALAR_SYMBOL_BASE_V2 {
                return Err(ProductionSourceOwnedViewErrorV18::Binding(
                    "source-only scalar leaf namespace exhausted",
                ));
            }
            while cursor < reserved.len() {
                budget.charge_work(1)?;
                if reserved[cursor] >= next {
                    break;
                }
                cursor = cursor
                    .checked_add(1)
                    .ok_or(ArgumentResourceV1::Arithmetic)?;
            }
            if reserved.get(cursor).copied() != Some(next) {
                break;
            }
            next = next.checked_add(1).ok_or(ArgumentResourceV1::Arithmetic)?;
        }
        row.symbol = next;
        next = next.checked_add(1).ok_or(ArgumentResourceV1::Arithmetic)?;
    }
    Ok(())
}

include!("production_source_leaf_lookup_work_v18.rs");
include!("production_source_wrapping_value_v23.rs");
include!("production_source_scalar_boundaries_v31.rs");
include!("production_source_issued_presence_v31.rs");

impl<'relation, 'source> SourceScalarLeavesV18<'relation, 'source> {
    fn observe_custody(&self, budget: &ArgumentBudgetV1<'_>) -> SourceOwnedResultV18<()> {
        self.relation.observe_custody(budget)?;
        if budget.storage() < self.floor {
            self.relation.source.cleanup.deny_refund();
            return Err(ArgumentResourceV1::Accounting.into());
        }
        Ok(())
    }

    fn query(&self, budget: &mut ArgumentBudgetV1<'_>) -> SourceOwnedResultV18<()> {
        self.relation.retain_query((|| {
            self.observe_custody(budget)?;
            self.relation.query(budget)
        })())
    }

    fn build(
        relation: &'relation ProductionSourceCorrespondenceV18<'source>,
        root: usize,
        namespace: &SourceScalarNamespaceV18<'_>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<Self> {
        relation.retain_query((|| {
            relation.query(budget)?;
            let semantic = relation.source.source_semantic(budget)?;
            let (source_function, physical) = relation.source.root(root, budget)?;
            let declaration = semantic.functions().get(source_function.index() as usize)
                .ok_or(ProductionSourceOwnedViewErrorV18::Binding("scalar leaf original root"))?;
            let function = relation.inventory.functions().get(physical)
                .ok_or(ProductionSourceOwnedViewErrorV18::Binding("scalar leaf physical root"))?;
            budget.charge_work(1)?;
            if let SourceScalarNamespaceV18::Ranked(recipe) = namespace {
                if recipe.function_name() != function.function.id.as_str() {
                    return relation.source.missing("scalar leaf ranked root differs");
                }
            }
            // This only reserves names against collision. Ranked/source
            // equivalence is checked by the containing consumer, not here.
            budget.reserve_storage(argument_sum_v1(&[
                size_of::<Self>(), size_of::<Vec<u32>>(),
                size_of::<[Option<(&ProductionSemanticExpressionV2, usize)>;
                    MAX_PRODUCTION_SEMANTIC_EXPRESSION_DEPTH_V2 * 2 + 1]>(),
                source_leaf_lookup_work_headers_v18()?,
            ])?)?;
            let mut count = 0usize;
            visit_source_reserved_symbols_v18(declaration, namespace, budget, &mut |_, budget| {
                budget.charge_work(1)?;
                count = count.checked_add(1).ok_or(ArgumentResourceV1::Arithmetic)?;
                Ok(())
            })?;
            let mut reserved = emission_vec_v1(count, budget).map_err(source_emission_error_v18)?;
            visit_source_reserved_symbols_v18(declaration, namespace, budget, &mut |symbol, budget| {
                budget.charge_work(1)?;
                if reserved.len() >= count || reserved.len() == reserved.capacity() {
                    return relation.source.missing("scalar symbol census changed between passes");
                }
                reserved.push(symbol);
                Ok(())
            })?;
            if reserved.len() != count { return relation.source.missing("scalar symbol census is incomplete"); }
            private_array_heapsort_v1(&mut reserved, |symbol| [*symbol as usize],
                &mut SourceCorrespondenceWorkV18(budget), || ArgumentResourceV1::Arithmetic.into())?;
            let root_row = relation.source.root_row(root)?;
            let mut capacity = 0usize;
            for source in &root_row.coordinates.sources.rows {
                budget.charge_work(1)?;
                let Some(sidecar) = relation.source.optional_sidecar(root, source.instance.index(), budget)? else { continue; };
                let anchors = sidecar.scoped_memory_anchors.as_ref()
                    .ok_or(ProductionSourceOwnedViewErrorV18::Binding("scalar leaf original memory census"))?;
                for anchor in &anchors.rows {
                    budget.charge_work(1)?;
                    if source_scalar_read_capture_v22(anchors, anchor,
                        matches!(namespace, SourceScalarNamespaceV18::PrivateSourceWritesV22))?.is_some() {
                        capacity = capacity.checked_add(1).ok_or(ArgumentResourceV1::Arithmetic)?;
                    }
                }
            }
            let mut rows = emission_vec_v1(capacity, budget).map_err(source_emission_error_v18)?;
            for source in &root_row.coordinates.sources.rows {
                budget.charge_work(1)?;
                let instance = source.instance.index();
                let Some(sidecar) = relation.source.optional_sidecar(root, instance, budget)? else { continue; };
                let anchors = sidecar.scoped_memory_anchors.as_ref()
                    .ok_or(ProductionSourceOwnedViewErrorV18::Binding("scalar leaf original memory census"))?;
                for (anchor_index, anchor) in anchors.rows.iter().enumerate() {
                    budget.charge_work(1)?;
                    let Some((read, typed_private)) = source_scalar_read_capture_v22(anchors, anchor,
                        matches!(namespace, SourceScalarNamespaceV18::PrivateSourceWritesV22))?
                        else { continue; };
                    let original = semantic.functions().get(source.function.index() as usize)
                        .ok_or(ProductionSourceOwnedViewErrorV18::Binding("scalar leaf original instance function"))?;
                    let place = match scoped_source_operand_v29(original, read.site, read.role) {
                        Some(SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place)) => Some(place),
                        _ => scoped_source_place_v29(original, read.site, read.role),
                    }.ok_or(ProductionSourceOwnedViewErrorV18::Binding("scalar leaf original place"))?;
                    budget.charge_work(4)?;
                    // Intermediate descriptor/pointer reads are not whole
                    // scalar operand leaves and cannot replace their suffix.
                    if read.prefix as usize != place.projections().len() { continue; }
                    if read.ty != place.ty() { return relation.source.missing("scalar leaf original type differs"); }
                    let Some(SemanticTypeShapeV1::Scalar(_)) = semantic.types().get(read.ty.index() as usize)
                        .map(SemanticTypeDeclV1::shape) else { continue; };
                    let lowered = lower_scalar_type(semantic.types(), read.ty).map_err(source_emission_error_v18)?;
                    let Some(scalar) = kir_semantic_scalar_v1(&lowered) else { continue; };
                    let key = TileAttachmentKeyV29 {
                        root, family: TileAttachmentFamilyV29::MemoryAnchor, instance, row: anchor_index,
                        field: TileAttachmentFieldV29::MemoryPosition, component: 0, part: 0,
                    };
                    let [position] = relation.attachment_range(key, budget)? else {
                        return relation.source.missing("scalar leaf source position census");
                    };
                    let ProductionSourceOperationV18::Operation(operation) =
                        relation.mapped_source_operation(position.location, budget)? else {
                            return relation.source.missing("scalar leaf has no actual emitted Load");
                        };
                    let value = source_scalar_read_value_v22(relation, root, instance,
                        anchor_index, anchor, operation, read, typed_private, budget)?;
                    let actual = function.function.body.as_ref()
                        .and_then(|body| body.blocks.get(operation.block.block as usize))
                        .and_then(|block| block.operations.get(operation.operation as usize))
                        .ok_or(ProductionSourceOwnedViewErrorV18::Binding("scalar leaf actual Load"))?;
                    budget.charge_work(4)?;
                    if operation.block.function != function.coordinate
                        || !matches!(actual.results.as_slice(), [result] if result.id == value && result.ty == lowered)
                        || !source_scalar_read_kind_v22(&actual.kind, typed_private)
                    {
                        return relation.source.missing("scalar leaf physical type or Load changed");
                    }
                    let occurrences = relation.source.source_ssa(budget)?.occurrences_v1()
                        .and_then(|view| view.function(source.function))
                        .ok_or(ProductionSourceOwnedViewErrorV18::Binding("scalar leaf original occurrences"))?;
                    check_scoped_payload_occurrence_v29(&occurrences, read.site, read.role,
                        place, read.occurrence, budget).map_err(source_emission_error_v18)?;
                    if rows.len() >= capacity || rows.len() == rows.capacity() {
                        return relation.source.missing("scalar leaf census exceeds its paid rows");
                    }
                    rows.push(SourceScalarLeafRowV18 { instance, anchor: anchor_index, place,
                        read: *read, operation, value, scalar, symbol: 0, typed_private });
                }
            }
            private_array_heapsort_v1(&mut rows, source_leaf_original_order_v18,
                &mut SourceCorrespondenceWorkV18(budget), || ArgumentResourceV1::Arithmetic.into())?;
            for pair in rows.windows(2) {
                budget.charge_work(3)?;
                if source_leaf_original_order_v18(&pair[0]) == source_leaf_original_order_v18(&pair[1]) {
                    return relation.source.missing("one source scalar occurrence has multiple emitted leaves");
                }
            }
            assign_source_leaf_symbols_v18(&mut rows, &reserved, budget)?;
            let count = argument_product_v1(rows.len(), 3)?;
            let mut lookup = emission_vec_v1(count, budget).map_err(source_emission_error_v18)?;
            for (row, leaf) in rows.iter().enumerate() {
                budget.charge_work(3)?;
                if lookup.capacity().checked_sub(lookup.len()).is_none_or(|remaining| remaining < 3) {
                    return Err(ArgumentResourceV1::Accounting.into());
                }
                lookup.push(SourceScalarLeafLookupV18 { key: [0, leaf.instance, leaf.place as usize], row });
                lookup.push(SourceScalarLeafLookupV18 { key: [1, leaf.value.0 as usize, 0], row });
                lookup.push(SourceScalarLeafLookupV18 { key: [2, leaf.symbol as usize, 0], row });
            }
            source_leaf_lookup_sort_v18(&mut lookup, budget)?;
            for pair in lookup.windows(2) {
                budget.charge_work(3)?;
                if pair[0].key == pair[1].key { return relation.source.missing("scalar leaf lookup is ambiguous"); }
            }
            // All temporary census credit remains in the containing checked
            // scope until its concrete owners have been destroyed.
            let boundaries = if matches!(namespace, SourceScalarNamespaceV18::PrivateSourceWritesV22) {
                source_scalar_boundaries_v31(relation, root, &rows, budget)?
            } else { SourceScalarBoundariesV31::empty() };
            drop(reserved);
            let ordinary_values = matches!(namespace,
                SourceScalarNamespaceV18::PrivateSourceWritesV22
                | SourceScalarNamespaceV18::OriginalSourceExpressionsV23);
            let wrapping = if ordinary_values {
                source_wrapping_values_v23(relation, root, budget)?
            } else { Vec::new() };
            let presences = if matches!(namespace, SourceScalarNamespaceV18::PrivateSourceWritesV22) {
                source_issued_presences_v31(relation, root, &rows, &boundaries, budget)?
            } else { SourceIssuedPresencesV31::empty() };
            let lengths = if ordinary_values {
                slice_view_v1::source_descriptor_lengths_v40(relation, root, &rows, &boundaries, &presences, budget)?
            } else { slice_view_v1::SourceDescriptorLengthsV40::empty() };
            let leaves = Self { relation, root, rows, lookup, wrapping, boundaries, presences, lengths,
                floor: budget.storage(), ordinary_values };
            leaves.check_boundary_equations_v31(budget)?;
            Ok(leaves)
        })())
    }

    fn find(
        &self,
        key: [usize; 3],
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<Option<&SourceScalarLeafRowV18>> {
        self.relation.retain_query((|| {
            self.query(budget)?;
            let first = if key[0] == 0 {
                source_leaf_address_partition_v18(&self.lookup, key, budget)?
            } else {
                private_array_partition_v1(
                    &self.lookup,
                    |entry| entry.key,
                    key,
                    false,
                    &mut SourceCorrespondenceWorkV18(budget),
                )?
            };
            budget.charge_work(2)?;
            let Some(entry) = self.lookup.get(first).filter(|entry| entry.key == key) else {
                return Ok(None);
            };
            let row =
                self.rows
                    .get(entry.row)
                    .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                        "scalar leaf lookup row differs",
                    ))?;
            let expected = match key[0] {
                0 => [0, row.instance, row.place as usize],
                1 => [1, row.value.0 as usize, 0],
                2 => [2, row.symbol as usize, 0],
                _ => {
                    return self
                        .relation
                        .source
                        .missing("scalar leaf lookup has an unknown key family");
                }
            };
            if key != expected {
                return self
                    .relation
                    .source
                    .missing("scalar leaf lookup changed its original identity");
            }
            let symbol_key = [2, row.symbol as usize, 0];
            let symbol = private_array_partition_v1(
                &self.lookup,
                |entry| entry.key,
                symbol_key,
                false,
                &mut SourceCorrespondenceWorkV18(budget),
            )?;
            budget.charge_work(2)?;
            if !self
                .lookup
                .get(symbol)
                .is_some_and(|symbol| symbol.key == symbol_key && symbol.row == entry.row)
            {
                return self
                    .relation
                    .source
                    .missing("scalar leaf changed its private symbol assignment");
            }
            let anchors = self
                .relation
                .source
                .sidecar(self.root, row.instance, budget)?
                .scoped_memory_anchors
                .as_ref()
                .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                    "scalar leaf original anchor census",
                ))?;
            let anchor =
                anchors
                    .rows
                    .get(row.anchor)
                    .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                        "scalar leaf original anchor",
                    ))?;
            let value = source_scalar_read_value_v22(
                self.relation,
                self.root,
                row.instance,
                row.anchor,
                anchor,
                row.operation,
                &row.read,
                row.typed_private,
                budget,
            )?;
            budget.charge_work(4)?;
            if value != row.value {
                return self
                    .relation
                    .source
                    .missing("scalar leaf changed its actual occurrence or result");
            }
            Ok(Some(row))
        })())
    }

    fn original_place(
        &self,
        instance: usize,
        function: &SemanticFunctionDeclV1,
        place: &SemanticPlaceV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<Option<ProductionSemanticExpressionV2>> {
        self.relation.retain_query((|| {
            self.query(budget)?;
            let semantic = self.relation.source.source_semantic(budget)?;
            let (original, _) = self.relation.source.instance(self.root, instance, budget)?;
            budget.charge_work(2)?;
            if !semantic
                .functions()
                .get(original.index() as usize)
                .is_some_and(|original| std::ptr::eq(original, function))
            {
                return self
                    .relation
                    .source
                    .missing("scalar leaf substituted source function");
            }
            let Some(row) = self.find(
                [0, instance, place as *const SemanticPlaceV1 as usize],
                budget,
            )?
            else {
                return Ok(None);
            };
            let exact = match scoped_source_operand_v29(function, row.read.site, row.read.role) {
                Some(SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place)) => {
                    Some(place)
                }
                _ => scoped_source_place_v29(function, row.read.site, row.read.role),
            };
            budget.charge_work(4)?;
            if !exact.is_some_and(|original| std::ptr::eq(original, place))
                || row.read.ty != place.ty()
                || row.read.prefix as usize != place.projections().len()
            {
                return self
                    .relation
                    .source
                    .missing("scalar leaf substituted source occurrence");
            }
            Ok(Some(ProductionSemanticExpressionV2::Symbol {
                symbol: row.symbol,
                scalar: row.scalar,
            }))
        })())
    }

    fn actual_value(
        &self,
        function: &Function,
        value: ValueId,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<Option<NormalizedScalarExpressionV1>> {
        self.relation.retain_query((|| {
            self.query(budget)?;
            let physical = self.relation.source.root(self.root, budget)?.1;
            let original = self.relation.inventory.functions().get(physical).ok_or(
                ProductionSourceOwnedViewErrorV18::Binding("scalar leaf physical root"),
            )?;
            budget.charge_work(1)?;
            if !std::ptr::eq(original.function, function) {
                return self
                    .relation
                    .source
                    .missing("scalar leaf substituted physical root");
            }
            if !self.lengths.is_empty() {
                if let Some(length) = self.descriptor_length_value_v40(value, budget)? {
                    return Ok(Some(length));
                }
            }
            if let Some(row) = if self.presences.rows.is_empty() {
                None
            } else {
                self.presence_value_v31(value, budget)?
            } {
                return Ok(Some(NormalizedScalarExpressionV1::Symbol {
                    symbol: row.symbol,
                    scalar: ProductionSemanticScalarTypeV2::Bool,
                }));
            }
            if let Some(row) = self.boundary_find_v31([1, value.0 as usize, 0, 0], budget)? {
                return Ok(Some(NormalizedScalarExpressionV1::Symbol {
                    symbol: row.symbol,
                    scalar: row.scalar,
                }));
            }
            if let Some(row) = self.boundary_find_v31([4, value.0 as usize, 0, 0], budget)? {
                return Ok(Some(NormalizedScalarExpressionV1::Symbol {
                    symbol: row.symbol,
                    scalar: row.scalar,
                }));
            }
            let Some(row) = self.find([1, value.0 as usize, 0], budget)? else {
                return Ok(None);
            };
            let definition = self
                .relation
                .inventory
                .definition_for_value(original.coordinate, value, budget)
                .map_err(source_pointer_inventory_error_v18)?
                .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                    "scalar leaf actual definition missing",
                ))?;
            budget.charge_work(2)?;
            if definition.coordinate
                != (fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1::Result {
                    operation: row.operation,
                    result: 0,
                })
            {
                return self
                    .relation
                    .source
                    .missing("scalar leaf actual definition changed");
            }
            let anchors = self
                .relation
                .source
                .sidecar(self.root, row.instance, budget)?
                .scoped_memory_anchors
                .as_ref()
                .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                    "scalar leaf original memory census",
                ))?;
            budget.charge_work(3)?;
            if !anchors
                .rows
                .get(row.anchor)
                .map(|anchor| source_scalar_read_capture_v22(anchors, anchor, row.typed_private))
                .transpose()?
                .flatten()
                .is_some_and(|(read, typed)| *read == row.read && typed == row.typed_private)
            {
                return self
                    .relation
                    .source
                    .missing("scalar leaf changed original read capture");
            }
            Ok(Some(NormalizedScalarExpressionV1::Symbol {
                symbol: row.symbol,
                scalar: row.scalar,
            }))
        })())
    }
}

fn source_scalar_read_store_v18(
    relation: &ProductionSourceCorrespondenceV18<'_>,
    root: usize,
    access: &SourcePhysicalAccessV18<'_>,
    payload: &SourcePhysicalPayloadV18<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<Option<SourceScalarReadTransportV18>> {
    relation.retain_query((|| {
        relation.query(budget)?;
        if !matches!(&access.anchor.kind, ScopedMemoryAnchorKindV29::Access { payload: Some(original), .. }
            if std::ptr::eq(original, payload.source)) {
            return relation.source.missing("read transport changed its retained source capture");
        }
        let ScopedMemoryPayloadV29::Store { source: ScopedMemoryStoreSourceV29::Operand {
            site, role, ty, source: ScopedMemoryOperandSourceV29::Memory { occurrence, access: original_load },
        }, .. } = payload.source else { return Ok(None); };
        let source = relation.source.source_semantic(budget)?;
        let (original_function, _) = relation.source.instance(root, access.instance, budget)?;
        budget.charge_work(4)?;
        let declaration = source.functions().get(original_function.index() as usize)
            .ok_or(ProductionSourceOwnedViewErrorV18::Binding("read transport original function"))?;
        let Some(SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place)) =
            scoped_source_operand_v29(declaration, *site, *role)
        else {
            return relation.source.missing("read transport does not identify its original place operand");
        };
        if place.ty() != *ty {
            return relation.source.missing("read transport source type differs");
        }
        let physical = relation.source.root(root, budget)?.1;
        let function = relation.inventory.functions().get(physical)
            .ok_or(ProductionSourceOwnedViewErrorV18::Binding("read transport physical function"))?;
        let value = payload.value;
        let definition = relation.inventory.definition_for_value(function.coordinate, value, budget)
            .map_err(source_pointer_inventory_error_v18)?
            .ok_or(ProductionSourceOwnedViewErrorV18::Binding("read transport value definition"))?;
        let fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1::Result { operation, result } =
            definition.coordinate else {
                return relation.source.missing("read transport does not use its actual Load result");
            };
        budget.charge_work(4)?;
        let actual = function.function.body.as_ref()
            .and_then(|body| body.blocks.get(operation.block.block as usize))
            .and_then(|block| block.operations.get(operation.operation as usize))
            .ok_or(ProductionSourceOwnedViewErrorV18::Binding("read transport actual operation"))?;
        let pointer = match actual.kind {
            OperationKind::Load { pointer, .. } | OperationKind::GuardedLoad { pointer, .. } => pointer,
            _ => return relation.source.missing("read transport does not use its actual Load result"),
        };
        if operation.block.function != function.coordinate || result != 0 {
            return relation.source.missing("read transport result belongs to another function");
        }
        let load = relation.retained_memory_access(root, operation, pointer, budget)?
            .ok_or(ProductionSourceOwnedViewErrorV18::Binding("read transport lacks original Load capture"))?;
        let loaded = relation.retained_scalar_payload_v18(root, operation, &load, budget)?
            .ok_or(ProductionSourceOwnedViewErrorV18::Binding("read transport lacks Load value capture"))?;
        let ScopedMemoryPayloadV29::Load { read, .. } = loaded.source else {
            return relation.source.missing("read transport origin is not a captured Load");
        };
        let Some(fe2o3_kernel_ir::CanonicalKirUseCoordinateV1::OperationOperand { operation: store, .. }) =
            payload.store_use else {
                return relation.source.missing("read transport lacks its actual Store use");
            };
        budget.charge_work(12)?;
        if load.instance != access.instance || load.row != *original_load
            || load.row >= access.row || loaded.value != value
            || operation.block != store.block || operation.operation >= store.operation
            || read.site != *site || read.role != *role || read.ty != *ty
            || read.prefix as usize != place.projections().len() || read.occurrence != *occurrence
        {
            return relation.source.missing("stored scalar Load differs from its original operand occurrence");
        }
        Ok(Some(SourceScalarReadTransportV18 {
            instance: load.instance, operation, value, read: *read,
        }))
    })())
}

// This checks one existing scalar-literal grammar. None means another source
// payload arm still needs its expression/call/read-from relation, never success.
fn source_scalar_literal_store_v18(
    relation: &ProductionSourceCorrespondenceV18<'_>,
    root: usize,
    access: &SourcePhysicalAccessV18<'_>,
    payload: &SourcePhysicalPayloadV18<'_>,
    origins: &value_origin_v1::WholeValueOriginsV18<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<Option<Constant>> {
    relation.retain_query((|| {
        relation.query(budget)?;
        if !matches!(&access.anchor.kind, ScopedMemoryAnchorKindV29::Access { payload: Some(original), .. }
            if std::ptr::eq(original, payload.source)) {
            return relation.source.missing("literal payload changed its retained source capture");
        }
        let ScopedMemoryPayloadV29::Store { source: ScopedMemoryStoreSourceV29::Operand {
            site, role, ty, source: ScopedMemoryOperandSourceV29::Constant,
        }, .. } = payload.source else { return Ok(None); };
        let source = relation.source.source_semantic(budget)?;
        let (function, _) = relation.source.instance(root, access.instance, budget)?;
        budget.charge_work(4)?;
        let declaration = source.functions().get(function.index() as usize)
            .ok_or(ProductionSourceOwnedViewErrorV18::Binding("literal source function"))?;
        let Some(SemanticOperandV1::Constant(original)) = scoped_source_operand_v29(declaration, *site, *role) else {
            return relation.source.missing("literal payload does not identify its original constant operand");
        };
        if original.ty() != *ty {
            return relation.source.missing("literal payload source type differs");
        }
        let SemanticConstantValueV1::Scalar(value) = original.value() else { return Ok(None); };
        let expected = lower_constant(lower_scalar_type(source.types(), *ty).map_err(source_emission_error_v18)?, *value)
            .map_err(source_emission_error_v18)?;
        source_store_constant_value_v18(relation, root, payload.value, expected, origins, budget)
            .map(Some)
    })())
}

// A promoted use is matched to its original SSA definition, not to an earlier
// assignment to the same local. Nonliteral/phi/entry definitions use other
// expression or ABI relations and are deliberately not certified here.
fn source_promoted_literal_store_v18(
    relation: &ProductionSourceCorrespondenceV18<'_>,
    root: usize,
    access: &SourcePhysicalAccessV18<'_>,
    payload: &SourcePhysicalPayloadV18<'_>,
    origins: &value_origin_v1::WholeValueOriginsV18<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<Option<Constant>> {
    relation.retain_query((|| {
        relation.query(budget)?;
        if !matches!(&access.anchor.kind, ScopedMemoryAnchorKindV29::Access { payload: Some(original), .. }
            if std::ptr::eq(original, payload.source)) {
            return relation.source.missing("promoted literal changed its retained source capture");
        }
        let ScopedMemoryPayloadV29::Store { source: ScopedMemoryStoreSourceV29::Operand {
            site, role, ty, source: ScopedMemoryOperandSourceV29::Place(
                ScopedMemoryOccurrenceV29::Promoted { event, definition }),
        }, .. } = payload.source else { return Ok(None); };
        let source = relation.source.source_semantic(budget)?;
        let (function, _) = relation.source.instance(root, access.instance, budget)?;
        budget.charge_work(4)?;
        let declaration = source.functions().get(function.index() as usize)
            .ok_or(ProductionSourceOwnedViewErrorV18::Binding("promoted literal original function"))?;
        let Some(SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place)) =
            scoped_source_operand_v29(declaration, *site, *role)
        else { return relation.source.missing("promoted literal original operand"); };
        if place.ty() != *ty {
            return relation.source.missing("promoted literal original type");
        }
        if !place.projections().is_empty() { return Ok(None); }
        let owner = relation.source.source_ssa(budget)?;
        budget.charge_work(8)?;
        let occurrences = owner.occurrences_v1().and_then(|view| view.function(function))
            .ok_or(ProductionSourceOwnedViewErrorV18::Binding("promoted literal original occurrences"))?;
        let occurrence = occurrences.events().get(*event)
            .ok_or(ProductionSourceOwnedViewErrorV18::Binding("promoted literal use occurrence"))?;
        let variable = fe2o3_mir_model::SsaVariableIdV1::new(place.local().index());
        if !occurrence.is_reachable() || !occurrence.is_promoted()
            || occurrence.site() != *site || occurrence.operand() != *role
            || occurrence.role() != ExecutionEventV29::BaseUse
            || occurrence.resolved() != Some(SsaResolvedEventV1::Use { variable, value: *definition })
        {
            return relation.source.missing("promoted literal changed current SSA definition");
        }
        if !matches!(definition, SsaValueV1::Definition(_)) { return Ok(None); }
        let mut defining = None;
        for candidate in occurrences.events() {
            budget.charge_work(4)?;
            if candidate.resolved() != Some(SsaResolvedEventV1::Define { variable, value: *definition }) {
                continue;
            }
            if !candidate.is_reachable() || !candidate.is_promoted() || defining.replace(candidate).is_some() {
                return relation.source.missing("promoted literal has no unique original definition");
            }
        }
        let Some(defining) = defining else { return Ok(None); };
        budget.charge_work(5)?;
        let Some(SemanticStatementKindV1::Assign(assignment)) =
            scoped_source_statement_v29(declaration, defining.site())
        else { return Ok(None); };
        if assignment.destination().local() != place.local()
            || !assignment.destination().projections().is_empty()
            || assignment.destination().ty() != *ty
        { return relation.source.missing("promoted literal definition source place"); }
        let SemanticRvalueKindV1::Use(SemanticOperandV1::Constant(original)) = assignment.value().kind()
            else { return Ok(None); };
        if original.ty() != *ty { return relation.source.missing("promoted literal definition type"); }
        let SemanticConstantValueV1::Scalar(value) = original.value() else { return Ok(None); };
        let expected = lower_constant(lower_scalar_type(source.types(), *ty).map_err(source_emission_error_v18)?, *value)
            .map_err(source_emission_error_v18)?;
        source_store_constant_value_v18(relation, root, payload.value, expected, origins, budget)
            .map(Some)
    })())
}

fn source_store_constant_value_v18(
    relation: &ProductionSourceCorrespondenceV18<'_>,
    root: usize,
    stored: ValueId,
    expected: Constant,
    origins: &value_origin_v1::WholeValueOriginsV18<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<Constant> {
    let physical = relation.source.root(root, budget)?.1;
    let function = relation.inventory.functions().get(physical).ok_or(
        ProductionSourceOwnedViewErrorV18::Binding("literal physical function"),
    )?;
    if !origins.belongs_to(relation.inventory, function.coordinate) {
        return relation
            .source
            .missing("literal whole-value analysis changed original owner");
    }
    let inventory_error = |error| match error {
        fe2o3_kernel_analysis::CanonicalKirInventoryErrorV1::Resource(error) => error.into(),
        fe2o3_kernel_analysis::CanonicalKirInventoryErrorV1::InconsistentOwner => {
            ProductionSourceOwnedViewErrorV18::Binding("literal inventory association")
        }
    };
    let value = origins
        .operation_origin(stored, budget)
        .map_err(inventory_error)?
        .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
            "literal stored value has no unique operation origin",
        ))?;
    let definition = relation
        .inventory
        .definition_for_value(function.coordinate, value, budget)
        .map_err(inventory_error)?
        .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
            "literal value definition missing",
        ))?;
    let fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1::Result { operation, result } =
        definition.coordinate
    else {
        return relation
            .source
            .missing("literal value is not an operation result");
    };
    budget.charge_work(4)?;
    let actual = function
        .function
        .body
        .as_ref()
        .and_then(|body| body.blocks.get(operation.block.block as usize))
        .and_then(|block| block.operations.get(operation.operation as usize))
        .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
            "literal actual operation",
        ))?;
    if operation.block.function != function.coordinate
        || result != 0
        || !matches!(&actual.kind, OperationKind::Constant(constant) if *constant == expected)
        || !matches!(actual.results.as_slice(), [value] if value.ty == expected.ty())
    {
        return relation
            .source
            .missing("stored scalar literal differs from its original typed source");
    }
    Ok(expected)
}

impl SourceMemoryBindingV18 {
    fn bind(&mut self, key: TileAttachmentKeyV29) -> SourceOwnedResultV18<()> {
        if self
            .instance
            .is_some_and(|instance| instance != key.instance)
        {
            return Err(ProductionSourceOwnedViewErrorV18::Binding(
                "source effect has conflicting instance owners",
            ));
        }
        self.instance = Some(key.instance);
        let slot = match key.family {
            TileAttachmentFamilyV29::InstanceSpans => &mut self.span,
            TileAttachmentFamilyV29::Lifecycle => &mut self.lifecycle,
            TileAttachmentFamilyV29::Assertion => &mut self.failure,
            TileAttachmentFamilyV29::TerminalFailure => &mut self.terminal,
            _ => {
                return Err(ProductionSourceOwnedViewErrorV18::Binding(
                    "source effect attachment family",
                ));
            }
        };
        if slot.replace(key.row).is_some() {
            return Err(ProductionSourceOwnedViewErrorV18::Binding(
                "source effect has duplicate source occurrences",
            ));
        }
        Ok(())
    }
}

fn source_effect_key_v18(row: &SourceMemoryEffectV18) -> [usize; 3] {
    [
        row.coordinate.block.block as usize,
        row.coordinate.operation as usize,
        row.ordinal as usize,
    ]
}

fn bind_source_effect_operation_v18(
    effects: &[SourceMemoryEffectV18],
    bindings: &mut [SourceMemoryBindingV18],
    expected_function: usize,
    coordinate: fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1,
    key: TileAttachmentKeyV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<()> {
    if coordinate.block.function.0 as usize != expected_function {
        return Err(ProductionSourceOwnedViewErrorV18::Binding(
            "source effect attachment changed root",
        ));
    }
    let first = private_array_partition_v1(
        effects,
        source_effect_key_v18,
        [
            coordinate.block.block as usize,
            coordinate.operation as usize,
            0,
        ],
        false,
        &mut SourceCorrespondenceWorkV18(budget),
    )?;
    let last = private_array_partition_v1(
        effects,
        source_effect_key_v18,
        [
            coordinate.block.block as usize,
            coordinate.operation as usize,
            u32::MAX as usize,
        ],
        true,
        &mut SourceCorrespondenceWorkV18(budget),
    )?;
    let interval =
        bindings
            .get_mut(first..last)
            .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                "source effect binding interval",
            ))?;
    for binding in interval {
        budget.charge_work(1)?;
        binding.bind(key)?;
    }
    Ok(())
}

// A source-metadata join, not a graph index. Canonical operation order is reused
// directly; O(A log E + E) work and O(E) retained bindings for A attachments.
fn source_memory_bindings_v18(
    relation: &ProductionSourceCorrespondenceV18<'_>,
    root: usize,
    effects: &[SourceMemoryEffectV18],
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<Vec<SourceMemoryBindingV18>> {
    use TileAttachmentFamilyV29 as Family;
    use TileAttachmentFieldV29 as Field;
    relation.query(budget)?;
    let physical = relation.source.root_row(root)?.function_ordinal;
    for pair in effects.windows(2) {
        budget.charge_work(1)?;
        if source_effect_key_v18(&pair[0]) >= source_effect_key_v18(&pair[1]) {
            return relation
                .source
                .missing("source effects are not in unique canonical order");
        }
    }
    let mut bindings = emission_vec_v1(effects.len(), budget).map_err(source_emission_error_v18)?;
    budget.charge_work(effects.len())?;
    bindings.resize(effects.len(), SourceMemoryBindingV18::default());
    for row in relation.attachments {
        budget.charge_work(1)?;
        if row.key.root != root {
            continue;
        }
        match (row.key.family, row.key.field) {
            (Family::InstanceSpans, Field::Span)
            | (Family::Lifecycle, Field::LifecycleOperation)
            | (Family::TerminalFailure, Field::FailureCleanup | Field::FailureDiagnostic) => {
                if let ProductionSourceOperationV18::Operation(coordinate) =
                    relation.mapped_source_operation(row.location, budget)?
                {
                    bind_source_effect_operation_v18(
                        effects,
                        &mut bindings,
                        physical,
                        coordinate,
                        row.key,
                        budget,
                    )?;
                }
            }
            (Family::Assertion, Field::AssertFailureBlock) => {
                let TileAttachmentLocationV29::Origin(TileScalarSourceV29::Block {
                    function,
                    block,
                }) = row.location
                else {
                    return relation
                        .source
                        .missing("source assertion failure attachment is not a block");
                };
                if function != physical {
                    return relation
                        .source
                        .missing("source failure effect changed root");
                }
                let function_row = relation.inventory.functions().get(function).ok_or(
                    ProductionSourceOwnedViewErrorV18::Binding("source failure function"),
                )?;
                let block = function_row
                    .blocks
                    .start
                    .checked_add(block)
                    .filter(|index| *index < function_row.blocks.end)
                    .and_then(|index| relation.inventory.blocks().get(index))
                    .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                        "source failure block",
                    ))?;
                for operation in relation
                    .inventory
                    .operations()
                    .get(block.operations.clone())
                    .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                        "source failure operations",
                    ))?
                {
                    budget.charge_work(1)?;
                    bind_source_effect_operation_v18(
                        effects,
                        &mut bindings,
                        physical,
                        operation.coordinate,
                        row.key,
                        budget,
                    )?;
                }
            }
            _ => {}
        }
    }
    for binding in &bindings {
        budget.charge_work(1)?;
        if binding.instance.is_none() {
            return relation
                .source
                .missing("physical effect lacks an original source occurrence");
        }
    }
    Ok(bindings)
}

fn source_emission_error_v18(
    error: ProductionSemanticKirErrorV1,
) -> ProductionSourceOwnedViewErrorV18 {
    match error {
        ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(error) => error.into(),
        other => ProductionSourceOwnedViewErrorV18::Source(
            ProductionPendingScopedSourceErrorV29::Source(other),
        ),
    }
}

impl SourceMemoryEffectV18 {
    fn ranked_consumer(&self) -> Option<KirMemoryConsumerV1> {
        let memory_space = ranked_memory_space(self.concrete_space?)?;
        Some(KirMemoryConsumerV1 {
            location: self.location,
            operation_access_ordinal: self.ordinal,
            pointer: self.pointer,
            access: self.access,
            memory_space,
            atomic: self.atomic,
        })
    }
}

fn source_memory_effects_v18(
    relation: &ProductionSourceCorrespondenceV18<'_>,
    root: usize,
    spaces: &SourcePointerSpacesV18<'_, '_>,
    max_operations: usize,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<Vec<SourceMemoryEffectV18>> {
    relation.query(budget)?;
    let physical = relation.source.root_row(root)?.function_ordinal;
    let inventory = relation.inventory;
    source_memory_effects_endpoint_v18(
        relation,
        inventory,
        physical,
        spaces,
        max_operations,
        budget,
    )
}

fn optimized_source_memory_effects_v18(
    relation: &ProductionSourceCorrespondenceV18<'_>,
    optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
    root: usize,
    spaces: &SourcePointerSpacesV18<'_, '_>,
    max_operations: usize,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<Vec<SourceMemoryEffectV18>> {
    optimized_source_endpoints_v18(relation, optimized, budget)?;
    let physical = optimized_source_root_function_v18(relation, optimized, root, budget)?
        .coordinate
        .0 as usize;
    let inventory = optimized.output_inventory(budget)?;
    source_memory_effects_endpoint_v18(
        relation,
        inventory,
        physical,
        spaces,
        max_operations,
        budget,
    )
}

fn source_memory_effects_endpoint_v18(
    relation: &ProductionSourceCorrespondenceV18<'_>,
    inventory: &fe2o3_kernel_analysis::CanonicalKirInventoryV18<'_>,
    physical: usize,
    spaces: &SourcePointerSpacesV18<'_, '_>,
    max_operations: usize,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<Vec<SourceMemoryEffectV18>> {
    use fe2o3_kernel_ir::{
        KirLocalMemoryEffectRefV1 as Effect, StorageMemoryAccessKindV1 as StorageAccess,
    };
    if !std::ptr::eq(spaces.inventory, inventory) || spaces.function.0 as usize != physical {
        return relation
            .source
            .missing("source effect space analysis changed owner or root");
    }
    let function =
        inventory
            .functions()
            .get(physical)
            .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                "source effect function",
            ))?;
    let operations = inventory
        .operations()
        .get(function.operations.clone())
        .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
            "source effect operation interval",
        ))?;
    if operations.len() > max_operations {
        return relation.source.missing("source effect operation cap");
    }
    let mut count = 0usize;
    for effect in inventory.effects().get(function.effects.clone()).ok_or(
        ProductionSourceOwnedViewErrorV18::Binding("source effect inventory interval"),
    )? {
        budget.charge_work(1)?;
        if matches!(
            effect.effect,
            Effect::Read(_)
                | Effect::Write(_)
                | Effect::VolatileRead(_)
                | Effect::VolatileWrite(_)
                | Effect::Atomic { .. }
        ) {
            count = count.checked_add(1).ok_or(ArgumentResourceV1::Arithmetic)?;
        }
    }
    let mut rows = emission_vec_v1(count, budget).map_err(source_emission_error_v18)?;
    for operation in operations {
        budget.charge_work(1)?;
        let block = function
            .blocks
            .start
            .checked_add(operation.coordinate.block.block as usize)
            .filter(|index| *index < function.blocks.end)
            .and_then(|index| inventory.blocks().get(index))
            .filter(|row| row.coordinate == operation.coordinate.block)
            .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                "source effect block",
            ))?;
        let location =
            FunctionOperationLocation::new(block.block.id, operation.coordinate.operation as usize);
        let before = rows.len();
        let storage_operation = matches!(operation.operation.kind, OperationKind::Storage(_));
        let mut visit = |pointer, access, static_space, atomic| -> SourceOwnedResultV18<()> {
            budget.charge_work(1)?;
            let concrete_space = spaces.concrete_space(pointer, budget)?;
            if static_space != AddressSpace::Generic && concrete_space != Some(static_space) {
                return relation
                    .source
                    .missing("source access differs from its pointer space");
            }
            if rows.len() >= count || rows.len() == rows.capacity() {
                return relation
                    .source
                    .missing("source effect census grew during traversal");
            }
            let ordinal = u32::try_from(
                rows.len()
                    .checked_sub(before)
                    .ok_or(ArgumentResourceV1::Accounting)?,
            )
            .map_err(|_| ArgumentResourceV1::Arithmetic)?;
            rows.push(SourceMemoryEffectV18 {
                coordinate: operation.coordinate,
                location,
                ordinal,
                pointer,
                access,
                static_space,
                concrete_space,
                atomic,
                storage_operation,
            });
            Ok(())
        };
        match &operation.operation.kind {
            OperationKind::Storage(storage) => {
                storage.try_visit_memory_accesses(|pointer, access, kind| {
                    visit(
                        pointer,
                        match kind {
                            StorageAccess::Read => dialect_kernel::AccessKindAttr::Read,
                            StorageAccess::Write => dialect_kernel::AccessKindAttr::Write,
                        },
                        access.address_space,
                        None,
                    )
                })?
            }
            _ => try_visit_kir_memory_accesses_with_space_v18(operation.operation, &mut visit)?,
        }
        drop(visit);
        let mut expected = 0usize;
        for effect in inventory.effects().get(operation.effects.clone()).ok_or(
            ProductionSourceOwnedViewErrorV18::Binding("source effect operation census"),
        )? {
            budget.charge_work(1)?;
            if matches!(
                effect.effect,
                Effect::Read(_)
                    | Effect::Write(_)
                    | Effect::VolatileRead(_)
                    | Effect::VolatileWrite(_)
                    | Effect::Atomic { .. }
            ) {
                expected = expected
                    .checked_add(1)
                    .ok_or(ArgumentResourceV1::Arithmetic)?;
            }
        }
        if rows.len().checked_sub(before) != Some(expected)
            || matches!(&operation.operation.kind, OperationKind::InlineAssembly(assembly) if !assembly.declared_effects.is_empty())
        {
            return relation
                .source
                .missing("source effect has no complete checked access reader");
        }
    }
    if rows.len() != count {
        return relation
            .source
            .missing("source physical memory effect census is incomplete");
    }
    Ok(rows)
}
// These labels preserve the original invocation. They are effect-correlation
// metadata, not legacy correspondence rows or physical-memory certificates.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct SourceEffectSiteV18 {
    instance: usize,
    function: SemanticFunctionIdV1,
    block: SemanticBlockIdV1,
    statement: Option<u32>,
    ordinal: u32,
}

#[derive(Clone, Copy)]
struct SourceEffectLocationV18 {
    site: SourceEffectSiteV18,
    physical: fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1,
    access: u32,
    ranked: (u32, u32),
}

#[derive(Clone, Copy)]
struct SourceFlowEventV18 {
    block: u32,
    order: u64,
    site: SourceEffectSiteV18,
}

fn source_flow_site_key_v18(site: SourceEffectSiteV18) -> [usize; 6] {
    [
        site.instance,
        site.function.index() as usize,
        site.block.index() as usize,
        usize::from(site.statement.is_some()),
        site.statement.unwrap_or(0) as usize,
        site.ordinal as usize,
    ]
}

fn source_flow_event_key_v18(event: &SourceFlowEventV18) -> [usize; 9] {
    let site = source_flow_site_key_v18(event.site);
    [
        event.block as usize,
        (event.order >> 32) as usize,
        (event.order as u32) as usize,
        site[0],
        site[1],
        site[2],
        site[3],
        site[4],
        site[5],
    ]
}

enum SourceFlowGraphV18<'a, 'g> {
    Canonical {
        inventory: &'a fe2o3_kernel_analysis::CanonicalKirInventoryV18<'g>,
        function: fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1,
    },
    Ranked(&'a fe2o3_pliron::ProductionRankedKernelV1),
}

impl SourceFlowGraphV18<'_, '_> {
    fn ordinal(&self, block: u32, ledger: &CorrelationLedgerV18<'_, '_, '_>) -> Option<usize> {
        match self {
            Self::Canonical {
                inventory,
                function,
            } => ledger
                .inventory(|budget| inventory.block_for_id(*function, BlockId(block), budget))?
                .map(|row| row.coordinate.block as usize),
            Self::Ranked(recipe) => {
                ledger.with_budget(|budget| budget.charge_work(1)).ok()?;
                recipe.blocks().get(block as usize).map(|_| block as usize)
            }
        }
    }

    fn successors(
        &self,
        block: u32,
        ledger: &CorrelationLedgerV18<'_, '_, '_>,
        mut visit: impl FnMut(u32) -> Option<()>,
    ) -> Option<()> {
        match self {
            Self::Canonical {
                inventory,
                function,
            } => {
                let block = ledger.inventory(|budget| {
                    inventory.block_for_id(*function, BlockId(block), budget)
                })??;
                let edges = inventory.edges().get(block.edges.clone())?;
                for edge in edges {
                    ledger.with_budget(|budget| budget.charge_work(1)).ok()?;
                    if edge.coordinate.source != block.coordinate
                        || edge.target.function != *function
                    {
                        ledger.inconsistent_inventory.set(true);
                        return None;
                    }
                    visit(edge.target_id.0)?;
                }
            }
            Self::Ranked(recipe) => {
                ledger.with_budget(|budget| budget.charge_work(1)).ok()?;
                let block = recipe.blocks().get(block as usize)?;
                let (successors, count) = ranked_terminator_successors_v18(block.terminator());
                for target in &successors[..count] {
                    ledger.with_budget(|budget| budget.charge_work(1)).ok()?;
                    if recipe.blocks().get(*target as usize).is_none() {
                        ledger.inconsistent_inventory.set(true);
                        return None;
                    }
                    visit(*target)?;
                }
            }
        }
        Some(())
    }
}

struct SourceFlowScratchV18 {
    visited: Vec<u8>,
    pending: VecDeque<u32>,
    pending_limit: usize,
    found: Vec<SourceEffectSiteV18>,
    event_limit: usize,
    entry: Vec<SourceEffectSiteV18>,
    next: Vec<(SourceEffectSiteV18, SourceEffectSiteV18)>,
    next_limit: usize,
    next_count: usize,
    recording: bool,
}

impl SourceFlowScratchV18 {
    fn new(
        blocks: usize,
        edges: usize,
        events: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<Self> {
        // Each block's outgoing edges are enqueued once, plus the start block's
        // initial edges. No runtime-sized graph or reverse CFG is constructed.
        let pending_limit = argument_sum_v1(&[argument_product_v1(edges, 2)?, 1])?;
        budget.reserve_storage(size_of::<Self>())?;
        let mut visited = emission_vec_v1(blocks, budget).map_err(source_emission_error_v18)?;
        budget.charge_work(blocks)?;
        visited.resize(blocks, 0);
        budget.reserve_storage(argument_product_v1(pending_limit, size_of::<u32>())?)?;
        let mut pending = VecDeque::new();
        pending
            .try_reserve_exact(pending_limit)
            .map_err(|_| ArgumentResourceV1::Allocation)?;
        budget.reserve_storage(argument_product_v1(
            pending
                .capacity()
                .checked_sub(pending_limit)
                .ok_or(ArgumentResourceV1::Accounting)?,
            size_of::<u32>(),
        )?)?;
        Ok(Self {
            visited,
            pending,
            pending_limit,
            found: emission_vec_v1(events, budget).map_err(source_emission_error_v18)?,
            event_limit: events,
            entry: emission_vec_v1(events, budget).map_err(source_emission_error_v18)?,
            next: Vec::new(),
            next_limit: 0,
            next_count: 0,
            recording: false,
        })
    }

    fn prepare_output(&mut self, budget: &mut ArgumentBudgetV1<'_>) -> SourceOwnedResultV18<()> {
        if self.recording || !self.next.is_empty() || self.next.capacity() != 0 {
            return Err(ProductionSourceOwnedViewErrorV18::Binding(
                "effect order output was already reserved",
            ));
        }
        // Count through the same walk before allocating. Reserving events^2
        // up front would reject large linear functions despite a linear output.
        self.next = emission_vec_v1(self.next_count, budget).map_err(source_emission_error_v18)?;
        self.next_limit = self.next_count;
        self.next_count = 0;
        self.entry.clear();
        self.recording = true;
        Ok(())
    }

    fn canonicalize(&mut self, budget: &mut ArgumentBudgetV1<'_>) -> SourceOwnedResultV18<()> {
        private_array_heapsort_v1(
            &mut self.entry,
            |site| source_flow_site_key_v18(*site),
            &mut SourceCorrespondenceWorkV18(budget),
            || ArgumentResourceV1::Arithmetic.into(),
        )?;
        private_array_heapsort_v1(
            &mut self.next,
            |(first, second)| {
                let first = source_flow_site_key_v18(*first);
                let second = source_flow_site_key_v18(*second);
                std::array::from_fn::<_, 12, _>(|index| {
                    if index < 6 {
                        first[index]
                    } else {
                        second[index - 6]
                    }
                })
            },
            &mut SourceCorrespondenceWorkV18(budget),
            || ArgumentResourceV1::Arithmetic.into(),
        )?;
        budget.charge_work(argument_sum_v1(&[
            argument_product_v1(self.entry.len(), 6)?,
            argument_product_v1(self.next.len(), 12)?,
        ])?)?;
        self.entry.dedup();
        self.next.dedup();
        Ok(())
    }
}

struct SourceEffectWalkV18<'a, 'g, 'b, 'w, 'c> {
    graph: SourceFlowGraphV18<'a, 'g>,
    events: &'a [SourceFlowEventV18],
    scratch: &'a mut SourceFlowScratchV18,
    ledger: &'a CorrelationLedgerV18<'b, 'w, 'c>,
}

impl EffectWalkV18 for SourceEffectWalkV18<'_, '_, '_, '_, '_> {
    type Site = SourceEffectSiteV18;

    fn first_event(&mut self, block: u32, after: Option<u64>) -> Option<Option<Self::Site>> {
        let mut first = 0usize;
        let mut end = self.events.len();
        while first < end {
            self.ledger
                .with_budget(|budget| budget.charge_work(3))
                .ok()?;
            let middle = first + (end - first) / 2;
            let row = self.events.get(middle)?;
            let before = row.block < block
                || (row.block == block && after.is_some_and(|after| row.order <= after));
            if before {
                first = middle + 1;
            } else {
                end = middle;
            }
        }
        self.ledger
            .with_budget(|budget| budget.charge_work(2))
            .ok()?;
        Some(
            self.events
                .get(first)
                .filter(|row| row.block == block)
                .map(|row| row.site),
        )
    }

    fn extend_successors(&mut self, block: u32) -> Option<()> {
        let scratch = &mut *self.scratch;
        self.graph.successors(block, self.ledger, |target| {
            if scratch.pending.len() >= scratch.pending_limit
                || scratch.pending.len() == scratch.pending.capacity()
            {
                self.ledger.inconsistent_inventory.set(true);
                return None;
            }
            scratch.pending.push_back(target);
            Some(())
        })
    }

    fn pop(&mut self) -> Option<Option<u32>> {
        self.ledger
            .with_budget(|budget| budget.charge_work(1))
            .ok()?;
        Some(self.scratch.pending.pop_front())
    }

    fn visit(&mut self, block: u32) -> Option<bool> {
        let ordinal = self.graph.ordinal(block, self.ledger)?;
        self.ledger
            .with_budget(|budget| budget.charge_work(1))
            .ok()?;
        let visited = self.scratch.visited.get_mut(ordinal)?;
        let first = *visited == 0;
        *visited = 1;
        Some(first)
    }

    fn found(&mut self, site: Self::Site) -> Option<()> {
        self.ledger
            .with_budget(|budget| budget.charge_work(1))
            .ok()?;
        if self.scratch.found.len() >= self.scratch.event_limit
            || self.scratch.found.len() == self.scratch.found.capacity()
        {
            self.ledger.inconsistent_inventory.set(true);
            return None;
        }
        self.scratch.found.push(site);
        Some(())
    }
}

impl EffectSignatureWalkV18 for SourceEffectWalkV18<'_, '_, '_, '_, '_> {
    fn contains_entry(&mut self, entry: u32) -> Option<bool> {
        Some(self.graph.ordinal(entry, self.ledger).is_some())
    }
    fn reset(&mut self) -> Option<()> {
        self.ledger
            .with_budget(|budget| {
                budget.charge_work(argument_sum_v1(&[
                    self.scratch.visited.len(),
                    self.scratch.pending.len(),
                    self.scratch.found.len(),
                ])?)
            })
            .ok()?;
        self.scratch.visited.fill(0);
        self.scratch.pending.clear();
        self.scratch.found.clear();
        Some(())
    }
    fn emit(
        &mut self,
        from: Option<Self::Site>,
        budget: &mut dyn CorrelationChargeV18,
    ) -> Option<()> {
        for site in &self.scratch.found {
            if let Some(from) = from {
                budget.charge()?;
                self.scratch.next_count = self.scratch.next_count.checked_add(1)?;
                if self.scratch.recording {
                    if self.scratch.next.len() >= self.scratch.next_limit
                        || self.scratch.next.len() == self.scratch.next.capacity()
                    {
                        self.ledger.inconsistent_inventory.set(true);
                        return None;
                    }
                    self.scratch.next.push((from, *site));
                }
            } else {
                self.ledger
                    .with_budget(|budget| budget.charge_work(1))
                    .ok()?;
                if self.scratch.entry.len() >= self.scratch.event_limit
                    || self.scratch.entry.len() == self.scratch.entry.capacity()
                {
                    self.ledger.inconsistent_inventory.set(true);
                    return None;
                }
                self.scratch.entry.push(*site);
            }
        }
        Some(())
    }
}

#[allow(clippy::too_many_arguments)]
fn source_effect_order_pass_v18(
    relation: &ProductionSourceCorrespondenceV18<'_>,
    inventory: &fe2o3_kernel_analysis::CanonicalKirInventoryV18<'_>,
    function: fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1,
    entry: u32,
    recipe: &fe2o3_pliron::ProductionRankedKernelV1,
    canonical: &[SourceFlowEventV18],
    ranked: &[SourceFlowEventV18],
    canonical_flow: &mut SourceFlowScratchV18,
    ranked_flow: &mut SourceFlowScratchV18,
    remaining: &mut usize,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<()> {
    let ledger = CorrelationLedgerV18::new(budget, relation.source.cleanup);
    let mut charge = SourceCorrelationChargeV18 {
        ledger: &ledger,
        finite: UnsupportedIndexCorrelationBudgetV1 {
            remaining: *remaining,
        },
        finite_denied: false,
    };
    let canonical_result = {
        let mut walk = SourceEffectWalkV18 {
            graph: SourceFlowGraphV18::Canonical {
                inventory,
                function,
            },
            events: canonical,
            scratch: canonical_flow,
            ledger: &ledger,
        };
        effect_flow_signature_core_v18(
            entry,
            canonical.iter().map(|row| (row.block, row.order, row.site)),
            &mut walk,
            &mut charge,
        )
    };
    let ranked_result = if canonical_result.is_some() {
        let mut walk = SourceEffectWalkV18 {
            graph: SourceFlowGraphV18::Ranked(recipe),
            events: ranked,
            scratch: ranked_flow,
            ledger: &ledger,
        };
        effect_flow_signature_core_v18(
            0,
            ranked.iter().map(|row| (row.block, row.order, row.site)),
            &mut walk,
            &mut charge,
        )
    } else {
        None
    };
    *remaining = charge.finite.remaining;
    let resource = ledger.failure.get();
    let refused = ledger.inconsistent_inventory.get() || charge.finite_denied;
    drop(charge);
    drop(ledger);
    if let Some(error) = resource {
        return Err(error.into());
    }
    if refused || canonical_result.is_none() || ranked_result.is_none() {
        return relation
            .source
            .missing("source effect order traversal refused");
    }
    Ok(())
}

// This compares the reachability/order of the fully correlated ranked effects.
// It does not prove branch expressions, omitted storage effects or read-from.
fn source_ranked_effect_order_v18(
    relation: &ProductionSourceCorrespondenceV18<'_>,
    root: usize,
    recipe: &fe2o3_pliron::ProductionRankedKernelV1,
    locations: &[SourceEffectLocationV18],
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<()> {
    source_ranked_effect_order_endpoint_v18(
        relation,
        SourceEffectEndpointV18::Original,
        root,
        recipe,
        locations,
        budget,
    )
}

#[derive(Clone, Copy)]
enum SourceEffectEndpointV18<'scope> {
    Original,
    Optimized(&'scope OptimizedSourceEffectCensusV18<'scope>),
}

fn source_ranked_effect_order_endpoint_v18(
    relation: &ProductionSourceCorrespondenceV18<'_>,
    endpoint: SourceEffectEndpointV18<'_>,
    root: usize,
    recipe: &fe2o3_pliron::ProductionRankedKernelV1,
    locations: &[SourceEffectLocationV18],
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<()> {
    relation.retain_query((|| {
        if let SourceEffectEndpointV18::Optimized(census) = endpoint {
            census.observe_custody(budget)?;
        }
        relation.query(budget)?;
        let floor = budget.storage();
        scoped_source_attempt_v29(relation.source.cleanup, budget, floor, |budget| {
            let floor = budget.storage();
            let (inventory, physical) = match endpoint {
                SourceEffectEndpointV18::Original => {
                    (relation.inventory, relation.source.root(root, budget)?.1)
                }
                SourceEffectEndpointV18::Optimized(census) => {
                    census.check(relation, root, budget)?;
                    let physical = optimized_source_root_function_v18(
                        relation,
                        census.optimized,
                        root,
                        budget,
                    )?;
                    (
                        census.optimized.output_inventory(budget)?,
                        physical.coordinate.0 as usize,
                    )
                }
            };
            let function = inventory.functions().get(physical).ok_or(
                ProductionSourceOwnedViewErrorV18::Binding("effect order physical root"),
            )?;
            let body = function.function.body.as_ref().ok_or(
                ProductionSourceOwnedViewErrorV18::Binding("effect order root body"),
            )?;
            let entry = body
                .blocks
                .first()
                .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                    "effect order physical entry",
                ))?
                .id
                .0;
            let work_limit = relation
                .source
                .limits(budget)?
                .max_operations
                .checked_mul(UNSUPPORTED_INDEX_CORRELATION_STEPS_PER_OPERATION_V1)
                .ok_or(ArgumentResourceV1::Arithmetic)?;
            budget.reserve_storage(argument_sum_v1(&[
                size_of::<Vec<usize>>(),
                size_of::<Vec<u8>>(),
                argument_product_v1(2, size_of::<Vec<SourceFlowEventV18>>())?,
                size_of::<CorrelationLedgerV18<'_, '_, '_>>(),
                size_of::<SourceCorrelationChargeV18<'_, '_, '_, '_>>(),
                size_of::<SourceEffectWalkV18<'_, '_, '_, '_, '_>>(),
            ])?)?;
            let mut order =
                emission_vec_v1(locations.len(), budget).map_err(source_emission_error_v18)?;
            let mut selected =
                emission_vec_v1(locations.len(), budget).map_err(source_emission_error_v18)?;
            let mut canonical =
                emission_vec_v1(locations.len(), budget).map_err(source_emission_error_v18)?;
            let mut ranked =
                emission_vec_v1(locations.len(), budget).map_err(source_emission_error_v18)?;
            budget.charge_work(argument_product_v1(locations.len(), 2)?)?;
            order.extend(0..locations.len());
            selected.resize(locations.len(), 0u8);
            for row in locations {
                budget.charge_work(5)?;
                if row.physical.block.function != function.coordinate
                    || relation.source.instance(root, row.site.instance, budget)?.0
                        != row.site.function
                    || recipe
                        .blocks()
                        .get(row.ranked.0 as usize)
                        .and_then(|block| block.operations().get(row.ranked.1 as usize))
                        .is_none()
                {
                    return relation
                        .source
                        .missing("effect order changed original instance or physical/ranked site");
                }
                match endpoint {
                    SourceEffectEndpointV18::Original => {
                        let mut matched = 0usize;
                        let mut match_operation = |operation, budget: &mut ArgumentBudgetV1<'_>| {
                            budget.charge_work(1)?;
                            if operation == ProductionSourceOperationV18::Operation(row.physical) {
                                matched = matched
                                    .checked_add(1)
                                    .ok_or(ArgumentResourceV1::Arithmetic)?;
                            }
                            Ok::<_, ProductionSourceOwnedViewErrorV18>(())
                        };
                        relation.visit_source_operations(
                            root,
                            row.site.instance,
                            row.site.block,
                            row.site.statement,
                            budget,
                            &mut match_operation,
                        )?;
                        if matched != 1 {
                            return relation.source.missing(
                                "effect order operation is not its original source occurrence",
                            );
                        }
                    }
                    SourceEffectEndpointV18::Optimized(census) => {
                        census.check_location(relation, root, row, budget)?
                    }
                }
                let block = body.blocks.get(row.physical.block.block as usize).ok_or(
                    ProductionSourceOwnedViewErrorV18::Binding("effect order physical block"),
                )?;
                let operation = block
                    .operations
                    .get(row.physical.operation as usize)
                    .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                        "effect order physical operation",
                    ))?;
                let mut count = 0usize;
                operation.try_visit_local_memory_effects_v1(
                    |effect| -> SourceOwnedResultV18<()> {
                        use fe2o3_kernel_ir::KirLocalMemoryEffectRefV1 as Effect;
                        budget.charge_work(1)?;
                        if matches!(
                            effect,
                            Effect::Read(_)
                                | Effect::Write(_)
                                | Effect::VolatileRead(_)
                                | Effect::VolatileWrite(_)
                                | Effect::Atomic { .. }
                        ) {
                            count = count.checked_add(1).ok_or(ArgumentResourceV1::Arithmetic)?;
                        }
                        Ok(())
                    },
                )?;
                if row.access as usize >= count {
                    return relation
                        .source
                        .missing("effect order physical access ordinal");
                }
            }
            // Preserve the shared legacy rule for compound operations, while
            // retaining the source instance/function in the duplicate check.
            private_array_heapsort_v1(
                &mut order,
                |index| {
                    let row = &locations[*index];
                    [row.ranked.0 as usize, row.ranked.1 as usize, *index]
                },
                &mut SourceCorrespondenceWorkV18(budget),
                || ArgumentResourceV1::Arithmetic.into(),
            )?;
            let mut previous: Option<usize> = None;
            for index in &order {
                budget.charge_work(3)?;
                let row = &locations[*index];
                if let Some(first) = previous.filter(|first| locations[*first].ranked == row.ranked)
                {
                    let first = locations[first].site;
                    if (first.instance, first.function, first.block, first.statement)
                        != (
                            row.site.instance,
                            row.site.function,
                            row.site.block,
                            row.site.statement,
                        )
                    {
                        return relation.source.missing(
                            "ranked effect combines distinct original source occurrences",
                        );
                    }
                    continue;
                }
                selected[*index] = 1;
                previous = Some(*index);
            }
            for (index, row) in locations.iter().enumerate() {
                budget.charge_work(1)?;
                if selected[index] == 0 {
                    continue;
                }
                canonical.push(SourceFlowEventV18 {
                    block: body.blocks[row.physical.block.block as usize].id.0,
                    order: (u64::from(row.physical.operation) << 32) | u64::from(row.access),
                    site: row.site,
                });
                ranked.push(SourceFlowEventV18 {
                    block: row.ranked.0,
                    order: u64::from(row.ranked.1) << 32,
                    site: row.site,
                });
            }
            for rows in [&mut canonical, &mut ranked] {
                private_array_heapsort_v1(
                    rows,
                    source_flow_event_key_v18,
                    &mut SourceCorrespondenceWorkV18(budget),
                    || ArgumentResourceV1::Arithmetic.into(),
                )?;
                for pair in rows.windows(2) {
                    budget.charge_work(2)?;
                    if (pair[0].block, pair[0].order) == (pair[1].block, pair[1].order) {
                        return relation
                            .source
                            .missing("effect order repeats a physical or ranked occurrence");
                    }
                }
            }
            let mut ranked_edges = 0usize;
            for block in recipe.blocks() {
                budget.charge_work(1)?;
                ranked_edges = ranked_edges
                    .checked_add(ranked_terminator_successors_v18(block.terminator()).1)
                    .ok_or(ArgumentResourceV1::Arithmetic)?;
            }
            let mut canonical_flow = SourceFlowScratchV18::new(
                function.blocks.len(),
                function.edges.len(),
                canonical.len(),
                budget,
            )?;
            let mut ranked_flow = SourceFlowScratchV18::new(
                recipe.blocks().len(),
                ranked_edges,
                ranked.len(),
                budget,
            )?;
            let mut remaining = work_limit;
            source_effect_order_pass_v18(
                relation,
                inventory,
                function.coordinate,
                entry,
                recipe,
                &canonical,
                &ranked,
                &mut canonical_flow,
                &mut ranked_flow,
                &mut remaining,
                budget,
            )?;
            canonical_flow.prepare_output(budget)?;
            ranked_flow.prepare_output(budget)?;
            let storage = budget
                .storage()
                .checked_sub(floor)
                .ok_or(ArgumentResourceV1::Accounting)?;
            source_effect_order_pass_v18(
                relation,
                inventory,
                function.coordinate,
                entry,
                recipe,
                &canonical,
                &ranked,
                &mut canonical_flow,
                &mut ranked_flow,
                &mut remaining,
                budget,
            )?;
            if canonical_flow.next_count != canonical_flow.next_limit
                || ranked_flow.next_count != ranked_flow.next_limit
            {
                return relation
                    .source
                    .missing("effect order replay changed its exact output census");
            }
            canonical_flow.canonicalize(budget)?;
            ranked_flow.canonicalize(budget)?;
            budget.charge_work(argument_sum_v1(&[
                argument_product_v1(canonical_flow.entry.len().max(ranked_flow.entry.len()), 6)?,
                argument_product_v1(canonical_flow.next.len().max(ranked_flow.next.len()), 12)?,
            ])?)?;
            if canonical_flow.entry != ranked_flow.entry || canonical_flow.next != ranked_flow.next
            {
                return relation
                    .source
                    .missing("actual source and ranked effect order differ");
            }
            drop((
                canonical_flow,
                ranked_flow,
                canonical,
                ranked,
                order,
                selected,
            ));
            budget.release_storage(storage)?;
            Ok(())
        })
    })())
}

// Generated operations reuse the neutral recipe replay below. Their exact
// coordinates come from original instance attachments, including split spans.
// Any query failure remains sticky in the original source view and ledger;
// the containing relation selects that diagnostic before a replay mismatch.
struct SourceGeneratedRecipeReaderV18<'a, 'g, 'b, 'w, 'c> {
    relation: &'a ProductionSourceCorrespondenceV18<'g>,
    root: usize,
    instance: usize,
    origins: &'a value_origin_v1::WholeValueOriginsV18<'g>,
    ledger: &'a CorrelationLedgerV18<'b, 'w, 'c>,
}

impl SourceGeneratedRecipeReaderV18<'_, '_, '_, '_, '_> {
    fn query<T>(
        &self,
        query: impl FnOnce(&mut ArgumentBudgetV1<'_>) -> SourceOwnedResultV18<T>,
    ) -> Option<T> {
        match self
            .ledger
            .with_budget(|budget| Ok(self.relation.retain_query(query(budget))))
        {
            Ok(Ok(value)) => Some(value),
            Ok(Err(ProductionSourceOwnedViewErrorV18::Resource(error))) => {
                self.ledger.fail(error);
                None
            }
            Ok(Err(_)) => {
                self.ledger.inconsistent_inventory.set(true);
                None
            }
            Err(_) => None,
        }
    }
}

impl NeutralRecipeMeterV18 for SourceGeneratedRecipeReaderV18<'_, '_, '_, '_, '_> {
    fn charge_recipe_step(&self) -> Option<()> {
        self.ledger.with_budget(|budget| budget.charge_work(1)).ok()
    }
}

impl GeneratedRecipeSourceV18 for SourceGeneratedRecipeReaderV18<'_, '_, '_, '_, '_> {
    fn operation_origin(
        &self,
        body: &FunctionBody,
        _kir: &dyn KirCorrelationGraphV18,
        value: ValueId,
    ) -> Option<ValueId> {
        self.query(|budget| {
            self.relation.query(budget)?;
            let function = self.relation.source.root(self.root, budget)?.1;
            let row = self.relation.inventory.functions().get(function).ok_or(
                ProductionSourceOwnedViewErrorV18::Binding("recipe origin root"),
            )?;
            if !row
                .function
                .body
                .as_ref()
                .is_some_and(|actual| std::ptr::eq(actual, body))
                || !self
                    .origins
                    .belongs_to(self.relation.inventory, row.coordinate)
            {
                return self
                    .relation
                    .source
                    .missing("recipe origin analysis changed owner or body");
            }
            self.relation
                .retain_query(
                    self.origins
                        .operation_origin(value, budget)
                        .map_err(|error| {
                            match error {
                    fe2o3_kernel_analysis::CanonicalKirInventoryErrorV1::Resource(error) => {
                        error.into()
                    }
                    fe2o3_kernel_analysis::CanonicalKirInventoryErrorV1::InconsistentOwner => {
                        ProductionSourceOwnedViewErrorV18::Binding(
                            "recipe origin inventory association",
                        )
                    }
                }
                        }),
                )
        })?
    }

    fn charge(&self, amount: usize) -> Result<(), ProductionMirPlironTranslationErrorV1> {
        self.ledger
            .with_budget(|budget| budget.charge_work(amount))
            .map_err(|_| ProductionMirPlironTranslationErrorV1::ResourceLimit)
    }

    fn reserve(&self, bytes: usize) -> Result<(), ProductionMirPlironTranslationErrorV1> {
        self.ledger
            .with_budget(|budget| budget.reserve_storage(bytes))
            .map_err(|_| ProductionMirPlironTranslationErrorV1::ResourceLimit)
    }

    fn values(&self, block: u32) -> Option<GeneratedRecipeValuesV18> {
        self.query(|budget| {
            self.relation.generated_recipe_values_v18(
                self.root,
                self.instance,
                SemanticBlockIdV1::from_index(block),
                budget,
            )
        })
    }

    fn operations(&self, block: u32) -> Option<NeutralRecipeOperationsV18<'_>> {
        self.query(|budget| {
            let rows = self.relation.source_operation_rows(
                self.root,
                self.instance,
                SemanticBlockIdV1::from_index(block),
                None,
                budget,
            )?;
            let function = self.relation.source.root(self.root, budget)?.1;
            let function = fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1(
                u32::try_from(function).map_err(|_| ArgumentResourceV1::Arithmetic)?,
            );
            Ok(NeutralRecipeOperationsV18::Source {
                rows,
                inventory: self.relation.inventory,
                function,
                meter: self,
            })
        })
    }

    fn producer_contains(&self, block: u32, location: FunctionOperationLocation) -> Option<bool> {
        let operations = self.operations(block)?;
        let NeutralRecipeOperationsV18::Source { rows, .. } = &operations else {
            return None;
        };
        for (index, row) in rows.iter().enumerate() {
            self.charge_recipe_step()?;
            match row.location {
                TileAttachmentLocationV29::Gap(_)
                | TileAttachmentLocationV29::Tombstone
                | TileAttachmentLocationV29::NoOutput => continue,
                TileAttachmentLocationV29::Origin(TileScalarSourceV29::Operation(_)) => {}
                _ => {
                    return self.query(|_| {
                        self.relation
                            .source
                            .missing("recipe producer span contains a non-operation attachment")
                    });
                }
            }
            let Some((_, actual)) = operations.get(index) else {
                return self.query(|_| {
                    self.relation
                        .source
                        .missing("recipe producer operation is absent")
                });
            };
            if actual == location {
                return Some(true);
            }
        }
        Some(false)
    }
}
