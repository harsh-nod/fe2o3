use fe2o3_kernel_ir::ExecutionOperationV15 as TileExecutionV29;
type TileBuildResultV29<T> = Result<T, ScopedTileFailureKindV29>;

#[derive(Clone, Copy)]
struct TileLoadPlanV29 {
    point: TileScalarPointV29,
    input: ValueId,
    base: ValueId,
    lanes: u16,
    elements: u16,
    order: ScopedTileOrderV29,
    first_value: u32,
    first_block: u32,
    first_block_ordinal: usize,
}
impl TileLoadPlanV29 {
    // The planner bounds each complete range before graph allocation.
    fn value(self, offset: u32) -> ValueId {
        ValueId(self.first_value + offset)
    }
    fn block(self, offset: u32) -> BlockId {
        BlockId(self.first_block + offset)
    }
    fn ordinal(self, offset: usize) -> usize {
        self.first_block_ordinal + offset
    }
}
fn tile_build_add_v29(a: usize, b: usize) -> TileBuildResultV29<usize> {
    Ok(argument_sum_v1(&[a, b])?)
}
fn tile_build_mul_v29(a: usize, b: usize) -> TileBuildResultV29<usize> {
    Ok(argument_product_v1(a, b)?)
}
fn tile_build_limit_v29(value: usize, limit: usize) -> TileBuildResultV29<()> {
    if value > limit {
        Err(ScopedTileFailureKindV29::Canonical)
    } else {
        Ok(())
    }
}
fn tile_build_u32_v29(value: usize) -> TileBuildResultV29<u32> {
    u32::try_from(value).map_err(|_| ArgumentResourceV1::Arithmetic.into())
}
fn tile_definition_point_v29(
    index: &AssertGraphIndexV1<'_>,
    function: usize,
    value: ValueId,
    budget: &mut ArgumentBudgetV1<'_>,
) -> TileBuildResultV29<TileScalarPointV29> {
    let function = fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1(tile_build_u32_v29(function)?);
    let definition = index
        .definition(function, value, budget)
        .map_err(ProductionSemanticKirErrorV1::AssertOrigin)?;
    let AssertDefinitionCoordinateV1::Result {
        operation,
        result: 0,
    } = definition.coordinate
    else {
        return Err(ScopedTileFailureKindV29::ReplayMismatch);
    };
    Ok(TileScalarPointV29 {
        function: operation.block.function.0 as usize,
        block: operation.block.block as usize,
        operation: operation.operation as usize,
    })
}
fn tile_original_operation_v29(
    module: &Module,
    point: TileScalarPointV29,
) -> TileBuildResultV29<&Operation> {
    module
        .functions
        .get(point.function)
        .and_then(|f| f.body.as_ref())
        .and_then(|b| b.blocks.get(point.block))
        .and_then(|b| b.operations.get(point.operation))
        .ok_or(ScopedTileFailureKindV29::ReplayMismatch)
}
fn tile_parts_plan_v29(
    module: &Module,
    index: &AssertGraphIndexV1<'_>,
    plans: &[TileLoadPlanV29],
    function: usize,
    fragment: ValueId,
    lanes: u16,
    elements: u16,
    budget: &mut ArgumentBudgetV1<'_>,
) -> TileBuildResultV29<TileLoadPlanV29> {
    let fragment_point = tile_definition_point_v29(index, function, fragment, budget)?;
    budget.charge_work(2)?;
    let OperationKind::Execution(TileExecutionV29::TileIntoFragmentU32 {
        tile,
        lanes: actual_lanes,
        elements: actual_elements,
    }) = &tile_original_operation_v29(module, fragment_point)?.kind
    else {
        return Err(ScopedTileFailureKindV29::ReplayMismatch);
    };
    if (*actual_lanes, *actual_elements) != (lanes, elements) {
        return Err(ScopedTileFailureKindV29::Geometry);
    }
    let load = tile_definition_point_v29(index, function, *tile, budget)?;
    budget.charge_work(tile_build_add_v29(plans.len(), 1)?)?;
    let plan = plans
        .iter()
        .find(|p| p.point == load)
        .ok_or(ScopedTileFailureKindV29::Census)?;
    if (plan.lanes, plan.elements) != (lanes, elements) {
        return Err(ScopedTileFailureKindV29::Geometry);
    }
    Ok(*plan)
}
fn tile_plan_loads_v29(
    original: &Module,
    roots: &[ScopedModuleRootV29],
    selections: &[ScopedTileSelectionV29],
    budget: &mut ArgumentBudgetV1<'_>,
) -> TileBuildResultV29<Vec<TileLoadPlanV29>> {
    let mut plans = Vec::new();
    for (function, declaration) in original.functions.iter().enumerate() {
        budget.charge_work(1)?;
        let Some(body) = &declaration.body else {
            continue;
        };
        let (mut maximum_value, mut maximum_block) = (0u32, 0u32);
        for value in &body.parameters {
            budget.charge_work(1)?;
            maximum_value = maximum_value.max(value.0);
        }
        for block in &body.blocks {
            budget.charge_work(1)?;
            maximum_block = maximum_block.max(block.id.0);
            for value in &block.parameters {
                budget.charge_work(1)?;
                maximum_value = maximum_value.max(value.id.0);
            }
            for operation in &block.operations {
                budget.charge_work(1)?;
                for value in &operation.results {
                    budget.charge_work(1)?;
                    maximum_value = maximum_value.max(value.id.0);
                }
            }
        }
        let (mut fresh_values, mut fresh_blocks) = (0usize, 0usize);
        for (block_ordinal, block) in body.blocks.iter().enumerate() {
            let mut continuation_operations = 0usize;
            for (operation_ordinal, operation) in block.operations.iter().enumerate() {
                budget.charge_work(1)?;
                match &operation.kind {
                    OperationKind::Execution(TileExecutionV29::MaskedTileLoadU32 {
                        workgroup,
                        input: slice,
                        base,
                        lanes,
                        elements,
                    }) => {
                        let point = TileScalarPointV29 {
                            function,
                            block: block_ordinal,
                            operation: operation_ordinal,
                        };
                        let mut selected = None;
                        for selection in selections {
                            budget.charge_work(2)?;
                            let root = roots
                                .get(selection.root)
                                .ok_or(ScopedTileFailureKindV29::Census)?;
                            if root.function_ordinal != function
                                || selection.witness.after.block != block.id
                                || selection.witness.after.first as usize != operation_ordinal
                            {
                                continue;
                            }
                            let DeferredTileInputV29::Load {
                                workgroup: w,
                                input: i,
                                base: b,
                            } = selection.tile.input
                            else {
                                return Err(ScopedTileFailureKindV29::Census);
                            };
                            if selected.is_some()
                                || selection.witness.after.count != 1
                                || (w.value, i, b) != (*workgroup, *slice, *base)
                                || (selection.tile.lanes, selection.tile.elements)
                                    != (*lanes, *elements)
                                || !matches!(operation.results.as_slice(), [v]
                                    if v.id == selection.tile.first_result)
                            {
                                return Err(ScopedTileFailureKindV29::Census);
                            }
                            selected = Some(selection.order);
                        }
                        let order = selected.ok_or(ScopedTileFailureKindV29::Census)?;
                        if *lanes == 0
                            || *elements == 0
                            || *lanes > fe2o3_kernel_ir::MAX_EXECUTION_LANES_V15
                            || *elements > fe2o3_kernel_ir::MAX_EXECUTION_ELEMENTS_V15
                        {
                            return Err(ScopedTileFailureKindV29::Geometry);
                        }
                        tile_build_limit_v29(
                            tile_build_add_v29(continuation_operations, 20)?,
                            MAX_BLOCK_OPERATIONS_V1,
                        )?;
                        let next_values = tile_build_add_v29(
                            fresh_values,
                            tile_build_add_v29(8, tile_build_mul_v29(20, usize::from(*elements))?)?,
                        )?;
                        let next_blocks = tile_build_add_v29(
                            fresh_blocks,
                            tile_build_mul_v29(2, usize::from(*elements))?,
                        )?;
                        maximum_value
                            .checked_add(tile_build_u32_v29(next_values)?)
                            .ok_or(ArgumentResourceV1::Arithmetic)?;
                        maximum_block
                            .checked_add(tile_build_u32_v29(next_blocks)?)
                            .ok_or(ArgumentResourceV1::Arithmetic)?;
                        tile_build_limit_v29(
                            tile_build_add_v29(body.blocks.len(), next_blocks)?,
                            fe2o3_kernel_ir::MAX_BLOCKS_V1,
                        )?;
                        let first_value = maximum_value
                            .checked_add(tile_build_u32_v29(fresh_values)?)
                            .and_then(|n| n.checked_add(1))
                            .ok_or(ArgumentResourceV1::Arithmetic)?;
                        let first_block = maximum_block
                            .checked_add(tile_build_u32_v29(fresh_blocks)?)
                            .and_then(|n| n.checked_add(1))
                            .ok_or(ArgumentResourceV1::Arithmetic)?;
                        emission_push_v1(
                            &mut plans,
                            TileLoadPlanV29 {
                                point,
                                input: *slice,
                                base: *base,
                                lanes: *lanes,
                                elements: *elements,
                                order,
                                first_value,
                                first_block,
                                first_block_ordinal: tile_build_add_v29(
                                    body.blocks.len(),
                                    fresh_blocks,
                                )?,
                            },
                            budget,
                        )?;
                        fresh_values = next_values;
                        fresh_blocks = next_blocks;
                        continuation_operations = 0;
                    }
                    OperationKind::Execution(TileExecutionV29::FragmentIntoPartsU32 {
                        elements,
                        ..
                    }) => {
                        let count = tile_build_mul_v29(2, usize::from(*elements))?;
                        if operation.results.len() != count {
                            return Err(ScopedTileFailureKindV29::Census);
                        }
                        continuation_operations =
                            tile_build_add_v29(continuation_operations, count)?;
                    }
                    OperationKind::Execution(
                        TileExecutionV29::ContextIssue
                        | TileExecutionV29::WorkgroupDerive { .. }
                        | TileExecutionV29::TileIntoFragmentU32 { .. }
                        | TileExecutionV29::ScopeEnd { .. },
                    ) => {}
                    _ => continuation_operations = tile_build_add_v29(continuation_operations, 1)?,
                }
                tile_build_limit_v29(continuation_operations, MAX_BLOCK_OPERATIONS_V1)?;
            }
        }
    }
    budget.charge_work(1)?;
    if plans.len() != selections.len() {
        return Err(ScopedTileFailureKindV29::Census);
    }
    Ok(plans)
}

struct TileEmitterV29<'a, 'work> {
    function: usize,
    body: &'a mut FunctionBody,
    relations: &'a mut TileScalarRelationsV29,
    budget: &'a mut ArgumentBudgetV1<'work>,
}
impl TileEmitterV29<'_, '_> {
    fn piece(
        &mut self,
        piece: TileScalarPieceV29,
        stage: TileScalarStageV29,
        component: Option<u32>,
    ) -> TileBuildResultV29<()> {
        emission_push_v1(
            &mut self.relations.pieces,
            TileScalarTaggedPieceV29 {
                piece,
                component,
                stage,
            },
            self.budget,
        )?;
        Ok(())
    }
    fn origin(&mut self, source: TileScalarSourceV29, first: usize) -> TileBuildResultV29<()> {
        let count = self
            .relations
            .pieces
            .len()
            .checked_sub(first)
            .ok_or(ArgumentResourceV1::Accounting)?;
        emission_push_v1(
            &mut self.relations.origins,
            TileScalarOriginV29 {
                source,
                first,
                count,
            },
            self.budget,
        )?;
        Ok(())
    }
    fn single(
        &mut self,
        source: TileScalarSourceV29,
        piece: TileScalarPieceV29,
    ) -> TileBuildResultV29<()> {
        let first = self.relations.pieces.len();
        self.piece(piece, TileScalarStageV29::Preserved, None)?;
        self.origin(source, first)
    }
    fn push_operation(
        &mut self,
        block: usize,
        operation: Operation,
        stage: TileScalarStageV29,
        component: Option<u32>,
        own_results: bool,
    ) -> TileBuildResultV29<TileScalarPointV29> {
        let result_count = operation.results.len();
        let point = TileScalarPointV29 {
            function: self.function,
            block,
            operation: self.body.blocks[block].operations.len(),
        };
        tile_build_limit_v29(
            tile_build_add_v29(point.operation, 1)?,
            MAX_BLOCK_OPERATIONS_V1,
        )?;
        emission_push_shared_v1(
            &mut self.body.blocks[block].operations,
            operation,
            self.budget,
        )?;
        self.piece(TileScalarPieceV29::Operation(point), stage, component)?;
        if own_results {
            for result in 0..result_count {
                self.piece(
                    TileScalarPieceV29::Result {
                        operation: point,
                        result,
                    },
                    stage,
                    component,
                )?;
            }
        }
        Ok(point)
    }
    fn emit<const N: usize>(
        &mut self,
        block: usize,
        kind: OperationKind,
        definitions: [ValueDef; N],
        stage: TileScalarStageV29,
        component: Option<u32>,
        own_results: bool,
    ) -> TileBuildResultV29<TileScalarPointV29> {
        let mut results = emission_vec_v1(N, self.budget)?;
        self.budget.charge_work(N)?;
        results.extend(definitions);
        self.push_operation(
            block,
            Operation::new(results, kind),
            stage,
            component,
            own_results,
        )
    }
    fn arguments<const N: usize>(
        &mut self,
        values: [ValueId; N],
    ) -> TileBuildResultV29<Vec<ValueId>> {
        let mut result = emission_vec_v1(N, self.budget)?;
        self.budget.charge_work(N)?;
        result.extend(values);
        Ok(result)
    }
    fn terminator(
        &mut self,
        block: usize,
        terminator: Terminator,
        stage: TileScalarStageV29,
        component: Option<u32>,
        own_edges: bool,
    ) -> TileBuildResultV29<usize> {
        if self.body.blocks[block].terminator.is_some() {
            return Err(ScopedTileFailureKindV29::ReplayMismatch);
        }
        let mut edges = 0usize;
        terminator.try_visit_edges_v1(|_, _| -> TileBuildResultV29<()> {
            self.budget.charge_work(1)?;
            edges = tile_build_add_v29(edges, 1)?;
            Ok(())
        })?;
        self.body.blocks[block].terminator = Some(terminator);
        self.piece(
            TileScalarPieceV29::Terminator {
                function: self.function,
                block,
            },
            stage,
            component,
        )?;
        if own_edges {
            for edge in 0..edges {
                self.piece(
                    TileScalarPieceV29::Edge {
                        function: self.function,
                        block,
                        edge,
                    },
                    stage,
                    component,
                )?;
            }
        }
        Ok(edges)
    }
    fn pointer_type(&mut self) -> TileBuildResultV29<Type> {
        self.budget.charge_work(1)?;
        self.budget.reserve_storage(size_of::<Type>())?;
        Ok(Type::pointer(
            Type::Scalar(ScalarType::U32),
            AddressSpace::Global,
            AccessMode::ReadOnly,
        ))
    }
    fn load(&mut self, plan: TileLoadPlanV29, mut current: usize) -> TileBuildResultV29<usize> {
        use TileScalarStageV29 as Stage;
        let preludes = [
            (
                OperationKind::Intrinsic(IntrinsicOperation::new(
                    IntrinsicKind::InvocationIndex {
                        kind: IndexKind::Local,
                        axis: Axis::X,
                    },
                    Type::INDEX,
                )),
                Type::INDEX,
            ),
            (
                OperationKind::SliceLength { slice: plan.input },
                Type::INDEX,
            ),
            (
                OperationKind::Constant(Constant::Index(u64::from(plan.lanes))),
                Type::INDEX,
            ),
            (
                OperationKind::Constant(Constant::Index(u64::from(plan.elements))),
                Type::INDEX,
            ),
            (
                OperationKind::Constant(Constant::U32(0)),
                Type::Scalar(ScalarType::U32),
            ),
            (OperationKind::Constant(Constant::Bool(false)), Type::BOOL),
            (OperationKind::Constant(Constant::Bool(true)), Type::BOOL),
            (
                OperationKind::Compare {
                    predicate: ComparePredicate::LessThan,
                    lhs: plan.value(0),
                    rhs: plan.value(2),
                },
                Type::BOOL,
            ),
        ];
        for (ordinal, (kind, ty)) in preludes.into_iter().enumerate() {
            self.emit(
                current,
                kind,
                [ValueDef::new(plan.value(ordinal as u32), ty)],
                Stage::Prelude,
                None,
                true,
            )?;
        }
        for j in 0..u32::from(plan.elements) {
            let component = Some(j);
            let first = 8 + 20 * j;
            let value = |n| plan.value(first + n);
            self.emit(
                current,
                OperationKind::Constant(Constant::Index(u64::from(j))),
                [ValueDef::new(value(0), Type::INDEX)],
                Stage::Predicate,
                component,
                true,
            )?;
            let (multiplicand, multiplier, extra) = match plan.order {
                ScopedTileOrderV29::Blocked => (plan.value(0), plan.value(3), value(0)),
                ScopedTileOrderV29::Striped => (value(0), plan.value(2), plan.value(0)),
            };
            for (offset, operator, lhs, rhs) in [
                (1, CheckedBinaryOperator::Multiply, multiplicand, multiplier),
                (3, CheckedBinaryOperator::Add, value(1), extra),
                (5, CheckedBinaryOperator::Add, plan.base, value(3)),
            ] {
                self.emit(
                    current,
                    OperationKind::Binary {
                        op: BinaryOp::Checked(operator),
                        lhs,
                        rhs,
                    },
                    [
                        ValueDef::new(value(offset), Type::INDEX),
                        ValueDef::new(value(offset + 1), Type::BOOL),
                    ],
                    Stage::Predicate,
                    component,
                    true,
                )?;
            }
            self.emit(
                current,
                OperationKind::Compare {
                    predicate: ComparePredicate::LessThan,
                    lhs: value(5),
                    rhs: plan.value(1),
                },
                [ValueDef::new(value(7), Type::BOOL)],
                Stage::Predicate,
                component,
                true,
            )?;
            for (ordinal, operand) in [value(2), value(4), value(6)].into_iter().enumerate() {
                self.emit(
                    current,
                    OperationKind::Unary {
                        op: UnaryOp::Not,
                        operand,
                    },
                    [ValueDef::new(value(8 + ordinal as u32), Type::BOOL)],
                    Stage::Predicate,
                    component,
                    true,
                )?;
            }
            let mut lhs = plan.value(7);
            for (ordinal, rhs) in [value(8), value(9), value(10), value(7)]
                .into_iter()
                .enumerate()
            {
                let result = value(11 + ordinal as u32);
                self.emit(
                    current,
                    OperationKind::Binary {
                        op: BinaryOp::BitAnd,
                        lhs,
                        rhs,
                    },
                    [ValueDef::new(result, Type::BOOL)],
                    Stage::Predicate,
                    component,
                    true,
                )?;
                lhs = result;
            }
            let read = plan.ordinal((2 * j) as usize);
            let join = plan.ordinal((2 * j + 1) as usize);
            let false_arguments = self.arguments([plan.value(4), plan.value(5)])?;
            self.terminator(
                current,
                Terminator::ConditionalBranch {
                    condition: value(14),
                    then_target: plan.block(2 * j),
                    then_arguments: Vec::new(),
                    else_target: plan.block(2 * j + 1),
                    else_arguments: false_arguments,
                },
                Stage::Conditional,
                component,
                true,
            )?;
            self.piece(
                TileScalarPieceV29::Block {
                    function: self.function,
                    block: read,
                },
                Stage::ReadBlock,
                component,
            )?;
            let data_type = self.pointer_type()?;
            self.emit(
                read,
                OperationKind::SliceData { slice: plan.input },
                [ValueDef::new(value(15), data_type)],
                Stage::Read,
                component,
                true,
            )?;
            let element_type = self.pointer_type()?;
            self.emit(
                read,
                OperationKind::GetElementPointer {
                    base: value(15),
                    offset: value(5),
                },
                [ValueDef::new(value(16), element_type)],
                Stage::Read,
                component,
                true,
            )?;
            self.emit(
                read,
                OperationKind::Load {
                    pointer: value(16),
                    access: MemoryAccess::new(AddressSpace::Global, 1),
                },
                [ValueDef::new(value(17), Type::Scalar(ScalarType::U32))],
                Stage::Read,
                component,
                true,
            )?;
            let true_arguments = self.arguments([value(17), plan.value(6)])?;
            self.terminator(
                read,
                Terminator::Branch {
                    target: plan.block(2 * j + 1),
                    arguments: true_arguments,
                },
                Stage::ReadBranch,
                component,
                true,
            )?;
            self.piece(
                TileScalarPieceV29::Block {
                    function: self.function,
                    block: join,
                },
                Stage::Join,
                component,
            )?;
            for (parameter, definition) in [
                ValueDef::new(value(18), Type::Scalar(ScalarType::U32)),
                ValueDef::new(value(19), Type::BOOL),
            ]
            .into_iter()
            .enumerate()
            {
                emission_push_shared_v1(
                    &mut self.body.blocks[join].parameters,
                    definition,
                    self.budget,
                )?;
                self.piece(
                    TileScalarPieceV29::BlockParameter {
                        function: self.function,
                        block: join,
                        parameter,
                    },
                    Stage::Join,
                    component,
                )?;
            }
            current = join;
        }
        Ok(current)
    }
}

#[derive(Clone, Copy)]
enum TileResultMapV29 {
    Preserved(TileScalarPointV29),
    Parts(TileScalarPointV29, usize),
    Erased,
}
fn tile_emit_module_v29(
    original: &Module,
    index: &AssertGraphIndexV1<'_>,
    plans: &[TileLoadPlanV29],
    module: &mut Module,
    budget: &mut ArgumentBudgetV1<'_>,
) -> TileBuildResultV29<TileScalarRelationsV29> {
    use TileScalarPieceV29 as Piece;
    use TileScalarSourceV29 as Source;
    use TileScalarStageV29 as Stage;
    let mut relations = TileScalarRelationsV29 {
        origins: Vec::new(),
        pieces: Vec::new(),
    };
    for (function, source_function) in original.functions.iter().enumerate() {
        budget.charge_work(1)?;
        let Some(source_body) = &source_function.body else {
            continue;
        };
        let body = module
            .functions
            .get_mut(function)
            .and_then(|f| f.body.as_mut())
            .ok_or(ScopedTileFailureKindV29::ReplayMismatch)?;
        budget.charge_work(plans.len())?;
        for plan in plans.iter().filter(|p| p.point.function == function) {
            for offset in 0..2 * u32::from(plan.elements) {
                if body.blocks.len() != plan.ordinal(offset as usize) {
                    return Err(ScopedTileFailureKindV29::ReplayMismatch);
                }
                emission_push_shared_v1(
                    &mut body.blocks,
                    BasicBlock::new(plan.block(offset)),
                    budget,
                )?;
            }
        }
        let mut emitter = TileEmitterV29 {
            function,
            body,
            relations: &mut relations,
            budget,
        };
        for parameter in 0..source_body.parameters.len() {
            emitter.single(
                Source::FunctionParameter {
                    function,
                    parameter,
                },
                Piece::FunctionParameter {
                    function,
                    parameter,
                },
            )?;
        }
        for (block, source_block) in source_body.blocks.iter().enumerate() {
            emitter.single(
                Source::Block { function, block },
                Piece::Block { function, block },
            )?;
            for parameter in 0..source_block.parameters.len() {
                emitter.single(
                    Source::BlockParameter {
                        function,
                        block,
                        parameter,
                    },
                    Piece::BlockParameter {
                        function,
                        block,
                        parameter,
                    },
                )?;
            }
            let operations = std::mem::take(&mut emitter.body.blocks[block].operations);
            let terminator = emitter.body.blocks[block]
                .terminator
                .take()
                .ok_or(ScopedTileFailureKindV29::ReplayMismatch)?;
            let mut current = block;
            for (operation_ordinal, operation) in operations.into_iter().enumerate() {
                emitter.budget.charge_work(1)?;
                let source_point = TileScalarPointV29 {
                    function,
                    block,
                    operation: operation_ordinal,
                };
                let source_operation = source_block
                    .operations
                    .get(operation_ordinal)
                    .ok_or(ScopedTileFailureKindV29::ReplayMismatch)?;
                let first = emitter.relations.pieces.len();
                let result_map = match &source_operation.kind {
                    OperationKind::Execution(TileExecutionV29::MaskedTileLoadU32 { .. }) => {
                        emitter
                            .budget
                            .charge_work(tile_build_add_v29(plans.len(), 1)?)?;
                        let plan = plans
                            .iter()
                            .find(|p| p.point == source_point)
                            .ok_or(ScopedTileFailureKindV29::Census)?;
                        current = emitter.load(*plan, current)?;
                        TileResultMapV29::Erased
                    }
                    OperationKind::Execution(TileExecutionV29::FragmentIntoPartsU32 {
                        fragment,
                        lanes,
                        elements,
                    }) => {
                        let plan = tile_parts_plan_v29(
                            original,
                            index,
                            plans,
                            function,
                            *fragment,
                            *lanes,
                            *elements,
                            emitter.budget,
                        )?;
                        let count = usize::from(*elements);
                        let start = TileScalarPointV29 {
                            function,
                            block: current,
                            operation: emitter.body.blocks[current].operations.len(),
                        };
                        for (result, definition) in source_operation.results.iter().enumerate() {
                            emitter.budget.charge_work(1)?;
                            let component = result % count;
                            let ty = if result < count {
                                Type::Scalar(ScalarType::U32)
                            } else {
                                Type::BOOL
                            };
                            if definition.ty != ty {
                                return Err(ScopedTileFailureKindV29::ReplayMismatch);
                            }
                            let offset =
                                8 + 20 * component as u32 + if result < count { 18 } else { 19 };
                            let saved = plan.value(offset);
                            emitter.emit(
                                current,
                                OperationKind::Select {
                                    condition: plan.value(6),
                                    true_value: saved,
                                    false_value: saved,
                                },
                                [ValueDef::new(definition.id, ty)],
                                Stage::Parts,
                                Some(component as u32),
                                false,
                            )?;
                        }
                        TileResultMapV29::Parts(start, count)
                    }
                    OperationKind::Execution(
                        TileExecutionV29::ContextIssue
                        | TileExecutionV29::WorkgroupDerive { .. }
                        | TileExecutionV29::TileIntoFragmentU32 { .. }
                        | TileExecutionV29::ScopeEnd { .. },
                    ) => {
                        let gap = TileScalarPointV29 {
                            function,
                            block: current,
                            operation: emitter.body.blocks[current].operations.len(),
                        };
                        emitter.piece(Piece::Anchor(gap), Stage::Erased, None)?;
                        TileResultMapV29::Erased
                    }
                    _ => TileResultMapV29::Preserved(emitter.push_operation(
                        current,
                        operation,
                        Stage::Preserved,
                        None,
                        false,
                    )?),
                };
                emitter.origin(Source::Operation(source_point), first)?;
                for (result, definition) in source_operation.results.iter().enumerate() {
                    let first = emitter.relations.pieces.len();
                    match result_map {
                        TileResultMapV29::Preserved(point) => {
                            emitter.piece(
                                Piece::Result {
                                    operation: point,
                                    result,
                                },
                                Stage::Preserved,
                                None,
                            )?;
                        }
                        TileResultMapV29::Parts(mut point, count) => {
                            point.operation = tile_build_add_v29(point.operation, result)?;
                            emitter.piece(
                                Piece::Result {
                                    operation: point,
                                    result: 0,
                                },
                                Stage::Parts,
                                Some((result % count) as u32),
                            )?;
                        }
                        TileResultMapV29::Erased => {
                            emitter.piece(
                                Piece::ErasedValue(definition.id),
                                Stage::Erased,
                                None,
                            )?;
                        }
                    }
                    emitter.origin(
                        Source::Result {
                            operation: source_point,
                            result,
                        },
                        first,
                    )?;
                }
            }
            let first = emitter.relations.pieces.len();
            let edges = emitter.terminator(current, terminator, Stage::Preserved, None, false)?;
            emitter.origin(Source::Terminator { function, block }, first)?;
            for edge in 0..edges {
                emitter.single(
                    Source::Edge {
                        function,
                        block,
                        edge,
                    },
                    Piece::Edge {
                        function,
                        block: current,
                        edge,
                    },
                )?;
            }
        }
    }
    Ok(relations)
}
fn tile_scalar_build_v29(
    input: &PreparedScopedTileSourceV29,
    module: &mut Module,
    budget: &mut ArgumentBudgetV1<'_>,
) -> TileBuildResultV29<TileScalarRelationsV29> {
    let original = input.pending.inner.pending.graph.module();
    let index = AssertGraphIndexV1::build_functions(&original.functions, true, budget)
        .map_err(ProductionSemanticKirErrorV1::AssertOrigin)?;
    let plans = tile_plan_loads_v29(
        original,
        &input.pending.inner.pending.roots,
        &input.selections,
        budget,
    )?;
    let relations = tile_emit_module_v29(original, &index, &plans, module, budget)?;
    index
        .release(budget)
        .map_err(ProductionSemanticKirErrorV1::AssertOrigin)?;
    let plan_bytes = argument_product_v1(plans.capacity(), size_of::<TileLoadPlanV29>())?;
    drop(plans);
    budget.release_storage(plan_bytes)?;
    Ok(relations)
}

// Raw graph-boundary coverage only; no fabricated Pending/source owner.
#[cfg(test)]
pub(super) mod max_id_graph_boundary_tests {
    use super::*;

    pub(in super::super) fn graph_only_emit_and_check(
        original: &Module,
        check: impl FnOnce(&Module, &TileScalarRelationsV29),
    ) {
        let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(1_000_000);
        let mut budget = ArgumentBudgetV1::new(&mut work, 1_000_000);
        budget.reserve_storage(37).unwrap();
        let (input, input_receipt) =
            fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV15::from_module_ref_with_verification_budget_v15(
                original, &mut budget,
            ).expect("raw input must independently pass V15 graph admission");
        let input_paid = input_receipt.retained_storage();
        budget.reserve_storage(input_paid).unwrap();
        let index =
            AssertGraphIndexV1::build_functions(&input.module().functions, true, &mut budget)
                .unwrap();
        let mut output = input.module().clone();
        let relations =
            tile_emit_module_v29(input.module(), &index, &[], &mut output, &mut budget).unwrap();
        index.release(&mut budget).unwrap();
        let retained = relations.storage().unwrap();
        assert_eq!(budget.storage(), 37 + input_paid + retained);
        let (verified, receipt) =
            fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12::from_module_ref_with_verification_budget_v12(
                &output, &mut budget,
            ).expect("erased raw graph must independently pass V12 admission");
        let paid = receipt.retained_storage();
        budget.reserve_storage(paid).unwrap();
        assert_eq!(verified.module(), &output);
        check(verified.module(), &relations);
        drop(verified);
        budget.release_storage(paid).unwrap();
        drop(output);
        drop(relations);
        budget.release_storage(retained).unwrap();
        drop(input);
        budget.release_storage(input_paid).unwrap();
        assert_eq!(budget.storage(), 37);
        budget.release_storage(37).unwrap();
    }

    pub(in super::super) fn assert_missing_tile_root(
        input: &PreparedScopedTileSourceV29,
        missing: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) {
        let floor = budget.storage();
        let identity = *input.pending.pending_identity();
        let roots = &input.pending.inner.pending.roots;
        assert_ne!(roots[missing].function_ordinal, 0);
        let selected: Vec<_> = input
            .selections
            .iter()
            .copied()
            .filter(|row| row.root != missing)
            .collect();
        assert!(!selected.is_empty());
        assert!(selected.len() < input.selections.len());
        let result = scoped_tile_attempt_v29(budget, |budget| {
            tile_plan_loads_v29(input.pending.pending_module(), roots, &selected, budget)
        });
        assert!(matches!(result, Err(ScopedTileFailureKindV29::Census)));
        assert_eq!(budget.storage(), floor);
        assert_eq!(input.pending.pending_identity(), &identity);
        input.replay_with_budget(budget).unwrap();
        assert_eq!(budget.storage(), floor);
    }

    // Census uses borrowed source evidence; ID limits use caller-owned raw graph
    // variants only. No mutated graph is promoted to a Pending owner.
    pub(in super::super) fn assert_planner_boundaries(
        input: &PreparedScopedTileSourceV29,
        budget: &mut ArgumentBudgetV1<'_>,
    ) {
        let original = input.pending.pending_module();
        let roots = &input.pending.inner.pending.roots;
        let selections = input.selections.clone();
        assert_eq!(roots.len(), 1);
        assert!(!selections.is_empty());
        let floor = budget.storage();
        let identity = *input.pending.pending_identity();
        let mut duplicate = selections.clone();
        duplicate.push(selections[0]);
        let mut unmatched = selections.clone();
        let mut extra = selections[0];
        extra.witness.after.first = u32::MAX;
        unmatched.push(extra);
        let mut invalid_root = selections.clone();
        invalid_root[0].root = roots.len();
        for wrong in [
            &selections[..selections.len() - 1],
            duplicate.as_slice(),
            unmatched.as_slice(),
            invalid_root.as_slice(),
        ] {
            let result = scoped_tile_attempt_v29(budget, |budget| {
                tile_plan_loads_v29(original, roots, wrong, budget)
            });
            assert!(matches!(result, Err(ScopedTileFailureKindV29::Census)));
            assert_eq!(budget.storage(), floor);
        }
        let values: u32 = selections
            .iter()
            .map(|row| 8 + 20 * u32::from(row.tile.elements))
            .sum();
        let blocks: u32 = selections
            .iter()
            .map(|row| 2 * u32::from(row.tile.elements))
            .sum();
        for (value_short, block_short) in [(0, 0), (1, 0), (0, 1)] {
            let mut graph = original.clone();
            let body = graph.functions[roots[0].function_ordinal]
                .body
                .as_mut()
                .unwrap();
            body.blocks[0].operations.push(Operation::effect_free(
                ValueDef::new(ValueId(u32::MAX - values + value_short), Type::INDEX),
                OperationKind::Constant(Constant::Index(0)),
            ));
            let mut spare = BasicBlock::new(BlockId(u32::MAX - blocks + block_short));
            spare.terminator = Some(Terminator::Return { values: vec![] });
            body.blocks.push(spare);
            let result = scoped_tile_attempt_v29(budget, |budget| {
                tile_plan_loads_v29(&graph, roots, &selections, budget)
            });
            if value_short == 0 && block_short == 0 {
                let plans = result.unwrap();
                assert_eq!(plans.len(), selections.len());
                let last = *plans.last().unwrap();
                assert_eq!(
                    last.value(8 + 20 * u32::from(last.elements) - 1).0,
                    u32::MAX
                );
                assert_eq!(last.block(2 * u32::from(last.elements) - 1).0, u32::MAX);
                let retained = plans.capacity() * size_of::<TileLoadPlanV29>();
                assert_eq!(budget.storage(), floor + retained);
                drop(plans);
                budget.release_storage(retained).unwrap();
            } else {
                assert!(matches!(
                    result,
                    Err(ScopedTileFailureKindV29::Resource(
                        ArgumentResourceV1::Arithmetic
                    ))
                ));
            }
            assert_eq!(budget.storage(), floor);
        }
        assert_eq!(input.pending.pending_identity(), &identity);
        assert_eq!(input.selections, selections);
        input.replay_with_budget(budget).unwrap();
        assert_eq!(budget.storage(), floor);
    }

    #[test]
    fn graph_only_max_value_and_block_ids_need_no_fresh_ids_without_loads() {
        let maximum = ValueId(u32::MAX);
        let mut block = BasicBlock::new(BlockId(u32::MAX));
        block.terminator = Some(Terminator::Return {
            values: vec![maximum],
        });
        let mut original = Module::new("graph_boundary_maximum_ids");
        original.functions.push(Function::definition(
            "maximum",
            fe2o3_kernel_ir::Signature::new(
                vec![Type::Scalar(ScalarType::U32)],
                vec![Type::Scalar(ScalarType::U32)],
            ),
            vec![maximum],
            vec![block],
        ));
        let mut output = original.clone();
        let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(1_000_000);
        let mut budget = ArgumentBudgetV1::new(&mut work, 1_000_000);
        budget.reserve_storage(37).unwrap();
        let index =
            AssertGraphIndexV1::build_functions(&original.functions, true, &mut budget).unwrap();
        let before_plan = budget.storage();
        let before_plan_work = budget.work();
        let plans = tile_plan_loads_v29(&original, &[], &[], &mut budget).unwrap();
        assert!(plans.is_empty());
        assert_eq!(plans.capacity(), 0, "no invented plan allocation");
        assert_eq!(budget.storage(), before_plan);
        assert!(budget.work() > before_plan_work);
        let relations =
            tile_emit_module_v29(&original, &index, &plans, &mut output, &mut budget).unwrap();
        drop(plans);
        assert_eq!(output, original);
        assert_eq!(relations.origins.len(), 3);
        assert_eq!(relations.pieces.len(), 3);
        for (ordinal, row) in relations.origins.iter().enumerate() {
            assert_eq!((row.first, row.count), (ordinal, 1));
            assert_eq!(
                relations.pieces[ordinal].stage,
                TileScalarStageV29::Preserved
            );
            assert_eq!(relations.pieces[ordinal].component, None);
        }
        index.release(&mut budget).unwrap();
        let retained = relations.storage().unwrap();
        assert_eq!(budget.storage(), 37 + retained);
        drop(relations);
        budget.release_storage(retained).unwrap();
        assert_eq!(budget.storage(), 37);
        budget.release_storage(37).unwrap();
    }
}
