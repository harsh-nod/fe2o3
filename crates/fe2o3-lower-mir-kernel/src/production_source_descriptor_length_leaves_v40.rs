// Names for immutable descriptor lengths. Original assignments and admitted
// entry recipes choose these names, never candidate scalar values or bits.
#[derive(Clone, Copy)]
struct DescriptorLengthRowV40 {
    instance: usize,
    block: u32,
    statement: u32,
    ty: SemanticTypeIdV1,
    operation: SliceOperation,
    value: ValueId,
    parameter: SliceDefinition,
    element: ScalarType,
    symbol: u32,
}

#[derive(Clone, Copy)]
struct DescriptorLengthLookupV40 {
    key: [usize; 4],
    row: usize,
}

pub(super) struct SourceDescriptorLengthsV40 {
    rows: Vec<DescriptorLengthRowV40>,
    lookup: Vec<DescriptorLengthLookupV40>,
}

impl SourceDescriptorLengthsV40 {
    pub(super) fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    pub(super) fn empty() -> Self {
        Self {
            rows: Vec::new(),
            lookup: Vec::new(),
        }
    }

    fn find(
        &self,
        key: [usize; 4],
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<Option<&DescriptorLengthRowV40>> {
        let (mut low, mut high) = (0, self.lookup.len());
        while low < high {
            budget.charge_work(2)?;
            let middle = low + (high - low) / 2;
            if self.lookup[middle].key < key {
                low = middle + 1;
            } else {
                high = middle;
            }
        }
        budget.charge_work(2)?;
        self.lookup
            .get(low)
            .filter(|row| row.key == key)
            .map(|row| {
                self.rows
                    .get(row.row)
                    .ok_or(ArgumentResourceV1::Accounting.into())
            })
            .transpose()
    }
}

#[derive(Clone, Copy)]
pub(super) struct OptimizedDescriptorLengthV40 {
    value: ValueId,
    original: usize,
}

fn descriptor_length_scalar_v40() -> ProductionSemanticScalarTypeV2 {
    ProductionSemanticScalarTypeV2::Integer {
        signed: false,
        bits: 64,
    }
}

fn descriptor_length_leaf_headers_v40() -> Result<usize, ArgumentResourceV1> {
    type Frame<'a> = (
        SourceDescriptorLengthsV40,
        DescriptorLengthRowV40,
        DescriptorLengthLookupV40,
        Vec<DescriptorLengthRowV40>,
        Vec<DescriptorLengthLookupV40>,
        Vec<(u32, usize)>,
        Vec<Option<ScalarType>>,
        Vec<OptimizedDescriptorLengthV40>,
        OptimizedDescriptorLengthV40,
        SourceOwnedResultV18<SourceDescriptorLengthsV40>,
        SourceOwnedResultV18<Vec<OptimizedDescriptorLengthV40>>,
        SourceOwnedResultV18<Option<&'a DescriptorLengthRowV40>>,
        SourceOwnedResultV18<ProductionSemanticExpressionV2>,
        SourceOwnedResultV18<Option<NormalizedScalarExpressionV1>>,
        SourceOwnedResultV18<()>,
        SourceOwnedResultV18<Option<ScalarType>>,
        SourceOwnedResultV18<(SliceOperation, ValueId)>,
        [&'a (); 14],
        [usize; 12],
        [SliceDefinition; 3],
        Option<ProductionSourceRootSliceAbiV36<'a, 'a>>,
        &'a ProductionSourceRootSliceAbisV36<'a, 'a>,
        std::slice::Iter<'a, SourceRvalueRowV30>,
        std::slice::Iter<'a, fe2o3_mir_model::semantic_mir_v1::SemanticLocalDeclV1>,
        std::iter::Enumerate<std::slice::Iter<'a, DescriptorLengthRowV40>>,
    );
    argument_sum_v1(&[
        size_of::<Frame<'_>>(),
        std::mem::align_of::<Frame<'_>>(),
        descriptor_length_headers_v30()?,
        source_descriptor_operand_headers_v30()?,
    ])
}

pub(super) fn source_descriptor_lengths_v40(
    relation: &ProductionSourceCorrespondenceV18<'_>,
    root: usize,
    memory: &[SourceScalarLeafRowV18],
    boundaries: &SourceScalarBoundariesV31,
    presences: &SourceIssuedPresencesV31,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<SourceDescriptorLengthsV40> {
    relation.retain_query((|| {
        relation.query(budget)?;
        budget.reserve_storage(descriptor_length_leaf_headers_v40()?)?;
        let root_row = relation.source.root_row(root)?;
        let roster =
            root_row
                .rvalue_results
                .as_ref()
                .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                    "descriptor length source roster absent",
                ))?;
        let mut count = 0usize;
        for locator in &roster.rows {
            budget.charge_work(1)?;
            let assignment = descriptor_length_assignment_v30(relation, root, locator, budget)?;
            if descriptor_length_source_scalar_v30(relation, assignment, budget)?.is_some() {
                count = count.checked_add(1).ok_or(ArgumentResourceV1::Arithmetic)?;
            }
        }
        if count == 0 {
            return Ok(SourceDescriptorLengthsV40::empty());
        }
        let (original, physical) = relation.source.root(root, budget)?;
        let source = relation.source.source_semantic(budget)?;
        let declaration = source.functions().get(original.index() as usize).ok_or(
            ProductionSourceOwnedViewErrorV18::Binding("descriptor length source root absent"),
        )?;
        let function = relation.inventory.functions().get(physical).ok_or(
            ProductionSourceOwnedViewErrorV18::Binding("descriptor length canonical root absent"),
        )?;
        // All buffers escaping scoped ABI/origin callbacks are paid before entry.
        let mut parameters = emission_vec_v1(function.function.signature.parameters.len(), budget)
            .map_err(source_emission_error_v18)?;
        budget.charge_work(parameters.capacity())?;
        parameters.resize(function.function.signature.parameters.len(), None);
        relation.with_root_slice_abis_v36(root, budget, |index, budget| {
            for (local, row) in declaration.locals().iter().enumerate() {
                budget.charge_work(1)?;
                let SemanticLocalRoleV1::Argument(argument) = row.role() else {
                    continue;
                };
                let Some(recipe) = index.argument(
                    argument,
                    SemanticLocalIdV1::from_index(
                        u32::try_from(local).map_err(|_| ArgumentResourceV1::Arithmetic)?,
                    ),
                    budget,
                )?
                else {
                    continue;
                };
                recipe.check(budget)?;
                let SliceDefinition::FunctionArgument {
                    function: owner,
                    argument,
                } = recipe.parameter()
                else {
                    return relation
                        .source
                        .missing("descriptor length entry recipe is not a parameter");
                };
                if owner != function.coordinate || recipe.metadata_bits() != 64 {
                    return relation
                        .source
                        .missing("descriptor length entry width or owner differs");
                }
                let slot = parameters
                    .get_mut(argument as usize)
                    .ok_or(ArgumentResourceV1::Accounting)?;
                if slot.replace(recipe.element()).is_some() {
                    return relation
                        .source
                        .missing("descriptor length duplicate entry recipe");
                }
            }
            Ok::<_, ProductionSourceOwnedViewErrorV18>(())
        })?;
        let mut rows = emission_vec_v1(count, budget).map_err(source_emission_error_v18)?;
        let result = super::value_origin_v1::with_whole_value_origins_v18(
            relation,
            function.coordinate,
            budget,
            |origins, budget| {
                let result = (|| {
                    for locator in &roster.rows {
                        budget.charge_work(2)?;
                        let assignment =
                            descriptor_length_assignment_v30(relation, root, locator, budget)?;
                        let Some(element) =
                            descriptor_length_source_scalar_v30(relation, assignment, budget)?
                        else {
                            continue;
                        };
                        let SourceRvalueEndpointV30::Scalar {
                            value,
                            scalar: ScalarType::Index,
                        } = locator.endpoint
                        else {
                            return relation
                                .source
                                .missing("descriptor length original scalar endpoint differs");
                        };
                        let definition = relation
                            .inventory
                            .definition_for_value(function.coordinate, value, budget)
                            .map_err(source_pointer_inventory_error_v18)?
                            .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                                "descriptor length original value absent",
                            ))?;
                        let (operation, receiver) = descriptor_length_operation_v30(
                            relation,
                            relation.inventory,
                            definition.coordinate,
                            element,
                            budget,
                        )?;
                        relation.check_descriptor_operand_v30(root, locator, receiver, budget)?;
                        let DescriptorOriginV30::Exact(
                            parameter @ SliceDefinition::FunctionArgument {
                                function: owner,
                                argument,
                            },
                        ) = source_descriptor_origin_v30(
                            relation.inventory,
                            function,
                            origins,
                            receiver,
                            budget,
                        )
                        .map_err(source_emission_error_v18)?
                        else {
                            return relation.source.missing(
                                "descriptor length lacks exact original entry provenance",
                            );
                        };
                        if owner != function.coordinate
                            || parameters.get(argument as usize) != Some(&Some(element))
                        {
                            return relation
                                .source
                                .missing("descriptor length lacks matching admitted entry recipe");
                        }
                        if rows.len() == count || rows.len() == rows.capacity() {
                            return Err(ArgumentResourceV1::Accounting.into());
                        }
                        rows.push(DescriptorLengthRowV40 {
                            instance: locator.instance,
                            block: locator.block,
                            statement: locator.statement,
                            ty: assignment.value().result_type(),
                            operation,
                            value,
                            parameter,
                            element,
                            symbol: 0,
                        });
                    }
                    Ok(())
                })();
                relation.source.retain_aggregate_source_result_v30(result)
            },
        );
        relation.source.retain_aggregate_source_result_v30(result)?;
        if rows.len() != count {
            return Err(ArgumentResourceV1::Accounting.into());
        }
        let mut next = 0u32;
        for symbol in memory
            .iter()
            .map(|r| r.symbol)
            .chain(boundaries.rows.iter().map(|r| r.symbol))
            .chain(presences.rows.iter().map(|r| r.symbol))
        {
            budget.charge_work(1)?;
            next = next.max(
                symbol
                    .checked_add(1)
                    .ok_or(ArgumentResourceV1::Arithmetic)?,
            );
        }
        let mut groups = emission_vec_v1(rows.len(), budget).map_err(source_emission_error_v18)?;
        for (index, row) in rows.iter().enumerate() {
            budget.charge_work(1)?;
            let SliceDefinition::FunctionArgument { argument, .. } = row.parameter else {
                return Err(ArgumentResourceV1::Accounting.into());
            };
            groups.push((argument, index));
        }
        private_array_heapsort_v1(
            &mut groups,
            |row| [row.0 as usize],
            &mut SourceCorrespondenceWorkV18(budget),
            || ArgumentResourceV1::Arithmetic.into(),
        )?;
        let mut previous = None;
        for (parameter, index) in groups {
            budget.charge_work(3)?;
            if previous.is_some_and(|last| last != parameter) {
                next = next.checked_add(1).ok_or(ArgumentResourceV1::Arithmetic)?;
            }
            if next >= PRODUCTION_KERNEL_SCALAR_SYMBOL_BASE_V2 {
                return relation
                    .source
                    .missing("descriptor length exhausts source scalar names");
            }
            rows[index].symbol = next;
            previous = Some(parameter);
        }
        let mut lookup = emission_vec_v1(argument_product_v1(rows.len(), 3)?, budget)
            .map_err(source_emission_error_v18)?;
        for (index, row) in rows.iter().enumerate() {
            budget.charge_work(3)?;
            for key in [
                [0, row.instance, row.block as usize, row.statement as usize],
                [1, row.value.0 as usize, 0, 0],
                [2, row.symbol as usize, 0, 0],
            ] {
                lookup.push(DescriptorLengthLookupV40 { key, row: index });
            }
        }
        private_array_heapsort_v1(
            &mut lookup,
            |row| row.key,
            &mut SourceCorrespondenceWorkV18(budget),
            || ArgumentResourceV1::Arithmetic.into(),
        )?;
        for pair in lookup.windows(2) {
            budget.charge_work(3)?;
            if pair[0].key == pair[1].key
                && (pair[0].key[0] == 0
                    || rows[pair[0].row].parameter != rows[pair[1].row].parameter
                    || rows[pair[0].row].symbol != rows[pair[1].row].symbol)
            {
                return relation
                    .source
                    .missing("descriptor length names have conflicting original provenance");
            }
        }
        Ok(SourceDescriptorLengthsV40 { rows, lookup })
    })())
}

impl SourceScalarLeavesV18<'_, '_> {
    pub(super) fn descriptor_length_expression_v40(
        &self,
        instance: usize,
        block: u32,
        statement: u32,
        value: &PrivateRvalueV22,
        scalar: ProductionSemanticScalarTypeV2,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<ProductionSemanticExpressionV2> {
        self.relation.retain_query((|| {
            self.query(budget)?;
            let row = self
                .lengths
                .find([0, instance, block as usize, statement as usize], budget)?
                .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                    "source descriptor length is not authenticated",
                ))?;
            let (_, assignment) = self.relation.assignment_result_row_v30(
                self.root,
                instance,
                SemanticBlockIdV1::from_index(block),
                statement,
                budget,
            )?;
            budget.charge_work(3)?;
            if !std::ptr::eq(assignment.value(), value)
                || value.result_type() != row.ty
                || scalar != descriptor_length_scalar_v40()
            {
                return self
                    .relation
                    .source
                    .missing("source descriptor length owner or scalar differs");
            }
            Ok(ProductionSemanticExpressionV2::Symbol {
                symbol: row.symbol,
                scalar,
            })
        })())
    }

    pub(super) fn descriptor_length_symbol_v40(
        &self,
        symbol: u32,
        scalar: ProductionSemanticScalarTypeV2,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<bool> {
        self.relation.retain_query((|| {
            self.query(budget)?;
            if self.lengths.rows.is_empty() {
                return Ok(false);
            }
            let Some(row) = self.lengths.find([2, symbol as usize, 0, 0], budget)? else {
                return Ok(false);
            };
            if row.symbol != symbol || scalar != descriptor_length_scalar_v40() {
                return self
                    .relation
                    .source
                    .missing("source descriptor length symbol type differs");
            }
            Ok(true)
        })())
    }

    pub(super) fn descriptor_length_value_v40(
        &self,
        value: ValueId,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<Option<NormalizedScalarExpressionV1>> {
        self.relation.retain_query((|| {
            self.query(budget)?;
            if self.lengths.rows.is_empty() {
                return Ok(None);
            }
            Ok(self
                .lengths
                .find([1, value.0 as usize, 0, 0], budget)?
                .map(|row| NormalizedScalarExpressionV1::Symbol {
                    symbol: row.symbol,
                    scalar: descriptor_length_scalar_v40(),
                }))
        })())
    }
}

pub(super) fn optimized_descriptor_lengths_v40(
    source: &SourceScalarLeavesV18<'_, '_>,
    optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
    function: &CanonicalKirFunctionRefV1<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<Vec<OptimizedDescriptorLengthV40>> {
    let relation = source.relation;
    relation.retain_query((|| {
        source.query(budget)?;
        optimized_source_endpoints_v18(relation, optimized, budget)?;
        budget.reserve_storage(descriptor_length_leaf_headers_v40()?)?;
        let inventory = optimized.output_inventory(budget)?;
        let mut capacity = 0usize;
        for row in &source.lengths.rows {
            budget.charge_work(1)?;
            for _ in optimized.definition_descendants(
                SliceDefinition::Result {
                    operation: row.operation,
                    result: 0,
                },
                budget,
            )? {
                budget.charge_work(1)?;
                capacity = capacity
                    .checked_add(1)
                    .ok_or(ArgumentResourceV1::Arithmetic)?;
            }
        }
        // The output table outlives the temporary original/selected origin view.
        let mut rows = emission_vec_v1(capacity, budget).map_err(source_emission_error_v18)?;
        if source.lengths.rows.is_empty() {
            return Ok(rows);
        }
        let result = super::value_origin_v1::with_optimized_whole_value_origins_v18(
            relation,
            optimized,
            inventory,
            function.coordinate,
            budget,
            |origins, budget| {
                let result = (|| {
                    for (original, row) in source.lengths.rows.iter().enumerate() {
                        budget.charge_work(2)?;
                        let definition = SliceDefinition::Result {
                            operation: row.operation,
                            result: 0,
                        };
                        for descendant in optimized.definition_descendants(definition, budget)? {
                            budget.charge_work(4)?;
                            let (operation, receiver) = descriptor_length_operation_v30(
                                relation,
                                inventory,
                                descendant.output,
                                row.element,
                                budget,
                            )?;
                            if operation.block.function != function.coordinate {
                                return relation
                                    .source
                                    .missing("descriptor length descendant root differs");
                            }
                            let DescriptorOriginV30::Exact(
                                parameter @ SliceDefinition::FunctionArgument {
                                    function: owner,
                                    ..
                                },
                            ) = source_descriptor_origin_v30(
                                inventory, function, origins, receiver, budget,
                            )
                            .map_err(source_emission_error_v18)?
                            else {
                                return relation
                                    .source
                                    .missing("optimized descriptor length lacks entry provenance");
                            };
                            if owner != function.coordinate {
                                return relation.source.missing(
                                    "optimized descriptor length parameter owner differs",
                                );
                            }
                            issued_output_definition_v18(
                                relation,
                                optimized,
                                row.parameter,
                                parameter,
                                budget,
                            )?;
                            let actual = optimized_source_definition_row_v18(
                                inventory,
                                descendant.output,
                                budget,
                            )?;
                            let value =
                                actual
                                    .value
                                    .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                                        "optimized descriptor length result is absent",
                                    ))?;
                            if rows.len() == capacity || rows.len() == rows.capacity() {
                                return Err(ArgumentResourceV1::Accounting.into());
                            }
                            rows.push(OptimizedDescriptorLengthV40 { value, original });
                        }
                    }
                    Ok(())
                })();
                relation.source.retain_aggregate_source_result_v30(result)
            },
        );
        relation.source.retain_aggregate_source_result_v30(result)?;
        if rows.len() != capacity {
            return Err(ArgumentResourceV1::Accounting.into());
        }
        private_array_heapsort_v1(
            &mut rows,
            |row| [row.value.0 as usize],
            &mut SourceCorrespondenceWorkV18(budget),
            || ArgumentResourceV1::Arithmetic.into(),
        )?;
        for pair in rows.windows(2) {
            budget.charge_work(3)?;
            if pair[0].value == pair[1].value
                && source.lengths.rows[pair[0].original].symbol
                    != source.lengths.rows[pair[1].original].symbol
            {
                return relation
                    .source
                    .missing("optimized descriptor lengths merge different entry parameters");
            }
        }
        Ok(rows)
    })())
}

impl ProductionOptimizedSourceScalarLeavesV18<'_> {
    pub(super) fn descriptor_length_value_v40(
        &self,
        value: ValueId,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<Option<NormalizedScalarExpressionV1>> {
        let relation = self.original.leaves.relation;
        relation.retain_query((|| {
            self.check(budget)?;
            if self.lengths.is_empty() {
                return Ok(None);
            }
            let (mut low, mut high) = (0, self.lengths.len());
            while low < high {
                budget.charge_work(2)?;
                let middle = low + (high - low) / 2;
                if self.lengths[middle].value < value {
                    low = middle + 1;
                } else {
                    high = middle;
                }
            }
            budget.charge_work(2)?;
            let Some(actual) = self.lengths.get(low).filter(|row| row.value == value) else {
                return Ok(None);
            };
            let source = self
                .original
                .leaves
                .lengths
                .rows
                .get(actual.original)
                .ok_or(ArgumentResourceV1::Accounting)?;
            Ok(Some(NormalizedScalarExpressionV1::Symbol {
                symbol: source.symbol,
                scalar: descriptor_length_scalar_v40(),
            }))
        })())
    }
}
