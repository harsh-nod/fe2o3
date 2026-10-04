// This child can inspect the closed source occurrence fields without exporting
// their constructors. Its only seed is the complete first actual native join;
// every later scalar/aggregate pair is replayed before final facts are rebuilt.
const AGGREGATE_GLOBAL_DEFINITIONS_V30: usize = 9;

#[cfg(test)]
include!("production_source_aggregate_global_headers_v30_tests.rs");

pub(super) struct AggregateGlobalTransportV30 {
    initial_premises: Vec<ProductionMixedSliceRuntimePremiseV26>,
    initial_occurrences: Vec<ProductionMixedRuntimeOccurrenceV26>,
    coordinates: AggregateCoordinateTransportV30,
    premise_columns: usize,
}

// Prepaid by the complete source-owned consumer, independently of vector
// backing and the nested CFG/native services' own callback/result frames.
pub(super) fn aggregate_global_headers_v30() -> Result<usize, ArgumentResourceV1> {
    type RuntimeRows = (
        Vec<ProductionMixedSliceRuntimePremiseV26>,
        Vec<ProductionMixedRuntimeOccurrenceV26>,
    );
    type Vectors = (
        Vec<SliceDefinition>,
        Vec<SliceOperation>,
        Vec<AggregateEdgeV30>,
        Vec<AggregateFunctionV30>,
        Vec<Option<(SliceOperation, SliceOperation)>>,
        Vec<fe2o3_kernel_ir::ExplicitLaunchExtent>,
        Vec<bool>,
    );
    type Endpoints = (
        GlobalSourceAccessPairV18,
        GlobalSourceAccessEndpointV18,
        GlobalSourceLogicalEndpointV18,
        [SliceDefinition; AGGREGATE_GLOBAL_DEFINITIONS_V30],
        ProductionMixedSliceRuntimePremiseV26,
        ProductionMixedRuntimeOccurrenceV26,
        GlobalSourceCfgGuardV85,
    );
    type Results = (
        Result<AggregateGlobalTransportV30, ProductionAggregateSourceErrorV30>,
        SourceOwnedResultV18<RuntimeRows>,
        SourceOwnedResultV18<Vec<Option<(SliceOperation, SliceOperation)>>>,
        SourceOwnedResultV18<Vec<fe2o3_kernel_ir::ExplicitLaunchExtent>>,
        SourceOwnedResultV18<GlobalSourceAccessPairV18>,
        SourceOwnedResultV18<Option<GlobalSourceAccessEndpointV18>>,
        SourceOwnedResultV18<[SliceDefinition; AGGREGATE_GLOBAL_DEFINITIONS_V30]>,
        SourceOwnedResultV18<usize>,
        SourceOwnedResultV18<()>,
        SourceOwnedResultV18<GlobalSourceCfgGuardV85>,
    );
    type Queries<'a> = (
        Result<
            Option<&'a fe2o3_kernel_ir::CanonicalConditionalSliceParameterV26>,
            fe2o3_kernel_ir::CanonicalGuardedGlobalReadErrorV1,
        >,
        Result<
            Option<&'a fe2o3_kernel_ir::CanonicalConditionalSliceAccessV26>,
            fe2o3_kernel_ir::CanonicalGuardedGlobalReadErrorV1,
        >,
        Result<
            Option<(
                fe2o3_kernel_ir::ExplicitLaunchExtent,
                fe2o3_kernel_ir::FormalIndexWidth,
                usize,
                usize,
            )>,
            fe2o3_kernel_ir::CanonicalGuardedGlobalReadErrorV1,
        >,
        SourceOwnedResultV18<&'a fe2o3_kernel_analysis::CanonicalKirDefinitionRefV1<'a>>,
        SourceOwnedResultV18<bool>,
        Result<
            fe2o3_kernel_ir::CanonicalGuardedGlobalReadOutcomeV18<'a, 'a>,
            fe2o3_kernel_ir::CanonicalGuardedGlobalReadErrorV1,
        >,
        Result<
            fe2o3_kernel_ir::CanonicalGuardedGlobalStoreOutcomeV24<'a, 'a>,
            fe2o3_kernel_ir::CanonicalGuardedGlobalReadErrorV1,
        >,
    );
    type Arguments<'a> = (
        &'a AggregateGlobalTransportV30,
        &'a ProductionSourceCorrespondenceV18<'a>,
        &'a fe2o3_kernel_opt::OwnedAggregateFixedpointV18,
        &'a fe2o3_kernel_analysis::CanonicalKirInventoryV18<'a>,
        &'a AggregateSourceStageV30<'a>,
        &'a mut ArgumentBudgetV1<'a>,
        &'a [fe2o3_kernel_ir::ExplicitLaunchExtent],
        [&'a (); 12],
        [usize; 20],
        [Option<SliceDefinition>; 3],
    );
    argument_sum_v1(&[
        size_of::<AggregateGlobalTransportV30>(),
        size_of::<RuntimeRows>(),
        size_of::<Vectors>(),
        std::mem::align_of::<Vectors>(),
        size_of::<Endpoints>(),
        std::mem::align_of::<Endpoints>(),
        size_of::<Results>(),
        std::mem::align_of::<Results>(),
        size_of::<Queries<'_>>(),
        std::mem::align_of::<Queries<'_>>(),
        size_of::<Arguments<'_>>(),
        std::mem::align_of::<Arguments<'_>>(),
        aggregate_definition_headers_v30()?,
        aggregate_coordinate_headers_v30()?,
    ])
}

impl AggregateStageStateV30 for AggregateGlobalTransportV30 {
    fn retained_storage(&self) -> Result<usize, ArgumentResourceV1> {
        argument_sum_v1(&[
            self.coordinates.retained_storage()?,
            aggregate_vector_credit_v30(&self.initial_premises)?,
            aggregate_vector_credit_v30(&self.initial_occurrences)?,
        ])
    }
}

fn aggregate_global_original_definitions_v30(
    input: &fe2o3_kernel_analysis::CanonicalKirInventoryV18<'_>,
    endpoint: &GlobalSourceAccessEndpointV18,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<[SliceDefinition; AGGREGATE_GLOBAL_DEFINITIONS_V30]> {
    let function = endpoint.logical.access.operation.block.function;
    let value = |value, budget: &mut ArgumentBudgetV1<'_>| {
        input
            .definition_for_value(function, value, budget)
            .map_err(source_pointer_inventory_error_v18)?
            .map(|row| row.coordinate)
            .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                "aggregate original external value",
            ))
    };
    Ok([
        endpoint.logical.root,
        endpoint.logical.index,
        endpoint.address_index,
        endpoint.logical.guard.condition(),
        value(endpoint.formation_pointer, budget)?,
        value(endpoint.pointer, budget)?,
        value(endpoint.value, budget)?,
        SliceDefinition::Result {
            operation: endpoint.logical.data,
            result: 0,
        },
        SliceDefinition::Result {
            operation: endpoint.logical.length,
            result: 0,
        },
    ])
}

impl AggregateGlobalTransportV30 {
    #[cfg(test)]
    pub(super) fn omit_last_occurrence_v30(&mut self) {
        self.initial_occurrences
            .pop()
            .expect("nonempty genuine global occurrence fixture");
    }

    #[cfg(test)]
    pub(super) fn omit_first_definition_transport_v30(&mut self, field: usize) {
        assert!(!self.initial_occurrences.is_empty());
        assert!(field < AGGREGATE_GLOBAL_DEFINITIONS_V30);
        let column = self.premise_columns + field;
        let relation = &mut self.coordinates.definitions;
        for row in relation.relation.chunks_exact_mut(relation.columns) {
            row[column] = 0;
        }
    }

    pub(super) fn into_runtime(
        self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<(
        Vec<ProductionMixedSliceRuntimePremiseV26>,
        Vec<ProductionMixedRuntimeOccurrenceV26>,
    )> {
        let Self {
            initial_premises,
            initial_occurrences,
            coordinates,
            ..
        } = self;
        let credit = coordinates.retained_storage()?;
        drop(coordinates);
        budget.release_storage(credit)?;
        Ok((initial_premises, initial_occurrences))
    }

    // The owned vectors are allocated before entering the native completion,
    // whose callback is deliberately non-escaping and exact-balance checked.
    pub(super) fn storage_vectors(
        initial: &fe2o3_kernel_analysis::CanonicalKirInventoryV18<'_>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<(
        Vec<ProductionMixedSliceRuntimePremiseV26>,
        Vec<ProductionMixedRuntimeOccurrenceV26>,
    )> {
        let premises = source_reference_emission_vec_v29(initial.definitions().len(), budget)
            .map_err(source_argument_error_v18)?;
        let occurrences = source_reference_emission_vec_v29(initial.operations().len(), budget)
            .map_err(source_argument_error_v18)?;
        Ok((premises, occurrences))
    }

    pub(super) fn fill_initial(
        completion: &CompletedGlobalSourcesV26<'_, '_>,
        original: &ProductionSourceCorrespondenceV18<'_>,
        optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
        premises: &mut Vec<ProductionMixedSliceRuntimePremiseV26>,
        occurrences: &mut Vec<ProductionMixedRuntimeOccurrenceV26>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<()> {
        completion.check_source_subject(original, optimized, budget)?;
        if !premises.is_empty()
            || !occurrences.is_empty()
            || premises.capacity() < completion.premises.len()
            || occurrences.capacity() < completion.occurrences.len()
        {
            return original
                .source
                .missing("aggregate initial runtime roster capacity or reuse");
        }
        budget.charge_work(argument_sum_v1(&[
            argument_product_v1(completion.premises.len(), 32)?,
            argument_product_v1(completion.occurrences.len(), 160)?,
        ])?)?;
        premises.extend_from_slice(completion.premises);
        occurrences.extend_from_slice(completion.occurrences);
        Ok(())
    }

    pub(super) fn seed(
        stage: &AggregateSourceStageV30<'_>,
        initial_premises: Vec<ProductionMixedSliceRuntimePremiseV26>,
        initial_occurrences: Vec<ProductionMixedRuntimeOccurrenceV26>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Self, ProductionAggregateSourceErrorV30> {
        stage.check(budget)?;
        let AggregateSourceStageRelationV30::Scalar(pair) = stage.relation else {
            return Err(ProductionSourceOwnedViewErrorV18::Binding(
                "aggregate runtime seed is not first scalar stage",
            )
            .into());
        };
        if stage.ordinal != 0 {
            return Err(ProductionSourceOwnedViewErrorV18::Binding(
                "aggregate runtime seed stage order",
            )
            .into());
        }
        let count = argument_sum_v1(&[
            initial_premises.len(),
            argument_product_v1(initial_occurrences.len(), AGGREGATE_GLOBAL_DEFINITIONS_V30)?,
        ])?;
        let mut definitions =
            source_reference_emission_vec_v29(count, budget).map_err(source_argument_error_v18)?;
        let mut operations = source_reference_emission_vec_v29(initial_occurrences.len(), budget)
            .map_err(source_argument_error_v18)?;
        let mut edges = source_reference_emission_vec_v29(initial_occurrences.len(), budget)
            .map_err(source_argument_error_v18)?;
        let roots = stage.source.root_count(budget)?;
        let mut functions =
            source_reference_emission_vec_v29(roots, budget).map_err(source_argument_error_v18)?;
        for root in 0..roots {
            let ordinal = stage.source.root(root, budget)?.1;
            functions.push(
                stage
                    .input
                    .functions()
                    .get(ordinal)
                    .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                        "aggregate original root function",
                    ))?
                    .coordinate,
            );
        }
        for premise in &initial_premises {
            budget.charge_work(4)?;
            let SliceDefinition::FunctionArgument { function, argument } = premise.parameter else {
                return Err(ProductionSourceOwnedViewErrorV18::Binding(
                    "aggregate runtime seed parameter shape",
                )
                .into());
            };
            // Function identity follows the checked map, not a numeric-ordinal
            // assumption. Parameter ordinal/type preservation is checked by it.
            let mut input = None;
            for row in pair.rows().functions {
                budget.charge_work(2)?;
                if row.output == function {
                    if input.replace(row.input).is_some() {
                        return Err(ProductionSourceOwnedViewErrorV18::Binding(
                            "aggregate runtime seed function ambiguity",
                        )
                        .into());
                    }
                }
            }
            definitions.push(SliceDefinition::FunctionArgument {
                function: input.ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                    "aggregate runtime seed function absent",
                ))?,
                argument,
            });
        }
        for occurrence in &initial_occurrences {
            budget.charge_work(8)?;
            if occurrence.premise >= initial_premises.len() {
                return Err(ProductionSourceOwnedViewErrorV18::Binding(
                    "aggregate runtime seed premise index",
                )
                .into());
            }
            definitions.extend_from_slice(&aggregate_global_original_definitions_v30(
                stage.input,
                &occurrence.source.input,
                budget,
            )?);
            operations.push(occurrence.source.input.logical.access.operation);
            edges.push(occurrence.source.input.logical.guard.require_cfg_v26()?.edge);
        }
        let coordinates = AggregateCoordinateTransportV30::seed(
            stage.input,
            &definitions,
            &operations,
            &edges,
            &functions,
            budget,
        )?;
        let temporary = argument_sum_v1(&[
            aggregate_vector_credit_v30(&definitions)?,
            aggregate_vector_credit_v30(&operations)?,
            aggregate_vector_credit_v30(&edges)?,
            aggregate_vector_credit_v30(&functions)?,
        ])?;
        drop(functions);
        drop(edges);
        drop(operations);
        drop(definitions);
        budget.release_storage(temporary)?;
        Ok(Self {
            premise_columns: initial_premises.len(),
            initial_premises,
            initial_occurrences,
            coordinates,
        })
    }

    pub(super) fn advance(
        self,
        stage: &AggregateSourceStageV30<'_>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Self, ProductionAggregateSourceErrorV30> {
        let Self {
            initial_premises,
            initial_occurrences,
            coordinates,
            premise_columns,
        } = self;
        let coordinates = coordinates.advance(stage, budget)?;
        Ok(Self {
            initial_premises,
            initial_occurrences,
            coordinates,
            premise_columns,
        })
    }

    fn check_final(
        &self,
        chain: &fe2o3_kernel_opt::OwnedAggregateFixedpointV18,
        output: &fe2o3_kernel_analysis::CanonicalKirInventoryV18<'_>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<usize> {
        budget.charge_work(12)?;
        let stages = argument_product_v1(chain.rounds().len(), 2)?;
        let columns = argument_sum_v1(&[
            self.initial_premises.len(),
            argument_product_v1(
                self.initial_occurrences.len(),
                AGGREGATE_GLOBAL_DEFINITIONS_V30,
            )?,
        ])?;
        let relation = &self.coordinates.definitions;
        if !std::ptr::eq(chain.owner(), output.owner())
            || relation.owner != std::ptr::from_ref(output.owner()) as usize
            || relation.next_stage != stages
            || relation.rows != output.definitions().len()
            || relation.columns != columns
            || self.premise_columns != self.initial_premises.len()
            || relation.relation.len() != argument_product_v1(relation.rows, columns)?
            || self.coordinates.operations.len() != self.initial_occurrences.len()
            || self.coordinates.edges.len() != self.initial_occurrences.len()
        {
            return Err(ProductionSourceOwnedViewErrorV18::Binding(
                "aggregate complete external chain census",
            ));
        }
        Ok(stages)
    }

    // Reuse the paid actual-definition index and typed cast/CFG walker. One CFG
    // scope per function serves every formation/data query; no per-access graph
    // is reconstructed and no origin is selected merely by equal numeric value.
    fn pointer_rows(
        &self,
        original: &ProductionSourceCorrespondenceV18<'_>,
        chain: &fe2o3_kernel_opt::OwnedAggregateFixedpointV18,
        output: &fe2o3_kernel_analysis::CanonicalKirInventoryV18<'_>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<Vec<Option<(SliceOperation, SliceOperation)>>> {
        self.check_final(chain, output, budget)?;
        let count = self.initial_occurrences.len();
        for &operation in &self.coordinates.operations {
            aggregate_operation_index_v30(output, operation, budget)?;
        }
        let mut rows =
            source_reference_emission_vec_v29(count, budget).map_err(source_argument_error_v18)?;
        budget.charge_work(count)?;
        rows.resize(count, None);
        for function in output.functions() {
            budget.charge_work(2)?;
            if function.function.body.is_none() {
                continue;
            }
            let local = self
                .coordinates
                .operations
                .iter()
                .filter(|operation| operation.block.function == function.coordinate)
                .count();
            budget.charge_work(count)?;
            if local == 0 {
                continue;
            }
            source_scalar_normalization_scratch_v18(
                original.source.cleanup,
                budget,
                0,
                |budget| {
                    let result = (|| {
                        let actual =
                            SourceIssuedActualV29::from_function(function.function, budget)
                                .map_err(source_emission_error_v18)?;
                        let definition_count = function.definitions.len();
                        let queries =
                            argument_product_v1(2, argument_product_v1(local, definition_count)?)?;
                        source_issued_pointer_walk_quote_v26(actual.values.len(), queries, budget)
                            .map_err(source_emission_error_v18)?;
                        budget.charge_work(argument_sum_v1(&[
                            argument_product_v1(queries, 24)?,
                            count,
                        ])?)?;
                        budget.reserve_storage(size_of::<(
                            SourceIssuedActualV29<'_>,
                            [usize; 16],
                            [&(); 12],
                            [Option<ValueId>; 4],
                            [Option<SliceOperation>; 3],
                        )>())?;
                        let mut valid = true;
                        fe2o3_kernel_ir::with_function_control_flow_v1(
                            function.function,
                            Default::default(),
                            budget,
                            |view| {
                                for (occurrence, operation) in
                                    self.coordinates.operations.iter().enumerate()
                                {
                                    if operation.block.function != function.coordinate {
                                        continue;
                                    }
                                    let block = &output.blocks()
                                        [function.blocks.start + operation.block.block as usize];
                                    let actual_operation = &output.operations()
                                        [block.operations.start + operation.operation as usize];
                                    let pointer = match actual_operation.operation.kind {
                                        OperationKind::Load { pointer, .. }
                                        | OperationKind::Store { pointer, .. } => pointer,
                                        _ => {
                                            valid = false;
                                            continue;
                                        }
                                    };
                                    let column = self.premise_columns
                                        + occurrence * AGGREGATE_GLOBAL_DEFINITIONS_V30;
                                    let mut formation = None;
                                    for index in function.definitions.clone() {
                                        if self.coordinates.definitions.relation[index
                                            * self.coordinates.definitions.columns
                                            + column
                                            + 4]
                                            == 0
                                        {
                                            continue;
                                        }
                                        let definition = &output.definitions()[index];
                                        let SliceDefinition::Result {
                                            operation: coordinate,
                                            result: 0,
                                        } = definition.coordinate
                                        else {
                                            continue;
                                        };
                                        let defining_block =
                                            &output.blocks()[function.blocks.start
                                                + coordinate.block.block as usize];
                                        let defining =
                                            &output.operations()[defining_block.operations.start
                                                + coordinate.operation as usize];
                                        let OperationKind::GetElementPointer { base, .. } =
                                            defining.operation.kind
                                        else {
                                            continue;
                                        };
                                        let Some(value) = definition.value else {
                                            continue;
                                        };
                                        if source_reference_pointer_transport_until_v29(
                                            &actual, pointer, value, view,
                                        )? == Some(value)
                                        {
                                            if formation.replace((coordinate, base)).is_some() {
                                                valid = false;
                                            }
                                        }
                                    }
                                    let Some((formation, base)) = formation else {
                                        valid = false;
                                        continue;
                                    };
                                    let mut data = None;
                                    for index in function.definitions.clone() {
                                        if self.coordinates.definitions.relation[index
                                            * self.coordinates.definitions.columns
                                            + column
                                            + 7]
                                            == 0
                                        {
                                            continue;
                                        }
                                        let definition = &output.definitions()[index];
                                        let SliceDefinition::Result {
                                            operation: coordinate,
                                            result: 0,
                                        } = definition.coordinate
                                        else {
                                            continue;
                                        };
                                        let defining_block =
                                            &output.blocks()[function.blocks.start
                                                + coordinate.block.block as usize];
                                        let defining =
                                            &output.operations()[defining_block.operations.start
                                                + coordinate.operation as usize];
                                        if !matches!(
                                            defining.operation.kind,
                                            OperationKind::SliceData { .. }
                                        ) {
                                            continue;
                                        }
                                        let Some(value) = definition.value else {
                                            continue;
                                        };
                                        if source_reference_pointer_transport_until_v29(
                                            &actual, base, value, view,
                                        )? == Some(value)
                                        {
                                            if data.replace(coordinate).is_some() {
                                                valid = false;
                                            }
                                        }
                                    }
                                    let Some(data) = data else {
                                        valid = false;
                                        continue;
                                    };
                                    if rows[occurrence].replace((formation, data)).is_some() {
                                        valid = false;
                                    }
                                }
                                Ok(())
                            },
                        )
                        .map_err(|error| match error {
                            fe2o3_kernel_ir::FunctionControlFlowScopeErrorV1::Resource(error) => {
                                ProductionSourceOwnedViewErrorV18::Resource(error)
                            }
                            _ => ProductionSourceOwnedViewErrorV18::Binding(
                                "aggregate final pointer control transport",
                            ),
                        })?;
                        if !valid {
                            return original
                                .source
                                .missing("aggregate final pointer source occurrence");
                        }
                        Ok(())
                    })();
                    original.source.retain_aggregate_source_result_v30(result)
                },
            )?;
        }
        budget.charge_work(rows.len())?;
        if rows.iter().any(Option::is_none) {
            return original
                .source
                .missing("aggregate final pointer occurrence census");
        }
        Ok(rows)
    }

    fn final_pair(
        &self,
        original: &ProductionSourceCorrespondenceV18<'_>,
        chain: &fe2o3_kernel_opt::OwnedAggregateFixedpointV18,
        output: &fe2o3_kernel_analysis::CanonicalKirInventoryV18<'_>,
        occurrence: usize,
        pointers: (SliceOperation, SliceOperation),
        root: ValueId,
        index: fe2o3_kernel_ir::CanonicalGuardedReadIndexOriginV1,
        length: ValueId,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<GlobalSourceAccessPairV18> {
        let stages = self.check_final(chain, output, budget)?;
        let initial = self.initial_occurrences.get(occurrence).ok_or(
            ProductionSourceOwnedViewErrorV18::Binding(
                "aggregate final external occurrence ordinal",
            ),
        )?;
        let operation = self.coordinates.operations[occurrence];
        let edge = self.coordinates.edges[occurrence];
        let function = operation.block.function;
        let definition = |value, budget: &mut ArgumentBudgetV1<'_>| {
            output
                .definition_for_value(function, value, budget)
                .map_err(source_pointer_inventory_error_v18)?
                .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                    "aggregate final external definition",
                ))
        };
        let index = match index {
            fe2o3_kernel_ir::CanonicalGuardedReadIndexOriginV1::ProvenOrigin(value)
            | fe2o3_kernel_ir::CanonicalGuardedReadIndexOriginV1::ExactBlockParameter(value) => {
                value
            }
        };
        let SliceDefinition::Result {
            operation: length,
            result: 0,
        } = definition(length, budget)?.coordinate
        else {
            return original
                .source
                .missing("aggregate final length occurrence shape");
        };
        let logical = GlobalSourceLogicalEndpointV18 {
            access: SliceAccess {
                operation,
                effect: initial.source.input.logical.access.effect,
            },
            root: definition(root, budget)?.coordinate,
            index: definition(index, budget)?.coordinate,
            data: pointers.1,
            length,
            address: pointers.0,
            guard: GlobalSourceGuardV85::CfgEdge(GlobalSourceCfgGuardV85 {
                condition: global_source_guard_definition_v18(output, edge.source, budget)?,
                edge,
            }),
        };
        let endpoint = global_source_endpoint_v18(output, logical, &initial.source.origin, budget)?
            .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                "aggregate final global endpoint is unsupported",
            ))?;
        let actual = aggregate_global_original_definitions_v30(output, &endpoint, budget)?;
        for (field, definition) in actual.into_iter().enumerate() {
            let column =
                self.premise_columns + occurrence * AGGREGATE_GLOBAL_DEFINITIONS_V30 + field;
            if !self
                .coordinates
                .definitions
                .contains(output, stages, column, definition, budget)?
            {
                return original.source.missing(
                    "aggregate final external definition lacks complete source transport",
                );
            }
        }
        if endpoint.scalar != initial.source.input.scalar
            || endpoint.memory != initial.source.input.memory
            || endpoint.writing != initial.source.input.writing
        {
            return original
                .source
                .missing("aggregate final external memory contract differs");
        }
        Ok(GlobalSourceAccessPairV18 {
            instance: initial.source.instance,
            origin: initial.source.origin,
            input: initial.source.input,
            output: endpoint,
        })
    }

    pub(super) fn function_launches(
        &self,
        original: &ProductionSourceCorrespondenceV18<'_>,
        chain: &fe2o3_kernel_opt::OwnedAggregateFixedpointV18,
        output: &fe2o3_kernel_analysis::CanonicalKirInventoryV18<'_>,
        launches: &[fe2o3_kernel_ir::ExplicitLaunchExtent],
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<Vec<fe2o3_kernel_ir::ExplicitLaunchExtent>> {
        self.check_final(chain, output, budget)?;
        if launches.len() != original.source.root_count(budget)?
            || launches.len() != self.coordinates.functions.len()
        {
            return original
                .source
                .missing("aggregate final source launch roster");
        }
        let mut function_launches =
            source_reference_emission_vec_v29(output.functions().len(), budget)
                .map_err(source_argument_error_v18)?;
        budget.charge_work(output.functions().len())?;
        function_launches.resize(
            output.functions().len(),
            fe2o3_kernel_ir::ExplicitLaunchExtent::Exact {
                rank: 1,
                extents: [1, 1, 1],
            },
        );
        let mut seen = source_reference_emission_vec_v29(output.functions().len(), budget)
            .map_err(source_argument_error_v18)?;
        budget.charge_work(output.functions().len())?;
        seen.resize(output.functions().len(), false);
        for (&coordinate, &launch) in self.coordinates.functions.iter().zip(launches) {
            budget.charge_work(4)?;
            let index = coordinate.0 as usize;
            if output
                .functions()
                .get(index)
                .is_none_or(|row| row.coordinate != coordinate)
                || seen.get(index) != Some(&false)
            {
                return original
                    .source
                    .missing("aggregate final repeated or absent root function");
            }
            seen[index] = true;
            function_launches[index] = launch;
        }
        let credit = aggregate_vector_credit_v30(&seen)?;
        drop(seen);
        budget.release_storage(credit)?;
        Ok(function_launches)
    }

    // Called inside the actual final native callback. Only existing vector
    // elements are replaced; all temporary pointer/domain scratch is settled
    // before the pending native scope's exact-balance postflight.
    pub(super) fn finish_domains(
        &mut self,
        original: &ProductionSourceCorrespondenceV18<'_>,
        chain: &fe2o3_kernel_opt::OwnedAggregateFixedpointV18,
        output: &fe2o3_kernel_analysis::CanonicalKirInventoryV18<'_>,
        native: &fe2o3_pliron::PendingCanonicalMixedMemoryPoliciesV26<'_, '_>,
        reads: &GlobalReadFactsV18<'_, '_>,
        stores: &GlobalStoreFactsV25<'_, '_>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<()> {
        original.check(budget)?;
        let stages = self.check_final(chain, output, budget)?;
        if !std::ptr::eq(
            native.owner(budget).map_err(slice_entry_native_error_v25)?,
            output.owner(),
        ) {
            return original.source.missing("aggregate final native owner");
        }
        let globals = native
            .conditional_globals(budget)
            .map_err(slice_entry_native_error_v25)?;
        let result = source_scalar_normalization_scratch_v18(
            original.source.cleanup,
            budget,
            0,
            |budget| {
                let result = (|| {
                    budget.reserve_storage(argument_sum_v1(&[
                        global_read_condition_headers_v18(0, 1)?,
                        slice_store_domain_headers_v25()?,
                        source_global_domain_join_headers_v30()?,
                    ])?)?;
                    let pointers = self.pointer_rows(original, chain, output, budget)?;
                    for ordinal in 0..self.initial_premises.len() {
                        budget.charge_work(32)?;
                        let mut premise = self.initial_premises[ordinal];
                        let mut parameter = None;
                        for definition in output.definitions() {
                            budget.charge_work(2)?;
                            if !matches!(
                                definition.coordinate,
                                SliceDefinition::FunctionArgument { .. }
                            ) {
                                continue;
                            }
                            if self.coordinates.definitions.contains(
                                output,
                                stages,
                                ordinal,
                                definition.coordinate,
                                budget,
                            )? {
                                if parameter.replace(definition.coordinate).is_some() {
                                    return original
                                        .source
                                        .missing("aggregate final source parameter is ambiguous");
                                }
                            }
                        }
                        let Some(SliceDefinition::FunctionArgument { function, argument }) =
                            parameter
                        else {
                            return original
                                .source
                                .missing("aggregate final source parameter absent");
                        };
                        let actual = globals
                            .parameter(function, argument, budget)
                            .map_err(|error| {
                                optimized_source_observed_formal_error_v18(original, &error)
                            })?
                            .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                                "aggregate final conditional parameter absent",
                            ))?;
                        let conditions = globals
                            .function_conditions(function, budget)
                            .map_err(|error| {
                                optimized_source_observed_formal_error_v18(original, &error)
                            })?
                            .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                                "aggregate final conditional function absent",
                            ))?;
                        if self.coordinates.functions.get(premise.root) != Some(&function)
                            || (conditions.0, conditions.1) != (premise.launch, premise.width)
                            || (
                                actual.scalar(),
                                actual.reads(),
                                actual.writes(),
                                actual.invocation_axis(),
                            ) != (premise.scalar, premise.reads, premise.writes, premise.axis)
                            || (premise.writes != 0
                                && (!premise.exclusive_contract
                                    || !actual.requires_exclusive_runtime_binding()
                                    || !actual.requires_exact_launch_binding()))
                            || (premise.reads != 0 && !actual.requires_initialized_extent())
                            || actual.requires_valid_aligned_extent()
                                != (premise.reads != 0 || premise.writes != 0)
                        {
                            return original
                                .source
                                .missing("aggregate final source runtime parameter differs");
                        }
                        premise.parameter = parameter.expect("checked actual parameter");
                        self.initial_premises[ordinal] = premise;
                    }
                    for ordinal in 0..self.initial_occurrences.len() {
                        budget.charge_work(32)?;
                        let initial = self.initial_occurrences[ordinal];
                        let operation = self.coordinates.operations[ordinal];
                        let fact = globals
                            .access_at(operation, budget)
                            .map_err(|error| {
                                optimized_source_observed_formal_error_v18(original, &error)
                            })?
                            .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                                "aggregate final conditional access absent",
                            ))?;
                        let domain = fact.domain();
                        if domain.writing() != initial.source.input.writing {
                            return original
                                .source
                                .missing("aggregate final global access kind");
                        }
                        let pointers =
                            pointers[ordinal].ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                                "aggregate final pointer row absent",
                            ))?;
                        let pair = if domain.writing() {
                            let fe2o3_kernel_ir::CanonicalGuardedGlobalStoreOutcomeV24::ProvedLocalConditions(local) = stores.store_at(operation, budget)
                        .map_err(|error| optimized_source_observed_formal_error_v18(original, &error))?
                        else { return original.source.missing("aggregate final Store conditions remain open"); };
                            let pair = self.final_pair(
                                original,
                                chain,
                                output,
                                ordinal,
                                pointers,
                                local.domain().slice(),
                                local.normalized_index_origin(),
                                local.normalized_length_origin(),
                                budget,
                            )?;
                            check_source_store_endpoint_prepaid_v30(
                                original, output, stores, &pair, &local, budget,
                            )?;
                            if domain
                                != fe2o3_kernel_ir::CanonicalConditionalSliceDomainV26::Store(
                                    local.domain(),
                                )
                            {
                                return original
                                    .source
                                    .missing("aggregate final Store conditional fact differs");
                            }
                            pair
                        } else {
                            let fe2o3_kernel_ir::CanonicalGuardedGlobalReadOutcomeV18::ProvedLocalConditions(local) = reads.read_at(operation, budget)
                        .map_err(|error| optimized_source_observed_formal_error_v18(original, &error))?
                        else { return original.source.missing("aggregate final read conditions remain open"); };
                            let pair = self.final_pair(
                                original,
                                chain,
                                output,
                                ordinal,
                                pointers,
                                local.domain().slice(),
                                local.normalized_index_origin(),
                                local.normalized_length_origin(),
                                budget,
                            )?;
                            check_source_read_endpoint_prepaid_v30(
                                original, output, reads, &pair, &local, budget,
                            )
                            .map_err(slice_completion_read_error_v25)?;
                            if domain
                                != fe2o3_kernel_ir::CanonicalConditionalSliceDomainV26::Read(
                                    *local.domain(),
                                )
                            {
                                return original
                                    .source
                                    .missing("aggregate final read conditional fact differs");
                            }
                            pair
                        };
                        let premise = self.initial_premises.get(initial.premise).ok_or(
                            ProductionSourceOwnedViewErrorV18::Binding(
                                "aggregate final occurrence premise absent",
                            ),
                        )?;
                        if pair.output.logical.root != premise.parameter
                            || pair.output.pointer != domain.pointer()
                            || fact.invocation_projection().map(|value| value.0)
                                != initial.projection.map(|value| value.0)
                        {
                            return original
                                .source
                                .missing("aggregate final occurrence runtime contract differs");
                        }
                        self.initial_occurrences[ordinal] = ProductionMixedRuntimeOccurrenceV26 {
                            premise: initial.premise,
                            source: pair,
                            cfg_guard: pair.output.logical.guard.require_cfg_v26()?,
                            domain,
                            projection: fact.invocation_projection(),
                        };
                    }
                    if globals.access_count(budget).map_err(|error| {
                        optimized_source_observed_formal_error_v18(original, &error)
                    })? != self.initial_occurrences.len()
                    {
                        return original
                            .source
                            .missing("aggregate final complete global access census");
                    }
                    let mut count = 0usize;
                    for function in output.functions() {
                        for argument in 0..function.function.signature.parameters.len() {
                            budget.charge_work(2)?;
                            if globals
                                .parameter(
                                    function.coordinate,
                                    u32::try_from(argument)
                                        .map_err(|_| ArgumentResourceV1::Arithmetic)?,
                                    budget,
                                )
                                .map_err(|error| {
                                    optimized_source_observed_formal_error_v18(original, &error)
                                })?
                                .is_some()
                            {
                                count = argument_sum_v1(&[count, 1])?;
                            }
                        }
                    }
                    if count != self.initial_premises.len() {
                        return original
                            .source
                            .missing("aggregate final complete conditional parameter census");
                    }
                    Ok(())
                })();
                original.source.retain_aggregate_source_result_v30(result)
            },
        );
        original.retain_query(result)
    }
}
