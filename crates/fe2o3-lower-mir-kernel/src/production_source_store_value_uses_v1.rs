// Numeric source-to-importer correspondence, not an executable value graph.

/// The source operand position consumed by one ordinary physical Store.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SemanticKirSourceStoreOperandV1 {
    /// The result of the source assignment's RHS rvalue.
    AssignmentRvalue,
    /// The value operand of an explicit non-atomic source Store.
    StoreValue,
}

/// Exact function-local definition of the value consumed by the emitted Store.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SemanticKirSourceStoreDefinitionV1 {
    /// A formal parameter, in signature order.
    FunctionArgument(u32),
    /// A block parameter, in the existing scalarized parameter order.
    BlockArgument {
        /// Function-local block identity.
        block: BlockId,
        /// Parameter ordinal in that block.
        argument: u32,
    },
    /// One result of an existing operation, including a retained Call result.
    OperationResult {
        /// Function-local block identity.
        block: BlockId,
        /// Operation ordinal in that block.
        operation: u32,
        /// Result ordinal in that operation.
        result: u32,
    },
}

/// Immutable importer correspondence for an ordinary scalar Store operand.
///
/// The source site and operand role identify the complete original RHS, not a
/// copied expression. `component` is its existing scalarization ordinal. The
/// emitted operation is an ordinary Store and `value` is its second operand.
/// This trace does not prove importer equivalence, numerical meaning, or any
/// property of a later optimized graph. Source replay compares every row.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SemanticKirSourceStoreValueUseV1 {
    correspondence_owner: SemanticFunctionIdV1,
    semantic_function: SemanticFunctionIdV1,
    semantic_block: SemanticBlockIdV1,
    statement: u32,
    operand: SemanticKirSourceStoreOperandV1,
    source_type: SemanticTypeIdV1,
    component: u32,
    kernel_ir_block: BlockId,
    operation: u32,
    value: ValueId,
    scalar: ScalarType,
    definition: SemanticKirSourceStoreDefinitionV1,
}

impl SemanticKirSourceStoreValueUseV1 {
    /// Selected source root owning this materialization instance.
    pub const fn correspondence_owner(&self) -> SemanticFunctionIdV1 {
        self.correspondence_owner
    }
    /// Actual source function containing the assignment.
    pub const fn semantic_function(&self) -> SemanticFunctionIdV1 {
        self.semantic_function
    }
    /// Exact source block and statement ordinal.
    pub const fn source_statement(&self) -> (SemanticBlockIdV1, u32) {
        (self.semantic_block, self.statement)
    }
    /// Exact original RHS operand position.
    pub const fn operand(&self) -> SemanticKirSourceStoreOperandV1 {
        self.operand
    }
    /// Source RHS type before the existing scalarization.
    pub const fn source_type(&self) -> SemanticTypeIdV1 {
        self.source_type
    }
    /// Logical component ordinal emitted by the existing lowering rule.
    pub const fn component(&self) -> u32 {
        self.component
    }
    /// Function-local emitted Store location.
    pub const fn store(&self) -> (BlockId, u32) {
        (self.kernel_ir_block, self.operation)
    }
    /// Actual function-local SSA value consumed by the Store.
    pub const fn value(&self) -> ValueId {
        self.value
    }
    /// Exact scalar type of that definition.
    pub const fn scalar(&self) -> ScalarType {
        self.scalar
    }
    /// Exact emitted definition, not a symbolic source expression.
    pub const fn definition(&self) -> SemanticKirSourceStoreDefinitionV1 {
        self.definition
    }
}

#[derive(Clone, Copy)]
struct SourceStoreScopeV1 {
    owner: SemanticFunctionIdV1,
    function: SemanticFunctionIdV1,
    block: SemanticBlockIdV1,
    kernel_ir_block: BlockId,
    statement: u32,
    operand: SemanticKirSourceStoreOperandV1,
    source_type: SemanticTypeIdV1,
}

#[derive(Clone, Copy)]
struct SourceStorePendingV1 {
    scope: SourceStoreScopeV1,
    component: u32,
    operation: u32,
    value: ValueId,
}

#[derive(Clone, Copy)]
struct SourceStoreDefinitionRowV1 {
    value: ValueId,
    scalar: Option<ScalarType>,
    definition: SemanticKirSourceStoreDefinitionV1,
}

struct SourceStoreWorkV1 {
    work: fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1,
    row_limit: usize,
    peak_rows: usize,
}

impl SourceStoreWorkV1 {
    fn charge(&mut self, amount: usize) -> Result<(), ProductionSemanticKirErrorV1> {
        charge_ordinary_helper_result_work_v1(&mut self.work, amount)
    }

    fn storage(&mut self, rows: usize) -> Result<(), ProductionSemanticKirErrorV1> {
        self.peak_rows = self.peak_rows.max(rows);
        enforce_limit(
            ProductionSemanticKirResourceV1::AnalysisStorage,
            rows,
            self.row_limit,
        )
    }
}

impl PrivateArrayChargeV1 for SourceStoreWorkV1 {
    type Error = ProductionSemanticKirErrorV1;

    fn charge_private_array_work(&mut self, amount: usize) -> Result<(), Self::Error> {
        self.charge(amount)
    }
}

fn source_store_arithmetic_v1() -> ProductionSemanticKirErrorV1 {
    ProductionSemanticKirErrorV1::ResourceLimit {
        resource: ProductionSemanticKirResourceV1::AnalysisStorage,
        actual: usize::MAX,
        limit: usize::MAX,
    }
}

// Logical rows, not bytes: every Vec's actual capacity is reconciled before
// the next fallible step. Existing elements' relocation is prepaid on growth.
fn source_store_reserve_v1<T>(
    rows: &mut Vec<T>,
    other_capacity: usize,
    work: &mut SourceStoreWorkV1,
) -> Result<(), ProductionSemanticKirErrorV1> {
    work.charge(3)?;
    if rows.len() < rows.capacity() {
        return Ok(());
    }
    let wanted = rows
        .len()
        .checked_add(1)
        .ok_or_else(source_store_arithmetic_v1)?;
    let remaining = work.row_limit.saturating_sub(other_capacity);
    let capacity = wanted.max(rows.capacity().saturating_mul(2).min(remaining));
    work.storage(
        other_capacity
            .checked_add(capacity)
            .ok_or_else(source_store_arithmetic_v1)?,
    )?;
    work.charge(rows.len().saturating_add(1))?;
    rows.try_reserve_exact(capacity - rows.len()).map_err(|_| {
        ProductionSemanticKirErrorV1::AllocationFailure {
            resource: ProductionSemanticKirResourceV1::AnalysisStorage,
        }
    })?;
    work.storage(
        other_capacity
            .checked_add(rows.capacity())
            .ok_or_else(source_store_arithmetic_v1)?,
    )
}

struct SourceStoreEmissionV1 {
    work: SourceStoreWorkV1,
    rows: Vec<SemanticKirSourceStoreValueUseV1>,
}

impl SourceStoreEmissionV1 {
    fn new(limit: usize) -> Self {
        Self {
            work: SourceStoreWorkV1 {
                work: fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(limit),
                row_limit: limit,
                peak_rows: 0,
            },
            rows: Vec::new(),
        }
    }
}

struct SourceStoreFunctionCaptureV1<'a> {
    emission: Option<&'a mut SourceStoreEmissionV1>,
    pending: Vec<SourceStorePendingV1>,
    active: Option<SourceStoreScopeV1>,
    block: Option<BlockId>,
    component: u32,
}

impl<'a> SourceStoreFunctionCaptureV1<'a> {
    fn new(emission: Option<&'a mut SourceStoreEmissionV1>) -> Self {
        Self {
            emission,
            pending: Vec::new(),
            active: None,
            block: None,
            component: 0,
        }
    }

    fn record(
        &mut self,
        operation: usize,
        value: ValueId,
        predicate: Option<ValueId>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        let (Some(scope), Some(emission)) = (self.active, self.emission.as_deref_mut()) else {
            return Ok(());
        };
        emission.work.charge(4)?;
        if predicate.is_some() {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
        let operation = u32::try_from(operation)
            .map_err(|_| ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        source_store_reserve_v1(
            &mut self.pending,
            emission.rows.capacity(),
            &mut emission.work,
        )?;
        self.pending.push(SourceStorePendingV1 {
            scope,
            component: self.component,
            operation,
            value,
        });
        Ok(())
    }

    fn finish(
        &mut self,
        arguments: &[ValueId],
        argument_types: &[Type],
        blocks: &[BasicBlock],
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        if self.pending.is_empty() {
            return Ok(());
        }
        let emission = self
            .emission
            .as_deref_mut()
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        emission.work.charge(1)?;
        if self.active.is_some() || arguments.len() != argument_types.len() {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
        let mut definitions = Vec::new();
        let other = emission
            .rows
            .capacity()
            .checked_add(self.pending.capacity())
            .ok_or_else(source_store_arithmetic_v1)?;
        let mut push = |value, ty: &Type, definition| {
            source_store_reserve_v1(&mut definitions, other, &mut emission.work)?;
            definitions.push(SourceStoreDefinitionRowV1 {
                value,
                scalar: ty.as_scalar(),
                definition,
            });
            Ok::<_, ProductionSemanticKirErrorV1>(())
        };
        for (argument, (value, ty)) in arguments.iter().zip(argument_types).enumerate() {
            push(
                *value,
                ty,
                SemanticKirSourceStoreDefinitionV1::FunctionArgument(
                    u32::try_from(argument).map_err(|_| source_store_arithmetic_v1())?,
                ),
            )?;
        }
        // The closure's last use precedes the separately charged block/op visits.
        for block in blocks {
            emission.work.charge(1)?;
            for (argument, value) in block.parameters.iter().enumerate() {
                source_store_reserve_v1(&mut definitions, other, &mut emission.work)?;
                definitions.push(SourceStoreDefinitionRowV1 {
                    value: value.id,
                    scalar: value.ty.as_scalar(),
                    definition: SemanticKirSourceStoreDefinitionV1::BlockArgument {
                        block: block.id,
                        argument: u32::try_from(argument)
                            .map_err(|_| source_store_arithmetic_v1())?,
                    },
                });
            }
            for (operation, op) in block.operations.iter().enumerate() {
                emission.work.charge(1)?;
                for (result, value) in op.results.iter().enumerate() {
                    source_store_reserve_v1(&mut definitions, other, &mut emission.work)?;
                    definitions.push(SourceStoreDefinitionRowV1 {
                        value: value.id,
                        scalar: value.ty.as_scalar(),
                        definition: SemanticKirSourceStoreDefinitionV1::OperationResult {
                            block: block.id,
                            operation: u32::try_from(operation)
                                .map_err(|_| source_store_arithmetic_v1())?,
                            result: u32::try_from(result)
                                .map_err(|_| source_store_arithmetic_v1())?,
                        },
                    });
                }
            }
        }
        private_array_heapsort_v1(
            &mut definitions,
            |row| [row.value.0 as usize],
            &mut emission.work,
            source_store_arithmetic_v1,
        )?;
        for pair in definitions.windows(2) {
            emission.work.charge(1)?;
            if pair[0].value == pair[1].value {
                return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
            }
        }
        let floor = emission.rows.len();
        let result = (|| {
            for pending in &self.pending {
                emission.work.charge(1)?;
                let mut lo = 0;
                let mut hi = definitions.len();
                let found = loop {
                    emission.work.charge(3)?;
                    if lo == hi {
                        return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
                    }
                    let mid = lo + (hi - lo) / 2;
                    let row = definitions[mid];
                    match row.value.cmp(&pending.value) {
                        std::cmp::Ordering::Less => lo = mid + 1,
                        std::cmp::Ordering::Greater => hi = mid,
                        std::cmp::Ordering::Equal => break row,
                    }
                };
                let scalar = found
                    .scalar
                    .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
                let other = self
                    .pending
                    .capacity()
                    .checked_add(definitions.capacity())
                    .ok_or_else(source_store_arithmetic_v1)?;
                source_store_reserve_v1(&mut emission.rows, other, &mut emission.work)?;
                let scope = pending.scope;
                emission.rows.push(SemanticKirSourceStoreValueUseV1 {
                    correspondence_owner: scope.owner,
                    semantic_function: scope.function,
                    semantic_block: scope.block,
                    statement: scope.statement,
                    operand: scope.operand,
                    source_type: scope.source_type,
                    component: pending.component,
                    kernel_ir_block: scope.kernel_ir_block,
                    operation: pending.operation,
                    value: pending.value,
                    scalar,
                    definition: found.definition,
                });
            }
            Ok(())
        })();
        if result.is_err() {
            emission.rows.truncate(floor);
        }
        result
    }
}

impl SemanticFunctionLoweringV1<'_> {
    #[allow(clippy::too_many_arguments)]
    fn assign_source_place_v1(
        &mut self,
        block: SemanticBlockIdV1,
        statement: Option<u32>,
        destination: &SemanticPlaceV1,
        value: SemanticValueBindingV1,
        volatility: SemanticVolatilityV1,
        operand: SemanticKirSourceStoreOperandV1,
        source_type: SemanticTypeIdV1,
        operations: &mut Vec<Operation>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        if self.source_stores.active.is_some() {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
        // No source scope for SSA bindings or internal enum payload transport.
        // RHS evaluation already completed before this method is entered.
        let ordinary = matches!(
            &value,
            SemanticValueBindingV1::Value {
                ty: Type::Scalar(_),
                ..
            } | SemanticValueBindingV1::IndexWitness { .. }
                | SemanticValueBindingV1::WaveLane { .. }
        ) || (matches!(&value, SemanticValueBindingV1::Aggregate(_))
            && self
                .retained_array_slot_v1(destination.local())
                .is_some_and(|slot| matches!(slot.kernel_type, Type::Scalar(_))));
        let physical = !destination.projections().is_empty()
            || self
                .retained_local_slots
                .contains_key(&destination.local().index());
        if ordinary && physical && self.source_stores.emission.is_some() {
            self.source_stores.active = Some(SourceStoreScopeV1 {
                owner: self.correspondence_owner,
                function: self.semantic_function,
                block,
                kernel_ir_block: self
                    .source_stores
                    .block
                    .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?,
                statement: statement.ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?,
                operand,
                source_type,
            });
        }
        self.source_stores.component = 0;
        let floor = self.source_stores.pending.len();
        let result =
            self.assign_place(block, statement, destination, value, volatility, operations);
        self.source_stores.active = None;
        self.source_stores.component = 0;
        if result.is_err() {
            self.source_stores.pending.truncate(floor);
        }
        result
    }
}
