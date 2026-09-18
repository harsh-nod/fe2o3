// These locators retain original-N custody. Only the live checked deletion
// relation supplies E coordinates; a copied disposition is inert metadata.
type ErasedFunctionV1 = fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1;
type ErasedBlockV1 = fe2o3_kernel_ir::CanonicalKirBlockCoordinateV1;
type ErasedOperationV1 = fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1;
type ErasedDefinitionV1 = fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1;
type ErasedOperandV1 = fe2o3_kernel_ir::CanonicalKirUseCoordinateV1;
type ErasedEdgeV1 = fe2o3_kernel_ir::CanonicalKirEdgeCoordinateV1;
// The complete scoped query surface is exercised by the occurrence tests.
#[cfg_attr(not(test), allow(dead_code))]
type ErasedEdgeArgumentV1 = fe2o3_kernel_ir::CanonicalKirEdgeArgumentCoordinateV1;
#[cfg_attr(not(test), allow(dead_code))]
type ErasedAccessV1 = fe2o3_kernel_ir::CanonicalKirAccessCoordinateV1;
type ErasedOccurrenceResultV1<T> = Result<T, ProductionSourceOutputErrorV1>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ErasedSourceOccurrenceV1<T> {
    Retained(T),
    DeletedUnitCall,
    DeletedLocalHelper,
}

impl<T> ErasedSourceOccurrenceV1<T> {
    fn map<U>(self, next: impl FnOnce(T) -> U) -> ErasedSourceOccurrenceV1<U> {
        match self {
            Self::Retained(value) => ErasedSourceOccurrenceV1::Retained(next(value)),
            Self::DeletedUnitCall => ErasedSourceOccurrenceV1::DeletedUnitCall,
            Self::DeletedLocalHelper => ErasedSourceOccurrenceV1::DeletedLocalHelper,
        }
    }

    fn retained(self) -> ErasedOccurrenceResultV1<T> {
        match self {
            Self::Retained(value) => Ok(value),
            _ => Err(erased_occurrence_invalid_v1("required retained occurrence")),
        }
    }
}

fn erased_occurrence_invalid_v1(detail: &'static str) -> ProductionSourceOutputErrorV1 {
    ProductionSourceOutputErrorV1::Invalid(detail)
}

fn erased_occurrence_charge_v1(
    budget: &mut AssertOriginBudgetV1<'_>,
    work: usize,
) -> ErasedOccurrenceResultV1<()> {
    budget
        .charge_work(work)
        .map_err(ProductionSourceOutputErrorV1::Resource)
}

fn erased_occurrence_index_v1(
    range: std::ops::Range<usize>,
    ordinal: u32,
) -> ErasedOccurrenceResultV1<usize> {
    let index = range.start.checked_add(ordinal as usize).ok_or(
        ProductionSourceOutputErrorV1::Resource(AssertOriginResourceV1::Arithmetic),
    )?;
    if index >= range.end {
        return Err(erased_occurrence_invalid_v1("original occurrence ordinal"));
    }
    Ok(index)
}

struct ErasedSourceCoordinateMapV1<'s> {
    deletion: &'s CheckedUnitLocalCallDeletionV1<'s>,
    floor: usize,
}

impl ErasedSourceCoordinateMapV1<'_> {
    fn inventory(&self) -> &CanonicalKirInventoryV1<'_> {
        self.deletion.stage.source.inventory
    }

    fn live(&self, budget: &mut AssertOriginBudgetV1<'_>) -> ErasedOccurrenceResultV1<()> {
        self.deletion
            .require_live(budget)
            .map_err(ProductionSourceOutputErrorV1::SourceReplay)?;
        erased_occurrence_charge_v1(budget, 2)?;
        if budget.storage() < self.floor {
            return Err(ProductionSourceOutputErrorV1::Resource(
                AssertOriginResourceV1::Accounting,
            ));
        }
        Ok(())
    }

    fn original_function(
        &self,
        original: ErasedFunctionV1,
        budget: &mut AssertOriginBudgetV1<'_>,
    ) -> ErasedOccurrenceResultV1<&fe2o3_kernel_analysis::CanonicalKirFunctionRefV1<'_>> {
        self.live(budget)?;
        erased_occurrence_charge_v1(budget, 3)?;
        self.inventory()
            .functions()
            .get(original.0 as usize)
            .filter(|row| row.coordinate == original)
            .ok_or_else(|| erased_occurrence_invalid_v1("original function coordinate"))
    }

    fn original_block(
        &self,
        original: ErasedBlockV1,
        budget: &mut AssertOriginBudgetV1<'_>,
    ) -> ErasedOccurrenceResultV1<&fe2o3_kernel_analysis::CanonicalKirBlockRefV1<'_>> {
        let function = self.original_function(original.function, budget)?;
        erased_occurrence_charge_v1(budget, 4)?;
        let index = erased_occurrence_index_v1(function.blocks.clone(), original.block)?;
        self.inventory()
            .blocks()
            .get(index)
            .filter(|row| row.coordinate == original)
            .ok_or_else(|| erased_occurrence_invalid_v1("original block coordinate"))
    }

    fn original_operation(
        &self,
        original: ErasedOperationV1,
        budget: &mut AssertOriginBudgetV1<'_>,
    ) -> ErasedOccurrenceResultV1<&fe2o3_kernel_analysis::CanonicalKirOperationRefV1<'_>> {
        let block = self.original_block(original.block, budget)?;
        erased_occurrence_charge_v1(budget, 4)?;
        let index = erased_occurrence_index_v1(block.operations.clone(), original.operation)?;
        self.inventory()
            .operations()
            .get(index)
            .filter(|row| row.coordinate == original)
            .ok_or_else(|| erased_occurrence_invalid_v1("original operation coordinate"))
    }

    fn original_edge(
        &self,
        original: ErasedEdgeV1,
        budget: &mut AssertOriginBudgetV1<'_>,
    ) -> ErasedOccurrenceResultV1<&fe2o3_kernel_analysis::CanonicalKirEdgeRefV1<'_>> {
        let block = self.original_block(original.source, budget)?;
        erased_occurrence_charge_v1(budget, 4)?;
        let index = erased_occurrence_index_v1(block.edges.clone(), original.successor)?;
        self.inventory()
            .edges()
            .get(index)
            .filter(|row| row.coordinate == original)
            .ok_or_else(|| erased_occurrence_invalid_v1("original edge coordinate"))
    }

    fn canonical_function(
        &self,
        original: ErasedFunctionV1,
        budget: &mut AssertOriginBudgetV1<'_>,
    ) -> ErasedOccurrenceResultV1<ErasedSourceOccurrenceV1<ErasedFunctionV1>> {
        self.original_function(original, budget)?;
        let mapped = self
            .deletion
            .function(original.0, budget)
            .map_err(ProductionSourceOutputErrorV1::SourceReplay)?;
        Ok(match mapped {
            Some(function) => ErasedSourceOccurrenceV1::Retained(
                fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1(function),
            ),
            None => ErasedSourceOccurrenceV1::DeletedLocalHelper,
        })
    }

    fn function(
        &self,
        root: SemanticFunctionIdV1,
        function: SemanticFunctionIdV1,
        budget: &mut AssertOriginBudgetV1<'_>,
    ) -> ErasedOccurrenceResultV1<ErasedSourceOccurrenceV1<ErasedFunctionV1>> {
        let original = self.source_function(root, function, budget)?;
        self.canonical_function(original, budget)
    }

    fn source_function(
        &self,
        root: SemanticFunctionIdV1,
        function: SemanticFunctionIdV1,
        budget: &mut AssertOriginBudgetV1<'_>,
    ) -> ErasedOccurrenceResultV1<ErasedFunctionV1> {
        self.live(budget)?;
        let origins = self.deletion.source().assert_origins();
        let index = assert_origin_find_v1(&origins.origins.functions, budget, |row, budget| {
            budget.charge_work(2)?;
            Ok((row.owner, row.function).cmp(&(root, function)))
        })
        .map_err(ProductionSourceOutputErrorV1::SourceOrigin)?
        .ok_or_else(|| erased_occurrence_invalid_v1("source function alias"))?;
        erased_occurrence_charge_v1(budget, 1)?;
        Ok(origins.origins.functions[index].canonical)
    }

    fn block(
        &self,
        original: ErasedBlockV1,
        budget: &mut AssertOriginBudgetV1<'_>,
    ) -> ErasedOccurrenceResultV1<ErasedSourceOccurrenceV1<ErasedBlockV1>> {
        self.original_block(original, budget)?;
        Ok(self
            .canonical_function(original.function, budget)?
            .map(|function| ErasedBlockV1 {
                function,
                block: original.block,
            }))
    }

    fn operation(
        &self,
        original: ErasedOperationV1,
        budget: &mut AssertOriginBudgetV1<'_>,
    ) -> ErasedOccurrenceResultV1<ErasedSourceOccurrenceV1<ErasedOperationV1>> {
        self.original_operation(original, budget)?;
        match self
            .deletion
            .operation(original, budget)
            .map_err(ProductionSourceOutputErrorV1::SourceReplay)?
        {
            Some(ProductionUnitLocalOperationDeletionV1::Retained(mapped)) => {
                Ok(ErasedSourceOccurrenceV1::Retained(mapped))
            }
            Some(ProductionUnitLocalOperationDeletionV1::DeletedUnitCall) => {
                Ok(ErasedSourceOccurrenceV1::DeletedUnitCall)
            }
            Some(ProductionUnitLocalOperationDeletionV1::DeletedLocalHelper) => {
                Ok(ErasedSourceOccurrenceV1::DeletedLocalHelper)
            }
            None => Err(erased_occurrence_invalid_v1(
                "complete original operation disposition",
            )),
        }
    }

    fn definition(
        &self,
        original: ErasedDefinitionV1,
        budget: &mut AssertOriginBudgetV1<'_>,
    ) -> ErasedOccurrenceResultV1<ErasedSourceOccurrenceV1<ErasedDefinitionV1>> {
        let (range, ordinal) = match original {
            ErasedDefinitionV1::FunctionArgument { function, argument } => {
                let row = self.original_function(function, budget)?;
                (row.definitions.clone(), argument)
            }
            ErasedDefinitionV1::BlockArgument { block, argument } => (
                self.original_block(block, budget)?.parameters.clone(),
                argument,
            ),
            ErasedDefinitionV1::Result { operation, result } => (
                self.original_operation(operation, budget)?.results.clone(),
                result,
            ),
        };
        erased_occurrence_charge_v1(budget, 4)?;
        let index = erased_occurrence_index_v1(range, ordinal)?;
        if self
            .inventory()
            .definitions()
            .get(index)
            .map(|row| row.coordinate)
            != Some(original)
        {
            return Err(erased_occurrence_invalid_v1(
                "original definition coordinate",
            ));
        }
        Ok(match original {
            ErasedDefinitionV1::FunctionArgument { function, argument } => self
                .canonical_function(function, budget)?
                .map(|function| ErasedDefinitionV1::FunctionArgument { function, argument }),
            ErasedDefinitionV1::BlockArgument { block, argument } => self
                .block(block, budget)?
                .map(|block| ErasedDefinitionV1::BlockArgument { block, argument }),
            ErasedDefinitionV1::Result { operation, result } => self
                .operation(operation, budget)?
                .map(|operation| ErasedDefinitionV1::Result { operation, result }),
        })
    }

    fn operand(
        &self,
        original: ErasedOperandV1,
        budget: &mut AssertOriginBudgetV1<'_>,
    ) -> ErasedOccurrenceResultV1<ErasedSourceOccurrenceV1<ErasedOperandV1>> {
        let (range, ordinal) = match original {
            ErasedOperandV1::OperationOperand { operation, operand } => (
                self.original_operation(operation, budget)?.operands.clone(),
                operand,
            ),
            ErasedOperandV1::TerminatorOperand { block, operand } => (
                self.original_block(block, budget)?.terminator_uses.clone(),
                operand,
            ),
        };
        erased_occurrence_charge_v1(budget, 4)?;
        let index = erased_occurrence_index_v1(range, ordinal)?;
        if self.inventory().uses().get(index).map(|row| row.coordinate) != Some(original) {
            return Err(erased_occurrence_invalid_v1("original operand coordinate"));
        }
        Ok(match original {
            ErasedOperandV1::OperationOperand { operation, operand } => self
                .operation(operation, budget)?
                .map(|operation| ErasedOperandV1::OperationOperand { operation, operand }),
            ErasedOperandV1::TerminatorOperand { block, operand } => self
                .block(block, budget)?
                .map(|block| ErasedOperandV1::TerminatorOperand { block, operand }),
        })
    }

    fn edge(
        &self,
        original: ErasedEdgeV1,
        budget: &mut AssertOriginBudgetV1<'_>,
    ) -> ErasedOccurrenceResultV1<ErasedSourceOccurrenceV1<ErasedEdgeV1>> {
        self.original_edge(original, budget)?;
        Ok(self
            .block(original.source, budget)?
            .map(|source| ErasedEdgeV1 {
                source,
                successor: original.successor,
            }))
    }

    #[cfg_attr(not(test), allow(dead_code))]
    fn edge_argument(
        &self,
        original: ErasedEdgeArgumentV1,
        budget: &mut AssertOriginBudgetV1<'_>,
    ) -> ErasedOccurrenceResultV1<ErasedSourceOccurrenceV1<ErasedEdgeArgumentV1>> {
        let edge = self.original_edge(original.edge, budget)?;
        erased_occurrence_charge_v1(budget, 4)?;
        let index = erased_occurrence_index_v1(edge.bindings.clone(), original.argument)?;
        if self
            .inventory()
            .edge_arguments()
            .get(index)
            .map(|row| row.coordinate)
            != Some(original)
        {
            return Err(erased_occurrence_invalid_v1(
                "original edge argument coordinate",
            ));
        }
        Ok(self
            .edge(original.edge, budget)?
            .map(|edge| ErasedEdgeArgumentV1 {
                edge,
                argument: original.argument,
            }))
    }

    #[cfg_attr(not(test), allow(dead_code))]
    fn access(
        &self,
        original: ErasedAccessV1,
        budget: &mut AssertOriginBudgetV1<'_>,
    ) -> ErasedOccurrenceResultV1<ErasedSourceOccurrenceV1<ErasedAccessV1>> {
        let operation = self.original_operation(original.operation, budget)?;
        erased_occurrence_charge_v1(budget, 4)?;
        let index = erased_occurrence_index_v1(operation.effects.clone(), original.effect)?;
        if self
            .inventory()
            .effects()
            .get(index)
            .map(|row| row.coordinate)
            != Some(original)
        {
            return Err(erased_occurrence_invalid_v1(
                "original memory effect coordinate",
            ));
        }
        Ok(self
            .operation(original.operation, budget)?
            .map(|operation| ErasedAccessV1 {
                operation,
                effect: original.effect,
            }))
    }
}
