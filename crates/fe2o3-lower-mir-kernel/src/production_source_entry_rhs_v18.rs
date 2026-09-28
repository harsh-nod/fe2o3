use scoped_raw_admission_v29::{
    OptimizedSourceObjectV18, optimized_source_object_payload_v18,
    visit_optimized_source_objects_v18,
};

#[derive(Clone, Copy, Debug, PartialEq)]
struct SourceEntryWriteRowV18 {
    instance: usize,
    anchor: usize,
    input: fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1,
    output: fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1,
    input_rhs: ValueId,
    output_rhs: ValueId,
    local: SemanticLocalIdV1,
    ty: SemanticTypeIdV1,
    scalar: ProductionSemanticScalarTypeV2,
    schema: fe2o3_kernel_ir::StorageLayoutIdV1,
}

/// One exact typed scalar entry write. The source argument must be resolved
/// independently; this request never exports a physical RHS as a source value.
pub struct ProductionSourceEntryWriteV18<'scope> {
    leaves: &'scope ProductionOptimizedSourceScalarLeavesV18<'scope>,
    arguments: &'scope SourceRootArgumentsV18<'scope, 'scope>,
    input_origins: &'scope value_origin_v1::WholeValueOriginsV18<'scope>,
    output_origins: &'scope value_origin_v1::WholeValueOriginsV18<'scope>,
    input_inline: &'scope Gfx942InlineScalarCorrespondenceV30<'scope>,
    output_inline: &'scope Gfx942InlineScalarCorrespondenceV30<'scope>,
    function: &'scope SemanticFunctionDeclV1,
    row: SourceEntryWriteRowV18,
    completed: &'scope std::cell::Cell<bool>,
    floor: usize,
}

/// Lexical exact-entry RHS correspondence. This contains only requests whose
/// original and optimized RHS checks completed, not memory or native authority.
pub struct ProductionCheckedSourceEntryWritesV18<'scope> {
    leaves: &'scope ProductionOptimizedSourceScalarLeavesV18<'scope>,
    rows: &'scope [SourceEntryWriteRowV18],
    floor: usize,
}

fn source_entry_write_row_v18(
    original: &ProductionSourceCorrespondenceV18<'_>,
    root: usize,
    row: &OptimizedSourceObjectV18<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<Option<SourceEntryWriteRowV18>> {
    let ScopedObjectRoleV29::WriteValue {
        destination,
        value:
            ScopedObjectValueOriginV29::Original(ScopedMemoryStoreSourceV29::EntryArgument {
                local,
                ty,
            }),
    } = row.original.source.role
    else {
        return Ok(None);
    };
    let semantic = original.source.source_semantic(budget)?;
    let (id, _) = original
        .source
        .instance(root, row.original.instance, budget)?;
    let function = semantic.functions().get(id.index() as usize).ok_or(
        ProductionSourceOwnedViewErrorV18::Binding("typed entry original function"),
    )?;
    budget.charge_work(10)?;
    let declaration = function
        .locals()
        .get(local.index() as usize)
        .filter(|declaration| declaration.ty() == ty)
        .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
            "typed entry original local",
        ))?;
    if !matches!(declaration.role(), SemanticLocalRoleV1::Argument(_))
        || !matches!(
            semantic
                .types()
                .get(ty.index() as usize)
                .map(SemanticTypeDeclV1::shape),
            Some(SemanticTypeShapeV1::Scalar(_))
        )
        || destination.source != (ScopedObjectSourceV29::EntryArgument { local })
        || !matches!(destination.object, ScopedObjectIdentityV29::Local {
            instance, local: actual, generation: 0,
        } if instance.index() == row.original.instance && actual == local)
        || destination.root_type != ty
        || destination.projected_type != ty
        || destination.root_schema != destination.projected_schema
        || destination.source_path.count != 0
        || destination.path.count != 0
        || row.original.source.result.is_some()
        || row.original.actual.role != row.original.source.role
        || row.actual.role != row.original.source.role
    {
        return original
            .source
            .missing("typed entry is not an exact whole scalar argument");
    }
    let (
        ScopedObjectOperationV29::WriteValue {
            value: input_rhs, ..
        },
        ScopedObjectOperationV29::WriteValue {
            value: output_rhs, ..
        },
    ) = (row.original.actual.operation, row.actual.operation)
    else {
        return original
            .source
            .missing("typed entry changed WriteValue endpoints");
    };
    let scalar = kir_semantic_scalar_v1(
        &lower_scalar_type(semantic.types(), ty).map_err(source_emission_error_v18)?,
    )
    .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
        "typed entry scalar type",
    ))?;
    Ok(Some(SourceEntryWriteRowV18 {
        instance: row.original.instance,
        anchor: row.original.row,
        input: row.input,
        output: row.output,
        input_rhs,
        output_rhs,
        local,
        ty,
        scalar,
        schema: destination.projected_schema,
    }))
}

impl ProductionSourceEntryWriteV18<'_> {
    fn check(&self, budget: &mut ArgumentBudgetV1<'_>) -> SourceOwnedResultV18<()> {
        let original = self.leaves.original.leaves.relation;
        original.retain_query((|| {
            self.leaves.observe_custody(budget)?;
            if budget.storage() < self.floor {
                original.source.cleanup.deny_refund();
                return Err(ArgumentResourceV1::Accounting.into());
            }
            self.leaves.check(budget)?;
            let root = self.leaves.original.leaves.root;
            let function = self
                .leaves
                .original
                .original_function(self.row.instance, budget)?;
            if !std::ptr::eq(function, self.function) {
                return original
                    .source
                    .missing("typed entry substituted original function");
            }
            let source = original.retained_object_payload_at_v29(
                root,
                self.row.instance,
                self.row.anchor,
                self.row.input,
                budget,
            )?;
            let actual = match self.leaves.optimized.operation(self.row.input, budget)? {
                ProductionOptimizedSourceOperationV18::Retained { output, .. }
                    if output == self.row.output =>
                {
                    optimized_source_object_payload_v18(
                        original,
                        self.leaves.optimized,
                        self.row.input,
                        output,
                        &source.actual,
                        budget,
                    )?
                }
                _ => {
                    return original
                        .source
                        .missing("typed entry changed exact retained operation");
                }
            };
            let candidate = source_entry_write_row_v18(
                original,
                root,
                &OptimizedSourceObjectV18 {
                    original: source,
                    input: self.row.input,
                    output: self.row.output,
                    actual,
                },
                budget,
            )?;
            if candidate != Some(self.row) {
                return original
                    .source
                    .missing("typed entry changed source or RHS binding");
            }
            Ok(())
        })())
    }

    /// Returns the exact original invocation and declaration.
    pub fn original(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<(usize, &SemanticFunctionDeclV1)> {
        self.check(budget)?;
        Ok((self.row.instance, self.function))
    }

    /// Lends only the authenticated logical source argument, never a guessed
    /// physical parameter ordinal or a descendant graph value.
    pub fn input_for<'function>(
        &self,
        function: &'function SemanticFunctionDeclV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<ProductionSourceScalarInputV18<'function>> {
        let original = self.leaves.original.leaves.relation;
        original.retain_query((|| {
            self.check(budget)?;
            budget.charge_work(2)?;
            if !std::ptr::eq(function, self.function) {
                return original
                    .source
                    .missing("typed entry resolver substituted original declaration");
            }
            let Some(SemanticLocalRoleV1::Argument(argument)) = function
                .locals()
                .get(self.row.local.index() as usize)
                .map(|local| local.role())
            else {
                return original
                    .source
                    .missing("typed entry original argument absent");
            };
            Ok(ProductionSourceScalarInputV18::EntryArgument { argument })
        })())
    }

    /// Returns the exact source scalar type after owner and occurrence checks.
    pub fn scalar(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<ProductionSemanticScalarTypeV2> {
        self.check(budget)?;
        Ok(self.row.scalar)
    }

    /// Independently compares the original and optimized physical RHS against
    /// an expression reconstructed from the authenticated original argument.
    /// Neither a successful callback nor transport descendants complete this.
    pub fn check_expression(
        &self,
        expression: &ProductionSemanticExpressionV2,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<()> {
        let original = self.leaves.original.leaves.relation;
        original.retain_query((|| {
            self.check(budget)?;
            if expression.scalar() != self.row.scalar {
                return original
                    .source
                    .missing("typed entry expression scalar type changed");
            }
            source_scalar_expression_value_v18(
                self.leaves.original.leaves,
                self.arguments,
                self.input_origins,
                self.input_inline,
                expression,
                self.row.input_rhs,
                budget,
            )?;
            let normalizer = OptimizedSourceScalarNormalizationV18 {
                leaves: self.leaves,
                arguments: self.arguments,
            };
            optimized_source_scalar_expression_endpoint_v18(
                self.leaves.original.leaves,
                self.leaves.optimized,
                self.leaves.optimized.output_inventory(budget)?,
                self.leaves.function.coordinate,
                &normalizer,
                self.output_origins,
                self.output_inline,
                expression,
                self.row.output_rhs,
                budget,
            )?;
            self.completed.set(true);
            Ok(())
        })())
    }
}

impl ProductionCheckedSourceEntryWritesV18<'_> {
    fn check_for(
        &self,
        original: &ProductionSourceCorrespondenceV18<'_>,
        optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
        root: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<()> {
        let owner = self.leaves.original.leaves.relation;
        owner.retain_query((|| {
            // Reject foreign owners and ledgers before any traversal debit.
            if !std::ptr::eq(original, owner)
                || !std::ptr::eq(optimized, self.leaves.optimized)
                || root != self.leaves.original.leaves.root
            {
                return owner
                    .source
                    .missing("typed entry context changed exact owners or root");
            }
            self.leaves.observe_custody(budget)?;
            if budget.storage() < self.floor {
                owner.source.cleanup.deny_refund();
                return Err(ArgumentResourceV1::Accounting.into());
            }
            self.leaves.check(budget)
        })())
    }

    fn require(
        &self,
        instance: usize,
        anchor: usize,
        input: fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1,
        output: fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1,
        input_rhs: ValueId,
        output_rhs: ValueId,
        scalar: ProductionSemanticScalarTypeV2,
        schema: fe2o3_kernel_ir::StorageLayoutIdV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<()> {
        let original = self.leaves.original.leaves.relation;
        original.retain_query((|| {
            self.check_for(
                original,
                self.leaves.optimized,
                self.leaves.original.leaves.root,
                budget,
            )?;
            let mut first = 0;
            let mut end = self.rows.len();
            while first < end {
                budget.charge_work(2)?;
                let middle = first + (end - first) / 2;
                if (self.rows[middle].instance, self.rows[middle].anchor) < (instance, anchor) {
                    first = middle + 1;
                } else {
                    end = middle;
                }
            }
            budget.charge_work(8)?;
            if !self.rows.get(first).is_some_and(|row| {
                row.instance == instance
                    && row.anchor == anchor
                    && row.input == input
                    && row.output == output
                    && row.input_rhs == input_rhs
                    && row.output_rhs == output_rhs
                    && row.scalar == scalar
                    && row.schema == schema
            }) {
                return original
                    .source
                    .missing("typed entry RHS has no exact completed request");
            }
            Ok(())
        })())
    }
}

type SourceEntryWritePreparedV18<'a> = (
    Vec<SourceEntryWriteRowV18>,
    SourceRootArgumentsV18<'a, 'a>,
    Gfx942InlineScalarCorrespondenceV30<'a>,
    Gfx942InlineScalarCorrespondenceV30<'a>,
    fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1,
    usize,
);

fn source_entry_write_callback_headers_v18(
    request_size: usize,
    request_alignment: usize,
    consume_size: usize,
    consume_alignment: usize,
) -> Result<usize, ArgumentResourceV1> {
    argument_sum_v1(&[
        request_size,
        consume_size,
        argument_product_v1(2, request_alignment)?,
        argument_product_v1(2, consume_alignment)?,
    ])
}

fn source_entry_write_headers_v18<T, E>() -> Result<usize, ArgumentResourceV1> {
    argument_sum_v1(&[
        size_of::<SourceEntryWritePreparedV18<'_>>(),
        argument_product_v1(
            2,
            size_of::<SourceOwnedResultV18<SourceEntryWritePreparedV18<'_>>>(),
        )?,
        size_of::<std::thread::Result<SourceOwnedResultV18<SourceEntryWritePreparedV18<'_>>>>(),
        size_of::<
            std::panic::AssertUnwindSafe<SourceOwnedResultV18<SourceEntryWritePreparedV18<'_>>>,
        >(),
        size_of::<Vec<SourceEntryWriteRowV18>>(),
        size_of::<SourceRootArgumentsV18<'_, '_>>(),
        2 * size_of::<Gfx942InlineScalarCorrespondenceV30<'_>>(),
        size_of::<ProductionSourceEntryWriteV18<'_>>(),
        size_of::<ProductionCheckedSourceEntryWritesV18<'_>>(),
        size_of::<OptimizedSourceScalarNormalizationV18<'_>>(),
        size_of::<Option<SourceEntryWriteRowV18>>(),
        size_of::<SourcePhysicalObjectV18<'_>>(),
        size_of::<OptimizedSourceObjectV18<'_>>(),
        size_of::<ScopedObjectPayloadV29>(),
        size_of::<[Option<ValueId>; 2]>(),
        size_of::<SourceEntryWriteRowV18>(),
        size_of::<SourceOwnedResultV18<Option<SourceEntryWriteRowV18>>>(),
        size_of::<SourceOwnedResultV18<SourcePhysicalObjectV18<'_>>>(),
        size_of::<SourceOwnedResultV18<ScopedObjectPayloadV29>>(),
        size_of::<ProductionOptimizedSourceOperationV18>(),
        size_of::<SourceOwnedResultV18<ProductionOptimizedSourceOperationV18>>(),
        size_of::<SourceOwnedResultV18<&SemanticFunctionDeclV1>>(),
        size_of::<SourceOwnedResultV18<(usize, &SemanticFunctionDeclV1)>>(),
        size_of::<SourceOwnedResultV18<(SemanticFunctionIdV1, Option<(usize, SemanticBlockIdV1)>)>>(
        ),
        size_of::<SourceOwnedResultV18<&AdmittedInertSemanticMirV1>>(),
        size_of::<Option<&SemanticFunctionDeclV1>>(),
        size_of::<Option<&fe2o3_mir_model::semantic_mir_v1::SemanticLocalDeclV1>>(),
        size_of::<Option<SemanticLocalRoleV1>>(),
        size_of::<ProductionSourceScalarInputV18<'_>>(),
        size_of::<SourceOwnedResultV18<ProductionSourceScalarInputV18<'_>>>(),
        size_of::<SourceOwnedResultV18<ProductionSemanticScalarTypeV2>>(),
        size_of::<Type>(),
        size_of::<Result<Type, ProductionSemanticKirErrorV1>>(),
        size_of::<Option<&SourceEntryWriteRowV18>>(),
        size_of::<Result<(), E>>(),
        size_of::<Result<T, E>>(),
        size_of::<Option<SourceOwnedQueryFailureV18>>(),
        size_of::<SourceOwnedResultV18<()>>(),
        size_of::<std::cell::Cell<bool>>(),
        size_of::<std::thread::Result<Result<T, E>>>(),
        source_reference_cleanup_headers_v29()?,
    ])
}

impl ProductionOptimizedSourceScalarLeavesV18<'_> {
    /// Checks every retained whole scalar typed entry write once, then lends
    /// exact completed rows to a separate source/currentness consumer. Generic
    /// Storage reads and other writes remain outside the scalar namespace.
    pub fn with_checked_entry_writes_v18<'work, T, E>(
        &self,
        budget: &mut ArgumentBudgetV1<'work>,
        mut check_request: impl for<'request> FnMut(
            &ProductionSourceEntryWriteV18<'request>,
            &mut ArgumentBudgetV1<'work>,
        ) -> Result<(), E>,
        consume: impl for<'scope> FnOnce(
            &ProductionCheckedSourceEntryWritesV18<'scope>,
            &mut ArgumentBudgetV1<'work>,
        ) -> Result<T, E>,
    ) -> Result<T, E>
    where
        E: From<ProductionSourceOwnedViewErrorV18>,
    {
        self.check(budget)?;
        let original = self.original.leaves.relation;
        let root = self.original.leaves.root;
        let floor = budget.storage();
        let (rows, arguments, input_inline, output_inline, function, retained) =
            scoped_source_attempt_v29(original.source.cleanup, budget, floor, |budget| {
                original.retain_query((|| {
                    budget.reserve_storage(argument_sum_v1(&[
                        source_entry_write_headers_v18::<T, E>()?,
                        source_entry_write_callback_headers_v18(
                            std::mem::size_of_val(&check_request),
                            std::mem::align_of_val(&check_request),
                            std::mem::size_of_val(&consume),
                            std::mem::align_of_val(&consume),
                        )?,
                    ])?)?;
                    let physical = original.source.root(root, budget)?.1;
                    let function = original.inventory.functions().get(physical).ok_or(
                        ProductionSourceOwnedViewErrorV18::Binding("typed entry root"),
                    )?;
                    let mut rows = emission_vec_v1(function.operations.len(), budget)
                        .map_err(source_emission_error_v18)?;
                    visit_optimized_source_objects_v18(
                        original,
                        self.optimized,
                        root,
                        budget,
                        |row, budget| {
                            if let Some(entry) =
                                source_entry_write_row_v18(original, root, &row, budget)?
                            {
                                if rows.len() == rows.capacity() {
                                    return original.source.missing("typed entry census capacity");
                                }
                                rows.push(entry);
                            }
                            Ok(())
                        },
                    )?;
                    private_array_heapsort_v1(
                        &mut rows,
                        |row| [row.instance, row.anchor],
                        &mut SourceCorrespondenceWorkV18(budget),
                        || ArgumentResourceV1::Arithmetic.into(),
                    )?;
                    for pair in rows.windows(2) {
                        budget.charge_work(1)?;
                        if (pair[0].instance, pair[0].anchor) == (pair[1].instance, pair[1].anchor)
                        {
                            return original
                                .source
                                .missing("typed entry census repeated source anchor");
                        }
                    }
                    let arguments = SourceRootArgumentsV18::build(original, root, budget)?;
                    let input_inline = Gfx942InlineScalarCorrespondenceV30::build_source_v18(
                        original, root, budget,
                    )?;
                    let output_inline = input_inline.transport_optimized_source_v18(
                        original,
                        self.optimized,
                        root,
                        budget,
                    )?;
                    let retained = budget
                        .storage()
                        .checked_sub(floor)
                        .ok_or(ArgumentResourceV1::Accounting)?;
                    Ok((
                        rows,
                        arguments,
                        input_inline,
                        output_inline,
                        function.coordinate,
                        retained,
                    ))
                })())
            })?;
        let retained_floor = budget.storage();
        let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            value_origin_v1::with_whole_value_origins_v18(
                original,
                function,
                budget,
                |input_origins, budget| {
                    value_origin_v1::with_optimized_whole_value_origins_v18(
                        original,
                        self.optimized,
                        self.optimized.output_inventory(budget)?,
                        self.function.coordinate,
                        budget,
                        |output_origins, budget| {
                            let request_floor = budget.storage();
                            for row in &rows {
                                let function =
                                    self.original.original_function(row.instance, budget)?;
                                let completed = std::cell::Cell::new(false);
                                let request = ProductionSourceEntryWriteV18 {
                                    leaves: self,
                                    arguments: &arguments,
                                    input_origins,
                                    output_origins,
                                    input_inline: &input_inline,
                                    output_inline: &output_inline,
                                    function,
                                    row: *row,
                                    completed: &completed,
                                    floor: request_floor,
                                };
                                check_request(&request, budget)?;
                                request.check(budget)?;
                                if !completed.get() {
                                    return original
                                        .source
                                        .missing(
                                            "typed entry request did not check both RHS endpoints",
                                        )
                                        .map_err(Into::into);
                                }
                            }
                            let checked = ProductionCheckedSourceEntryWritesV18 {
                                leaves: self,
                                rows: &rows,
                                floor: request_floor,
                            };
                            consume(&checked, budget)
                        },
                    )
                },
            )
        }));
        let prior = original.source.guard.first.get();
        let postflight = if budget.storage() < retained_floor {
            original.source.cleanup.deny_refund();
            Err(ArgumentResourceV1::Accounting.into())
        } else {
            self.observe_custody(budget)
        };
        drop((rows, arguments, input_inline, output_inline));
        source_owned_finish_callback_v18(
            caught,
            prior,
            postflight,
            original.source.cleanup,
            budget,
            retained,
        )
    }
}
