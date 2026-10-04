/// Checked control disposition of one complete original source site.
/// This does not establish purity, memory currentness, or ranked equivalence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductionOptimizedSourceSiteControlV18 {
    /// Every physical segment is executable in the checked transition.
    Retained,
    /// Every physical segment is independently checked unreachable.
    RemovedUnreachable,
    /// Both executable and unreachable physical segments remain.
    Mixed,
    /// The original source had no physical entry or site spans.
    OriginalUnmaterialized,
}

/// Borrowed block census over the existing checked source index.
/// No operation roster is copied. Consumers retain this header with their facts
/// and must not interpret a missing original span as optimizer elimination.
pub struct ProductionOptimizedSourceBlockControlV18<'a, 'g> {
    optimized: &'a ProductionOptimizedSourceCorrespondenceV18<'g>,
    root: usize,
    instance: usize,
    block: SemanticBlockIdV1,
    statements: usize,
    disposition: ProductionOptimizedSourceSiteControlV18,
}

struct OptimizedSourceGeneratedRecipeReaderV18<'a, 'g, 'b, 'w, 'c> {
    relation: &'a ProductionSourceCorrespondenceV18<'g>,
    optimized: &'a ProductionOptimizedSourceCorrespondenceV18<'g>,
    root: usize,
    instance: usize,
    origins: &'a value_origin_v1::WholeValueOriginsV18<'g>,
    ledger: &'a CorrelationLedgerV18<'b, 'w, 'c>,
}

impl OptimizedSourceGeneratedRecipeReaderV18<'_, '_, '_, '_, '_> {
    fn query<T>(
        &self,
        query: impl FnOnce(&mut ArgumentBudgetV1<'_>) -> SourceOwnedResultV18<T>,
    ) -> Option<T> {
        match self.ledger.with_budget(|budget| {
            Ok(self.relation.retain_query((|| {
                optimized_source_endpoints_v18(self.relation, self.optimized, budget)?;
                query(budget)
            })()))
        }) {
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

    fn result_value(
        &self,
        value: ValueId,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<ValueId> {
        use fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1 as Definition;
        let input_function = fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1(
            u32::try_from(self.relation.source.root(self.root, budget)?.1)
                .map_err(|_| ArgumentResourceV1::Arithmetic)?,
        );
        let output_function =
            optimized_source_root_function_v18(self.relation, self.optimized, self.root, budget)?
                .coordinate;
        let input = self
            .relation
            .inventory
            .definition_for_value(input_function, value, budget)
            .map_err(source_pointer_inventory_error_v18)?
            .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                "optimized generated input definition",
            ))?;
        let result_role = match input.coordinate {
            Definition::Result { operation, result } => {
                match self.optimized.operation(operation, budget)? {
                    ProductionOptimizedSourceOperationV18::Retained { output, .. } => {
                        Some(Definition::Result {
                            operation: output,
                            result,
                        })
                    }
                    ProductionOptimizedSourceOperationV18::Rewritten { .. } => None,
                    ProductionOptimizedSourceOperationV18::RemovedUnreachable { .. } => {
                        return self
                            .relation
                            .source
                            .missing("unreachable generated value has no executable recipe");
                    }
                }
            }
            Definition::FunctionArgument { argument, .. } => Some(Definition::FunctionArgument {
                function: output_function,
                argument,
            }),
            Definition::BlockArgument { .. } => None,
        };
        // A retained result's exact ordinal is a physical role, unlike selecting
        // the first mapped descendant. Replaced roles must have one full-range
        // descendant; ambiguity is an explicit refusal.
        let mut chosen = None;
        for descendant in self
            .optimized
            .definition_descendants(input.coordinate, budget)?
        {
            budget.charge_work(1)?;
            if result_role.is_some_and(|role| role != descendant.output) {
                continue;
            }
            if chosen.replace(descendant.output).is_some() {
                return self
                    .relation
                    .source
                    .missing("optimized generated value has ambiguous actual descendants");
            }
        }
        let coordinate = chosen.ok_or(ProductionSourceOwnedViewErrorV18::Binding(
            "optimized generated value lost its actual result role",
        ))?;
        let output = self.optimized.output_inventory(budget)?;
        let row = optimized_source_definition_row_v18(output, coordinate, budget)?;
        if row.ty != input.ty {
            return self
                .relation
                .source
                .missing("optimized generated value changed type");
        }
        let actual = row.value.ok_or(ProductionSourceOwnedViewErrorV18::Binding(
            "optimized generated value has no physical value",
        ))?;
        if !optimized_source_value_descends_v18(
            self.relation,
            self.optimized,
            input_function,
            value,
            output_function,
            actual,
            budget,
        )? {
            return self
                .relation
                .source
                .missing("optimized generated result changed function or definition");
        }
        Ok(actual)
    }
}

impl NeutralRecipeMeterV18 for OptimizedSourceGeneratedRecipeReaderV18<'_, '_, '_, '_, '_> {
    fn charge_recipe_step(&self) -> Option<()> {
        self.ledger.with_budget(|budget| budget.charge_work(1)).ok()
    }

    fn optimized_recipe_operation(
        &self,
        block: u32,
        ordinal: usize,
    ) -> Option<(&Operation, FunctionOperationLocation)> {
        self.query(|budget| {
            let actual = match self.optimized.source_span_entry(
                self.root,
                self.instance,
                SemanticBlockIdV1::from_index(block),
                None,
                ordinal,
                budget,
            )? {
                ProductionOptimizedSourceSpanV18::Operation(
                    ProductionOptimizedSourceOperationV18::Retained { output, .. },
                ) => output,
                ProductionOptimizedSourceSpanV18::Operation(
                    ProductionOptimizedSourceOperationV18::Rewritten { .. }
                    | ProductionOptimizedSourceOperationV18::RemovedUnreachable { .. },
                )
                | ProductionOptimizedSourceSpanV18::Gap(_)
                | ProductionOptimizedSourceSpanV18::OriginalRemovedCall
                | ProductionOptimizedSourceSpanV18::OriginalNoOperations => return Ok(None),
            };
            let function = optimized_source_root_function_v18(
                self.relation,
                self.optimized,
                self.root,
                budget,
            )?;
            if actual.block.function != function.coordinate {
                return self
                    .relation
                    .source
                    .missing("optimized generated operation changed function");
            }
            let inventory = self.optimized.output_inventory(budget)?;
            let row = source_operation_row_v18(inventory, actual, budget)?;
            let block = source_block_row_v18(inventory, actual.block, budget)?;
            Ok(Some((
                row.operation,
                FunctionOperationLocation::new(block.block.id, actual.operation as usize),
            )))
        })?
    }
}

impl GeneratedRecipeSourceV18 for OptimizedSourceGeneratedRecipeReaderV18<'_, '_, '_, '_, '_> {
    fn values(&self, block: u32) -> Option<GeneratedRecipeValuesV18> {
        self.query(|budget| {
            let original = self.relation.generated_recipe_values_v18(
                self.root,
                self.instance,
                SemanticBlockIdV1::from_index(block),
                budget,
            )?;
            Ok(GeneratedRecipeValuesV18 {
                destination_local: original.destination_local,
                input: self.result_value(original.input, budget)?,
                output: self.result_value(original.output, budget)?,
            })
        })
    }

    fn operations(&self, block: u32) -> Option<NeutralRecipeOperationsV18<'_>> {
        self.query(|budget| {
            let entries = self.optimized.source_span_entry_count(
                self.root,
                self.instance,
                SemanticBlockIdV1::from_index(block),
                None,
                budget,
            )?;
            Ok(NeutralRecipeOperationsV18::Optimized {
                block,
                entries,
                meter: self,
            })
        })
    }

    fn producer_contains(&self, block: u32, location: FunctionOperationLocation) -> Option<bool> {
        let operations = self.operations(block)?;
        for ordinal in 0..operations.len() {
            self.charge_recipe_step()?;
            if operations
                .get(ordinal)
                .is_some_and(|(_, actual)| actual == location)
            {
                return Some(true);
            }
        }
        self.query(|budget| self.relation.query(budget))?;
        Some(false)
    }

    fn operation_origin(
        &self,
        body: &FunctionBody,
        _kir: &dyn KirCorrelationGraphV18,
        value: ValueId,
    ) -> Option<ValueId> {
        self.query(|budget| {
            let inventory = self.optimized.output_inventory(budget)?;
            let function = optimized_source_root_function_v18(
                self.relation,
                self.optimized,
                self.root,
                budget,
            )?;
            budget.charge_work(2)?;
            if !function
                .function
                .body
                .as_ref()
                .is_some_and(|actual| std::ptr::eq(actual, body))
                || !self.origins.belongs_to(inventory, function.coordinate)
            {
                return self
                    .relation
                    .source
                    .missing("optimized generated origin changed output inventory or body");
            }
            self.origins
                .operation_origin(value, budget)
                .map_err(source_pointer_inventory_error_v18)
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
}
