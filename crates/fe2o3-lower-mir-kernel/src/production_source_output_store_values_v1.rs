// Source-rooted value-use facts on the existing canonical graph, not a value IR.

#[derive(Debug)]
struct SourceOutputStoreValueRowV1 {
    key: [u32; 5],
    source: usize,
    original: fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1,
    definition: fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1,
    output: Option<fe2o3_kernel_analysis::CanonicalKirOutputUseV1>,
    executable: bool,
}

#[derive(Debug)]
struct SourceOutputStoreValueOutputRowV1 {
    owner: u32,
    function: u32,
    used: fe2o3_kernel_ir::CanonicalKirUseCoordinateV1,
    row: usize,
}

#[derive(Debug, Default)]
struct SourceOutputStoreValueIndexV1 {
    rows: Vec<SourceOutputStoreValueRowV1>,
    by_output: Vec<SourceOutputStoreValueOutputRowV1>,
}

/// A borrowed source-rooted Store operand relation on this view's exact O.
///
/// Coordinates are inert locators. Source replay establishes importer
/// correspondence; the independently checked transition transports its use.
/// Neither is an independent mathematical-reference or termination proof.
/// This type has no constructor, mutation, serialization, or authority transfer.
pub struct ProductionSourceOutputStoreValueV1<'view> {
    source: &'view SemanticKirSourceStoreValueUseV1,
    row: &'view SourceOutputStoreValueRowV1,
    ordinal: usize,
}

impl ProductionSourceOutputStoreValueV1<'_> {
    /// Exact replayed source RHS/component and its actual N Store definition.
    pub const fn source(&self) -> &SemanticKirSourceStoreValueUseV1 {
        self.source
    }

    /// Original B use, after independently checked N/B coordinate preservation.
    pub const fn original_use(&self) -> fe2o3_kernel_ir::CanonicalKirUseCoordinateV1 {
        fe2o3_kernel_ir::CanonicalKirUseCoordinateV1::OperationOperand {
            operation: self.row.original,
            operand: 1,
        }
    }

    /// Exact original definition, including a block parameter when appropriate.
    pub const fn original_definition(&self) -> fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1 {
        self.row.definition
    }

    /// Actual O Store operand1 and definition; None means checked whole-block omission.
    pub const fn output_use(&self) -> Option<fe2o3_kernel_analysis::CanonicalKirOutputUseV1> {
        self.row.output
    }

    /// Physical retention alone does not establish executable reachability.
    pub const fn executable(&self) -> bool {
        self.row.executable
    }
}

fn source_output_store_definition_v1(
    inventory: &fe2o3_kernel_analysis::CanonicalKirInventoryV1<'_>,
    function: fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1,
    definition: SemanticKirSourceStoreDefinitionV1,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Result<fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1, ProductionSourceOutputErrorV1> {
    use ProductionSourceOutputErrorV1 as Error;
    use SemanticKirSourceStoreDefinitionV1 as Source;
    use fe2o3_kernel_ir::{
        CanonicalKirDefinitionCoordinateV1 as Definition, CanonicalKirOperationCoordinateV1,
    };
    budget.charge_work(2).map_err(Error::Resource)?;
    match definition {
        Source::FunctionArgument(argument) => {
            Ok(Definition::FunctionArgument { function, argument })
        }
        Source::BlockArgument { block, argument } => {
            let block = inventory
                .block_for_id(function, block, budget)
                .map_err(Error::Inventory)?
                .ok_or(Error::Invalid("source Store definition block absent"))?;
            Ok(Definition::BlockArgument {
                block: block.coordinate,
                argument,
            })
        }
        Source::OperationResult {
            block,
            operation,
            result,
        } => {
            let block = inventory
                .block_for_id(function, block, budget)
                .map_err(Error::Inventory)?
                .ok_or(Error::Invalid("source Store definition block absent"))?;
            Ok(Definition::Result {
                operation: CanonicalKirOperationCoordinateV1 {
                    block: block.coordinate,
                    operation,
                },
                result,
            })
        }
    }
}

fn source_output_store_values_v1(
    source: &ProductionPreRankedKirOwnerV1,
    coordinates: &fe2o3_kernel_analysis::CheckedCanonicalKirCoordinatePreservationV1<'_, '_>,
    transition: &fe2o3_kernel_analysis::CheckedCanonicalKirTransitionV1<'_, '_, '_, '_>,
    control: &fe2o3_kernel_analysis::CheckedCanonicalKirControlIndexV1<'_, '_, '_>,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Result<(SourceOutputStoreValueIndexV1, usize, usize), ProductionSourceOutputErrorV1> {
    use ProductionSourceOutputErrorV1 as Error;
    use fe2o3_kernel_ir::CanonicalKirUseCoordinateV1::OperationOperand;
    budget.charge_work(4).map_err(Error::Resource)?;
    let input = transition.input();
    if !std::ptr::eq(source.executable(), coordinates.input())
        || !std::ptr::eq(input.owner(), coordinates.output())
        || !std::ptr::eq(control.input().owner(), input.owner())
        || !std::ptr::eq(control.output().owner(), transition.output().owner())
    {
        return Err(Error::InputCustody);
    }
    let header = std::mem::size_of::<SourceOutputStoreValueIndexV1>();
    budget.charge_work(1).map_err(Error::Resource)?;
    budget.reserve_storage(header).map_err(Error::Resource)?;
    let mut index = SourceOutputStoreValueIndexV1::default();
    for (source_ordinal, captured) in source.source_store_value_uses_v1().iter().enumerate() {
        budget.charge_work(5).map_err(Error::Resource)?;
        let owner = captured.correspondence_owner();
        let function = captured.semantic_function();
        let canonical = source_output_ordinary_function_alias_v1(source, owner, function, budget)?;
        let (source_block, statement) = captured.source_statement();
        let (block_id, operation_ordinal) = captured.store();
        let block = input
            .block_for_id(canonical, block_id, budget)
            .map_err(Error::Inventory)?
            .ok_or(Error::Invalid("source Store original block absent"))?;
        budget.charge_work(5).map_err(Error::Resource)?;
        let offset = operation_ordinal as usize;
        if offset >= block.operations.len() {
            return Err(Error::Invalid("source Store original operation absent"));
        }
        let ordinal = block
            .operations
            .start
            .checked_add(offset)
            .ok_or(Error::Resource(AssertOriginResourceV1::Arithmetic))?;
        let original = input
            .operations()
            .get(ordinal)
            .ok_or(Error::Invalid("source Store original operation absent"))?;
        let OperationKind::Store { value, .. } = &original.operation.kind else {
            return Err(Error::Invalid(
                "source Store row names another operation kind",
            ));
        };
        if *value != captured.value() || !original.operation.results.is_empty() {
            return Err(Error::Invalid("source Store original value changed"));
        }
        let definition =
            source_output_store_definition_v1(input, canonical, captured.definition(), budget)?;
        let actual = input
            .definition_for_value(canonical, *value, budget)
            .map_err(Error::Inventory)?
            .ok_or(Error::Invalid("source Store original definition absent"))?;
        budget.charge_work(4).map_err(Error::Resource)?;
        if actual.coordinate != definition || actual.ty.as_scalar() != Some(captured.scalar()) {
            return Err(Error::Invalid(
                "source Store original definition or scalar changed",
            ));
        }
        let (n_value, n_type) =
            source_output_definition_v1(source.executable(), definition, budget)?;
        budget.charge_work(2).map_err(Error::Resource)?;
        if n_value != captured.value() || n_type.as_scalar() != Some(captured.scalar()) {
            return Err(Error::Invalid(
                "source Store N definition or scalar changed",
            ));
        }
        // This is the same exact pointer/value-use transport as the Global
        // occurrence bridge, factored without changing its legacy charge order.
        let (output, executable) =
            match source_output_memory_operands_v1(control, original, Some(*value), budget)? {
                SourceOutputMemoryOperandsV1::OmittedUnreachable => (None, false),
                SourceOutputMemoryOperandsV1::Retained {
                    operation,
                    value,
                    executable,
                    ..
                } => {
                    budget.charge_work(3).map_err(Error::Resource)?;
                    let used = value.ok_or(Error::Invalid("source Store output value absent"))?;
                    if used.coordinate
                        != (OperationOperand {
                            operation,
                            operand: 1,
                        })
                    {
                        return Err(Error::Invalid("source Store output operand changed"));
                    }
                    let (_, output_type) = source_output_definition_v1(
                        control.output().owner(),
                        used.definition,
                        budget,
                    )?;
                    budget.charge_work(1).map_err(Error::Resource)?;
                    if output_type.as_scalar() != Some(captured.scalar()) {
                        return Err(Error::Invalid("source Store output scalar changed"));
                    }
                    (Some(used), executable)
                }
            };
        assert_origin_push_v1(
            &mut index.rows,
            SourceOutputStoreValueRowV1 {
                key: [
                    owner.index(),
                    function.index(),
                    source_block.index(),
                    statement,
                    captured.component(),
                ],
                source: source_ordinal,
                original: original.coordinate,
                definition,
                output,
                executable,
            },
            budget,
        )
        .map_err(Error::SourceOrigin)?;
    }
    let payload = source_output_store_value_finish_v1(&mut index, budget)?;
    Ok((index, payload, header))
}

fn source_output_store_value_finish_v1(
    index: &mut SourceOutputStoreValueIndexV1,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Result<usize, ProductionSourceOutputErrorV1> {
    use ProductionSourceOutputErrorV1 as Error;
    assert_origin_sort_v1(&mut index.rows, budget, |a, b, budget| {
        budget.charge_work(5)?;
        Ok(a.key.cmp(&b.key))
    })
    .map_err(Error::SourceOrigin)?;
    for (row, entry) in index.rows.iter().enumerate() {
        budget.charge_work(3).map_err(Error::Resource)?;
        if row > 0 {
            budget.charge_work(5).map_err(Error::Resource)?;
            if index.rows[row - 1].key == entry.key {
                return Err(Error::Invalid(
                    "source Store component appears more than once",
                ));
            }
        }
        if let Some(used) = entry.output {
            assert_origin_push_v1(
                &mut index.by_output,
                SourceOutputStoreValueOutputRowV1 {
                    owner: entry.key[0],
                    function: entry.key[1],
                    used: used.coordinate,
                    row,
                },
                budget,
            )
            .map_err(Error::SourceOrigin)?;
        }
    }
    assert_origin_sort_v1(&mut index.by_output, budget, |a, b, budget| {
        budget.charge_work(6)?;
        Ok((a.owner, a.function, a.used).cmp(&(b.owner, b.function, b.used)))
    })
    .map_err(Error::SourceOrigin)?;
    for pair in index.by_output.windows(2) {
        budget.charge_work(6).map_err(Error::Resource)?;
        if (pair[0].owner, pair[0].function, pair[0].used)
            == (pair[1].owner, pair[1].function, pair[1].used)
        {
            return Err(Error::Invalid(
                "source Store output operand appears more than once",
            ));
        }
    }
    budget.charge_work(5).map_err(Error::Resource)?;
    let payload = index
        .rows
        .capacity()
        .checked_mul(std::mem::size_of::<SourceOutputStoreValueRowV1>())
        .and_then(|n| {
            index
                .by_output
                .capacity()
                .checked_mul(std::mem::size_of::<SourceOutputStoreValueOutputRowV1>())
                .and_then(|m| n.checked_add(m))
        })
        .ok_or(Error::Resource(AssertOriginResourceV1::Arithmetic))?;
    Ok(payload)
}

#[cfg(test)]
include!("production_semantic_kir_v1/tests/production_source_output_store_values_v1_tests.rs");

impl ProductionSourceOutputOccurrencesV1<'_, '_> {
    fn source_output_store_value_floor_v1(
        &self,
        budget: &mut AssertOriginBudgetV1<'_>,
    ) -> Result<(), ProductionSourceOutputErrorV1> {
        use ProductionSourceOutputErrorV1 as Error;
        budget.charge_work(4).map_err(Error::Resource)?;
        let live = self
            .source
            .executable_storage()
            .retained_storage()
            .checked_add(self.source.assert_origin_storage().payload_storage())
            .and_then(|n| n.checked_add(self.checked_output.storage().retained_storage()))
            .and_then(|n| n.checked_add(self.storage.retained_storage()))
            .ok_or(Error::Resource(AssertOriginResourceV1::Arithmetic))?;
        if budget.storage() < live {
            return Err(Error::Resource(AssertOriginResourceV1::Accounting));
        }
        Ok(())
    }

    /// Borrows a source-qualified value-use relation, not numerical authority.
    /// None is absence of a captured ordinary scalar Store, never its proof.
    pub fn source_store_value_v1(
        &self,
        owner: SemanticFunctionIdV1,
        function: SemanticFunctionIdV1,
        block: SemanticBlockIdV1,
        statement: u32,
        component: u32,
        budget: &mut AssertOriginBudgetV1<'_>,
    ) -> Result<Option<ProductionSourceOutputStoreValueV1<'_>>, ProductionSourceOutputErrorV1> {
        use ProductionSourceOutputErrorV1 as Error;
        self.source_output_store_value_floor_v1(budget)?;
        budget.charge_work(1).map_err(Error::Resource)?;
        let key = [
            owner.index(),
            function.index(),
            block.index(),
            statement,
            component,
        ];
        let row = assert_origin_find_v1(&self.store_values.rows, budget, |row, budget| {
            budget.charge_work(5)?;
            Ok(row.key.cmp(&key))
        })
        .map_err(Error::SourceOrigin)?;
        match row {
            Some(row) => self.source_output_store_value_at_v1(row, budget).map(Some),
            None => Ok(None),
        }
    }

    fn source_output_store_value_at_v1(
        &self,
        ordinal: usize,
        budget: &mut AssertOriginBudgetV1<'_>,
    ) -> Result<ProductionSourceOutputStoreValueV1<'_>, ProductionSourceOutputErrorV1> {
        use ProductionSourceOutputErrorV1 as Error;
        budget.charge_work(2).map_err(Error::Resource)?;
        let row = self
            .store_values
            .rows
            .get(ordinal)
            .ok_or(Error::Invalid("source Store index row absent"))?;
        let source = self
            .source
            .source_store_value_uses_v1()
            .get(row.source)
            .ok_or(Error::Invalid("source Store correspondence row absent"))?;
        Ok(ProductionSourceOutputStoreValueV1 {
            source,
            row,
            ordinal,
        })
    }

    fn source_output_store_value_for_output_v1(
        &self,
        owner: SemanticFunctionIdV1,
        function: SemanticFunctionIdV1,
        used: fe2o3_kernel_ir::CanonicalKirUseCoordinateV1,
        budget: &mut AssertOriginBudgetV1<'_>,
    ) -> Result<Option<ProductionSourceOutputStoreValueV1<'_>>, ProductionSourceOutputErrorV1> {
        use ProductionSourceOutputErrorV1 as Error;
        self.source_output_store_value_floor_v1(budget)?;
        let key = (owner.index(), function.index(), used);
        let found = assert_origin_find_v1(&self.store_values.by_output, budget, |row, budget| {
            budget.charge_work(6)?;
            Ok((row.owner, row.function, row.used).cmp(&key))
        })
        .map_err(Error::SourceOrigin)?;
        match found {
            Some(found) => {
                budget.charge_work(1).map_err(Error::Resource)?;
                self.source_output_store_value_at_v1(self.store_values.by_output[found].row, budget)
                    .map(Some)
            }
            None => Ok(None),
        }
    }
}
