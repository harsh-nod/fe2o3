use scoped_tile_materialization_replay_v29::replay_tile_scalar_graph_v29;

#[cfg(test)]
use scoped_tile_graph_test_oracles_v29::*;

#[cfg(test)]
mod scoped_tile_graph_test_oracles_v29 {
    use super::*;
    use std::collections::BTreeMap;

    pub(super) fn operation_points(
        relations: &TileScalarRelationsV29,
        row: TileScalarOriginV29,
    ) -> Vec<TileScalarPointV29> {
        relations.pieces[row.first..row.first + row.count]
            .iter()
            .filter_map(|piece| match piece.piece {
                TileScalarPieceV29::Operation(point) => Some(point),
                _ => None,
            })
            .collect()
    }

    pub(super) fn output_operation(graph: &Module, point: TileScalarPointV29) -> &Operation {
        &graph.functions[point.function]
            .body
            .as_ref()
            .unwrap()
            .blocks[point.block]
            .operations[point.operation]
    }
    pub(super) fn output_operation_mut(
        graph: &mut Module,
        point: TileScalarPointV29,
    ) -> &mut Operation {
        &mut graph.functions[point.function]
            .body
            .as_mut()
            .unwrap()
            .blocks[point.block]
            .operations[point.operation]
    }

    pub(super) fn preservation_edges(function: &Function) -> Vec<Vec<usize>> {
        let body = function.body.as_ref().expect("function body");
        let ordinals: BTreeMap<_, _> = body
            .blocks
            .iter()
            .enumerate()
            .map(|(ordinal, block)| (block.id, ordinal))
            .collect();
        assert_eq!(ordinals.len(), body.blocks.len());
        body.blocks
            .iter()
            .map(|block| {
                let mut edges = Vec::new();
                block
                    .terminator
                    .as_ref()
                    .unwrap()
                    .try_visit_edges_v1(|target, _| {
                        edges.push(*ordinals.get(&target).expect("retained target"));
                        Ok::<(), ()>(())
                    })
                    .unwrap();
                edges
            })
            .collect()
    }

    pub(super) fn preservation_has_cycle(edges: &[Vec<usize>]) -> bool {
        fn visit(node: usize, edges: &[Vec<usize>], states: &mut [u8]) -> bool {
            match states[node] {
                1 => return true,
                2 => return false,
                _ => {}
            }
            states[node] = 1;
            for &next in &edges[node] {
                if visit(next, edges, states) {
                    return true;
                }
            }
            states[node] = 2;
            false
        }
        assert!(!edges.is_empty());
        visit(0, edges, &mut vec![0; edges.len()])
    }

    pub(super) fn preservation_source_function(source: TileScalarSourceV29) -> usize {
        match source {
            TileScalarSourceV29::FunctionParameter { function, .. }
            | TileScalarSourceV29::Block { function, .. }
            | TileScalarSourceV29::BlockParameter { function, .. }
            | TileScalarSourceV29::Terminator { function, .. }
            | TileScalarSourceV29::Edge { function, .. } => function,
            TileScalarSourceV29::Operation(point)
            | TileScalarSourceV29::Result {
                operation: point, ..
            } => point.function,
        }
    }
}
use scoped_tile_projection_replay_v29::replay_scoped_tile_attachments_v29;

#[cfg_attr(
    not(test),
    allow(dead_code, reason = "Scoped tile projection remains gated")
)]
mod scoped_tile_projection_replay_v29 {
    use super::*;
    include!("production_scoped_tile_projection_replay_v29.rs");
}

#[cfg_attr(
    not(test),
    allow(dead_code, reason = "Scoped tile correspondence remains gated")
)]
mod scoped_tile_materialization_replay_v29 {
    use super::*;
    use super::{
        TileScalarPieceV29 as Piece, TileScalarSourceV29 as Source, TileScalarStageV29 as Stage,
    };
    use fe2o3_kernel_ir::{
        CanonicalKirDefinitionCoordinateV1 as Definition,
        CanonicalKirFunctionCoordinateV1 as FunctionCoordinate, CheckedBinaryOperator as Checked,
        ExecutionOperationV15 as Execution, IndexKind, IntrinsicKind, IntrinsicOperation,
    };

    type R<T> = Result<T, ScopedTileFailureKindV29>;

    fn require(valid: bool) -> R<()> {
        if valid {
            Ok(())
        } else {
            Err(ScopedTileFailureKindV29::ReplayMismatch)
        }
    }
    fn assert_error(error: SemanticKirAssertOriginErrorV1) -> ScopedTileFailureKindV29 {
        ProductionSemanticKirErrorV1::AssertOrigin(error).into()
    }
    fn key(point: TileScalarPointV29) -> (usize, usize, usize) {
        (point.function, point.block, point.operation)
    }
    fn definition<'a>(
        graph: &'a Module,
        index: &AssertGraphIndexV1<'_>,
        function: usize,
        value: ValueId,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> R<(TileScalarPointV29, &'a Operation)> {
        let function = FunctionCoordinate(
            u32::try_from(function).map_err(|_| ArgumentResourceV1::Arithmetic)?,
        );
        let found = index
            .definition(function, value, budget)
            .map_err(assert_error)?;
        let Definition::Result {
            operation,
            result: 0,
        } = found.coordinate
        else {
            return Err(ScopedTileFailureKindV29::ReplayMismatch);
        };
        let point = TileScalarPointV29 {
            function: operation.block.function.0 as usize,
            block: operation.block.block as usize,
            operation: operation.operation as usize,
        };
        let operation = graph
            .functions
            .get(point.function)
            .and_then(|function| function.body.as_ref())
            .and_then(|body| body.blocks.get(point.block))
            .and_then(|block| block.operations.get(point.operation))
            .ok_or(ScopedTileFailureKindV29::ReplayMismatch)?;
        require(matches!(operation.results.as_slice(), [result] if result.id == value))?;
        Ok((point, operation))
    }

    #[derive(Clone, Copy)]
    struct Load {
        point: TileScalarPointV29,
        input: ValueId,
        base: ValueId,
        lanes: u16,
        elements: u16,
        order: ScopedTileOrderV29,
        first_value: u32,
        first_block: u32,
        suffix: usize,
    }
    impl Load {
        fn value(self, offset: u32) -> R<ValueId> {
            self.first_value
                .checked_add(offset)
                .map(ValueId)
                .ok_or_else(|| ArgumentResourceV1::Arithmetic.into())
        }
        fn block(self, offset: u32) -> R<BlockId> {
            self.first_block
                .checked_add(offset)
                .map(BlockId)
                .ok_or_else(|| ArgumentResourceV1::Arithmetic.into())
        }
    }
    fn census(
        input: &PreparedScopedTileSourceV29,
        index: &AssertGraphIndexV1<'_>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> R<Vec<Load>> {
        let graph = input.pending.inner.pending.graph.module();
        let mut loads = Vec::new();
        for selection in &input.selections {
            budget.charge_work(1)?;
            let root = input
                .pending
                .inner
                .pending
                .roots
                .get(selection.root)
                .ok_or(ScopedTileFailureKindV29::ReplayMismatch)?;
            let (point, operation) = definition(
                graph,
                index,
                root.function_ordinal,
                selection.tile.first_result,
                budget,
            )?;
            let OperationKind::Execution(Execution::MaskedTileLoadU32 {
                input: slice,
                base,
                lanes,
                elements,
                ..
            }) = &operation.kind
            else {
                return Err(ScopedTileFailureKindV29::ReplayMismatch);
            };
            require(point.operation == selection.witness.after.first as usize)?;
            let block = &graph.functions[point.function]
                .body
                .as_ref()
                .ok_or(ScopedTileFailureKindV29::ReplayMismatch)?
                .blocks[point.block];
            require(block.id == selection.witness.after.block)?;
            emission_push_v1(
                &mut loads,
                Load {
                    point,
                    input: *slice,
                    base: *base,
                    lanes: *lanes,
                    elements: *elements,
                    order: selection.order,
                    first_value: 0,
                    first_block: 0,
                    suffix: 0,
                },
                budget,
            )?;
        }
        assert_origin_sort_v1(&mut loads, budget, |a, b, _| {
            Ok(key(a.point).cmp(&key(b.point)))
        })
        .map_err(assert_error)?;
        for pair in loads.windows(2) {
            budget.charge_work(1)?;
            require(key(pair[0].point) != key(pair[1].point))?;
        }
        let mut next = 0;
        for (function, source) in graph.functions.iter().enumerate() {
            budget.charge_work(1)?;
            let Some(body) = &source.body else { continue };
            let mut value_max = 0u32;
            let mut block_max = 0u32;
            for value in &body.parameters {
                budget.charge_work(1)?;
                value_max = value_max.max(value.0);
            }
            for block in &body.blocks {
                budget.charge_work(1)?;
                block_max = block_max.max(block.id.0);
                for value in &block.parameters {
                    budget.charge_work(1)?;
                    value_max = value_max.max(value.id.0);
                }
                for operation in &block.operations {
                    budget.charge_work(1)?;
                    for value in &operation.results {
                        budget.charge_work(1)?;
                        value_max = value_max.max(value.id.0);
                    }
                }
            }
            let mut suffix = body.blocks.len();
            for (block, source) in body.blocks.iter().enumerate() {
                for (operation, source) in source.operations.iter().enumerate() {
                    budget.charge_work(1)?;
                    if !matches!(
                        source.kind,
                        OperationKind::Execution(Execution::MaskedTileLoadU32 { .. })
                    ) {
                        continue;
                    }
                    let point = TileScalarPointV29 {
                        function,
                        block,
                        operation,
                    };
                    let load = loads
                        .get_mut(next)
                        .ok_or(ScopedTileFailureKindV29::ReplayMismatch)?;
                    require(load.point == point)?;
                    let values = u32::from(load.elements)
                        .checked_mul(20)
                        .and_then(|n| n.checked_add(8))
                        .ok_or(ArgumentResourceV1::Arithmetic)?;
                    let blocks = u32::from(load.elements)
                        .checked_mul(2)
                        .ok_or(ArgumentResourceV1::Arithmetic)?;
                    let last_value = value_max
                        .checked_add(values)
                        .ok_or(ArgumentResourceV1::Arithmetic)?;
                    let last_block = block_max
                        .checked_add(blocks)
                        .ok_or(ArgumentResourceV1::Arithmetic)?;
                    load.first_value = value_max
                        .checked_add(1)
                        .ok_or(ArgumentResourceV1::Arithmetic)?;
                    load.first_block = block_max
                        .checked_add(1)
                        .ok_or(ArgumentResourceV1::Arithmetic)?;
                    load.suffix = suffix;
                    value_max = last_value;
                    block_max = last_block;
                    suffix = argument_sum_v1(&[suffix, blocks as usize])?;
                    next = argument_sum_v1(&[next, 1])?;
                }
            }
        }
        require(next == loads.len())?;
        Ok(loads)
    }
    fn fragment_load(
        graph: &Module,
        index: &AssertGraphIndexV1<'_>,
        loads: &[Load],
        function: usize,
        fragment: ValueId,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> R<Load> {
        let (_, operation) = definition(graph, index, function, fragment, budget)?;
        let OperationKind::Execution(Execution::TileIntoFragmentU32 {
            tile,
            lanes,
            elements,
        }) = &operation.kind
        else {
            return Err(ScopedTileFailureKindV29::ReplayMismatch);
        };
        let (point, operation) = definition(graph, index, function, *tile, budget)?;
        require(matches!(&operation.kind,
            OperationKind::Execution(Execution::MaskedTileLoadU32 { lanes: l, elements: e, .. })
            if (l, e) == (lanes, elements)))?;
        let at =
            assert_origin_find_v1(
                loads,
                budget,
                |load, _| Ok(key(load.point).cmp(&key(point))),
            )
            .map_err(assert_error)?
            .ok_or(ScopedTileFailureKindV29::ReplayMismatch)?;
        Ok(loads[at])
    }

    #[derive(Clone, Copy)]
    enum Ty {
        Index,
        U32,
        Bool,
        Pointer,
    }
    impl Ty {
        fn matches(self, ty: &Type) -> bool {
            match self {
                Self::Index => *ty == Type::INDEX,
                Self::U32 => *ty == Type::Scalar(ScalarType::U32),
                Self::Bool => *ty == Type::BOOL,
                Self::Pointer => matches!(ty, Type::Pointer(pointer)
                    if pointer.address_space == AddressSpace::Global
                        && pointer.access == AccessMode::ReadOnly
                        && *pointer.pointee == Type::Scalar(ScalarType::U32)),
            }
        }
    }
    struct Checker<'a, 'b, 'work> {
        output: &'a Module,
        relations: &'a TileScalarRelationsV29,
        budget: &'b mut ArgumentBudgetV1<'work>,
        row: usize,
        piece: usize,
        end: usize,
    }
    impl Checker<'_, '_, '_> {
        fn row(&mut self, source: Source, count: usize) -> R<()> {
            self.budget.charge_work(1)?;
            require(self.piece == self.end)?;
            let expected = TileScalarOriginV29 {
                source,
                first: self.piece,
                count,
            };
            require(self.relations.origins.get(self.row) == Some(&expected))?;
            self.end = argument_sum_v1(&[self.piece, count])?;
            require(self.end <= self.relations.pieces.len())?;
            self.row = argument_sum_v1(&[self.row, 1])?;
            Ok(())
        }
        fn piece(&mut self, piece: Piece, component: Option<u32>, stage: Stage) -> R<()> {
            self.budget.charge_work(1)?;
            require(self.piece < self.end)?;
            let expected = TileScalarTaggedPieceV29 {
                piece,
                component,
                stage,
            };
            require(self.relations.pieces.get(self.piece) == Some(&expected))?;
            self.piece = argument_sum_v1(&[self.piece, 1])?;
            Ok(())
        }
        fn preserved(&mut self, source: Source, piece: Piece) -> R<()> {
            self.row(source, 1)?;
            self.piece(piece, None, Stage::Preserved)
        }
        fn block(&self, function: usize, block: usize) -> R<&BasicBlock> {
            self.output
                .functions
                .get(function)
                .and_then(|f| f.body.as_ref())
                .and_then(|b| b.blocks.get(block))
                .ok_or(ScopedTileFailureKindV29::ReplayMismatch)
        }
        fn end_block(&mut self, at: TileScalarPointV29) -> R<()> {
            self.budget.charge_work(1)?;
            require(self.block(at.function, at.block)?.operations.len() == at.operation)
        }
        fn operation(
            &mut self,
            at: &mut TileScalarPointV29,
            kind: OperationKind,
            results: &[(ValueId, Ty)],
            component: Option<u32>,
            stage: Stage,
            own_results: bool,
        ) -> R<()> {
            self.budget
                .charge_work(argument_sum_v1(&[1, results.len()])?)?;
            let actual = self
                .block(at.function, at.block)?
                .operations
                .get(at.operation)
                .ok_or(ScopedTileFailureKindV29::ReplayMismatch)?;
            require(actual.kind == kind && actual.results.len() == results.len())?;
            for (actual, (id, ty)) in actual.results.iter().zip(results) {
                require(actual.id == *id && ty.matches(&actual.ty))?;
            }
            self.piece(Piece::Operation(*at), component, stage)?;
            if own_results {
                for result in 0..results.len() {
                    self.piece(
                        Piece::Result {
                            operation: *at,
                            result,
                        },
                        component,
                        stage,
                    )?;
                }
            }
            at.operation = argument_sum_v1(&[at.operation, 1])?;
            Ok(())
        }
        fn conditional(
            &mut self,
            at: TileScalarPointV29,
            condition: ValueId,
            targets: [BlockId; 2],
            fallback: [ValueId; 2],
            component: u32,
        ) -> R<()> {
            self.end_block(at)?;
            require(matches!(&self.block(at.function, at.block)?.terminator,
                Some(Terminator::ConditionalBranch {
                    condition: c, then_target, then_arguments, else_target, else_arguments,
                }) if *c == condition && *then_target == targets[0] && then_arguments.is_empty()
                    && *else_target == targets[1] && else_arguments.as_slice() == fallback))?;
            self.piece(
                Piece::Terminator {
                    function: at.function,
                    block: at.block,
                },
                Some(component),
                Stage::Conditional,
            )?;
            for edge in 0..2 {
                self.piece(
                    Piece::Edge {
                        function: at.function,
                        block: at.block,
                        edge,
                    },
                    Some(component),
                    Stage::Conditional,
                )?;
            }
            Ok(())
        }
        fn read_branch(
            &mut self,
            at: TileScalarPointV29,
            target: BlockId,
            values: [ValueId; 2],
            component: u32,
        ) -> R<()> {
            self.end_block(at)?;
            require(matches!(&self.block(at.function, at.block)?.terminator,
                Some(Terminator::Branch { target: t, arguments })
                if *t == target && arguments.as_slice() == values))?;
            self.piece(
                Piece::Terminator {
                    function: at.function,
                    block: at.block,
                },
                Some(component),
                Stage::ReadBranch,
            )?;
            self.piece(
                Piece::Edge {
                    function: at.function,
                    block: at.block,
                    edge: 0,
                },
                Some(component),
                Stage::ReadBranch,
            )
        }
        fn load(&mut self, load: Load, at: &mut TileScalarPointV29) -> R<()> {
            let v = |offset| load.value(offset);
            let prelude = [
                (
                    OperationKind::Intrinsic(IntrinsicOperation::new(
                        IntrinsicKind::InvocationIndex {
                            kind: IndexKind::Local,
                            axis: fe2o3_kernel_ir::Axis::X,
                        },
                        Type::INDEX,
                    )),
                    Ty::Index,
                ),
                (OperationKind::SliceLength { slice: load.input }, Ty::Index),
                (
                    OperationKind::Constant(Constant::Index(u64::from(load.lanes))),
                    Ty::Index,
                ),
                (
                    OperationKind::Constant(Constant::Index(u64::from(load.elements))),
                    Ty::Index,
                ),
                (OperationKind::Constant(Constant::U32(0)), Ty::U32),
                (OperationKind::Constant(Constant::Bool(false)), Ty::Bool),
                (OperationKind::Constant(Constant::Bool(true)), Ty::Bool),
                (
                    OperationKind::Compare {
                        predicate: ComparePredicate::LessThan,
                        lhs: v(0)?,
                        rhs: v(2)?,
                    },
                    Ty::Bool,
                ),
            ];
            for (ordinal, (kind, ty)) in prelude.into_iter().enumerate() {
                self.operation(
                    at,
                    kind,
                    &[(v(ordinal as u32)?, ty)],
                    None,
                    Stage::Prelude,
                    true,
                )?;
            }
            for j in 0..u32::from(load.elements) {
                let offset = j
                    .checked_mul(20)
                    .and_then(|n| n.checked_add(8))
                    .ok_or(ArgumentResourceV1::Arithmetic)?;
                let p = |i| {
                    load.value(
                        offset
                            .checked_add(i)
                            .ok_or(ArgumentResourceV1::Arithmetic)?,
                    )
                };
                self.operation(
                    at,
                    OperationKind::Constant(Constant::Index(u64::from(j))),
                    &[(p(0)?, Ty::Index)],
                    Some(j),
                    Stage::Predicate,
                    true,
                )?;
                let (lhs, rhs, addend) = match load.order {
                    ScopedTileOrderV29::Blocked => (v(0)?, v(3)?, p(0)?),
                    ScopedTileOrderV29::Striped => (p(0)?, v(2)?, v(0)?),
                };
                for (operator, lhs, rhs, result) in [
                    (Checked::Multiply, lhs, rhs, 1),
                    (Checked::Add, p(1)?, addend, 3),
                    (Checked::Add, load.base, p(3)?, 5),
                ] {
                    self.operation(
                        at,
                        OperationKind::Binary {
                            op: BinaryOp::Checked(operator),
                            lhs,
                            rhs,
                        },
                        &[(p(result)?, Ty::Index), (p(result + 1)?, Ty::Bool)],
                        Some(j),
                        Stage::Predicate,
                        true,
                    )?;
                }
                self.operation(
                    at,
                    OperationKind::Compare {
                        predicate: ComparePredicate::LessThan,
                        lhs: p(5)?,
                        rhs: v(1)?,
                    },
                    &[(p(7)?, Ty::Bool)],
                    Some(j),
                    Stage::Predicate,
                    true,
                )?;
                for (input, result) in [(2, 8), (4, 9), (6, 10)] {
                    self.operation(
                        at,
                        OperationKind::Unary {
                            op: UnaryOp::Not,
                            operand: p(input)?,
                        },
                        &[(p(result)?, Ty::Bool)],
                        Some(j),
                        Stage::Predicate,
                        true,
                    )?;
                }
                let mut condition = v(7)?;
                for (rhs, result) in [(8, 11), (9, 12), (10, 13), (7, 14)] {
                    self.operation(
                        at,
                        OperationKind::Binary {
                            op: BinaryOp::BitAnd,
                            lhs: condition,
                            rhs: p(rhs)?,
                        },
                        &[(p(result)?, Ty::Bool)],
                        Some(j),
                        Stage::Predicate,
                        true,
                    )?;
                    condition = p(result)?;
                }
                let block_offset = j.checked_mul(2).ok_or(ArgumentResourceV1::Arithmetic)?;
                let read_id = load.block(block_offset)?;
                let join_id = load.block(
                    block_offset
                        .checked_add(1)
                        .ok_or(ArgumentResourceV1::Arithmetic)?,
                )?;
                let read = argument_sum_v1(&[load.suffix, block_offset as usize])?;
                let join = argument_sum_v1(&[read, 1])?;
                self.conditional(*at, condition, [read_id, join_id], [v(4)?, v(5)?], j)?;
                let actual = self.block(at.function, read)?;
                require(actual.id == read_id && actual.parameters.is_empty())?;
                self.piece(
                    Piece::Block {
                        function: at.function,
                        block: read,
                    },
                    Some(j),
                    Stage::ReadBlock,
                )?;
                let mut read_at = TileScalarPointV29 {
                    function: at.function,
                    block: read,
                    operation: 0,
                };
                for (kind, result, ty) in [
                    (
                        OperationKind::SliceData { slice: load.input },
                        15,
                        Ty::Pointer,
                    ),
                    (
                        OperationKind::GetElementPointer {
                            base: p(15)?,
                            offset: p(5)?,
                        },
                        16,
                        Ty::Pointer,
                    ),
                    (
                        OperationKind::Load {
                            pointer: p(16)?,
                            access: MemoryAccess::new(AddressSpace::Global, 1),
                        },
                        17,
                        Ty::U32,
                    ),
                ] {
                    self.operation(
                        &mut read_at,
                        kind,
                        &[(p(result)?, ty)],
                        Some(j),
                        Stage::Read,
                        true,
                    )?;
                }
                self.read_branch(read_at, join_id, [p(17)?, v(6)?], j)?;
                let value = p(18)?;
                let mask = p(19)?;
                let actual = self.block(at.function, join)?;
                require(actual.id == join_id && actual.parameters.len() == 2)?;
                require(
                    actual.parameters[0].id == value
                        && actual.parameters[0].ty == Type::Scalar(ScalarType::U32),
                )?;
                require(actual.parameters[1].id == mask && actual.parameters[1].ty == Type::BOOL)?;
                self.piece(
                    Piece::Block {
                        function: at.function,
                        block: join,
                    },
                    Some(j),
                    Stage::Join,
                )?;
                for parameter in 0..2 {
                    self.piece(
                        Piece::BlockParameter {
                            function: at.function,
                            block: join,
                            parameter,
                        },
                        Some(j),
                        Stage::Join,
                    )?;
                }
                *at = TileScalarPointV29 {
                    function: at.function,
                    block: join,
                    operation: 0,
                };
            }
            Ok(())
        }
        fn parts(&mut self, load: Load, source: &Operation, at: &mut TileScalarPointV29) -> R<()> {
            let elements = usize::from(load.elements);
            require(source.results.len() == argument_product_v1(elements, 2)?)?;
            for (ordinal, result) in source.results.iter().enumerate() {
                let j = u32::try_from(ordinal % elements)
                    .map_err(|_| ArgumentResourceV1::Arithmetic)?;
                let offset = j
                    .checked_mul(20)
                    .and_then(|n| n.checked_add(8))
                    .and_then(|n| n.checked_add(if ordinal < elements { 18 } else { 19 }))
                    .ok_or(ArgumentResourceV1::Arithmetic)?;
                let value = load.value(offset)?;
                let ty = if ordinal < elements {
                    Ty::U32
                } else {
                    Ty::Bool
                };
                require(ty.matches(&result.ty))?;
                self.operation(
                    at,
                    OperationKind::Select {
                        condition: load.value(6)?,
                        true_value: value,
                        false_value: value,
                    },
                    &[(result.id, ty)],
                    Some(j),
                    Stage::Parts,
                    false,
                )?;
            }
            Ok(())
        }
    }

    pub(super) fn replay_tile_scalar_graph_v29(
        input: &PreparedScopedTileSourceV29,
        output: &fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV18,
        relations: &TileScalarRelationsV29,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> R<()> {
        input.replay_inner_v29(budget)?;
        let original = &input.pending.inner.pending.graph;
        budget.charge_work(argument_product_v1(
            argument_sum_v1(&[
                original.canonical_bytes().len(),
                output.canonical_bytes().len(),
            ])?,
            3,
        )?)?;
        let graph = original.module();
        let Module {
            id,
            functions,
            kernels,
            required_capabilities,
            storage_layouts,
        } = graph;
        let out = output.module();
        require(
            (id, kernels, required_capabilities, storage_layouts)
                == (
                    &out.id,
                    &out.kernels,
                    &out.required_capabilities,
                    &out.storage_layouts,
                )
                && functions.len() == out.functions.len(),
        )?;
        let index =
            AssertGraphIndexV1::build_functions(functions, true, budget).map_err(assert_error)?;
        let loads = census(input, &index, budget)?;
        let mut checker = Checker {
            output: out,
            relations,
            budget,
            row: 0,
            piece: 0,
            end: 0,
        };
        let mut next_load = 0;
        for (function, (source, output)) in functions.iter().zip(&out.functions).enumerate() {
            checker.budget.charge_work(1)?;
            let Function {
                id,
                signature,
                role,
                body,
                required_capabilities,
            } = source;
            require(
                (id, signature, role, required_capabilities)
                    == (
                        &output.id,
                        &output.signature,
                        &output.role,
                        &output.required_capabilities,
                    ),
            )?;
            let (body, output_body) = match (body, &output.body) {
                (None, None) => continue,
                (Some(body), Some(output)) => (body, output),
                _ => return Err(ScopedTileFailureKindV29::ReplayMismatch),
            };
            let FunctionBody { parameters, blocks } = body;
            let mut load_end = next_load;
            while loads
                .get(load_end)
                .is_some_and(|load| load.point.function == function)
            {
                checker.budget.charge_work(1)?;
                load_end = argument_sum_v1(&[load_end, 1])?;
            }
            let expected_blocks = match loads.get(next_load..load_end).and_then(|rows| rows.last())
            {
                Some(load) => argument_sum_v1(&[
                    load.suffix,
                    argument_product_v1(usize::from(load.elements), 2)?,
                ])?,
                None => blocks.len(),
            };
            require(
                parameters == &output_body.parameters
                    && output_body.blocks.len() == expected_blocks,
            )?;
            for parameter in 0..parameters.len() {
                checker.preserved(
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
            for (block, source) in blocks.iter().enumerate() {
                let BasicBlock {
                    id,
                    parameters,
                    operations,
                    terminator,
                } = source;
                let actual = checker.block(function, block)?;
                require(*id == actual.id && *parameters == actual.parameters)?;
                checker.preserved(
                    Source::Block { function, block },
                    Piece::Block { function, block },
                )?;
                for parameter in 0..parameters.len() {
                    checker.preserved(
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
                let mut at = TileScalarPointV29 {
                    function,
                    block,
                    operation: 0,
                };
                for (operation, source) in operations.iter().enumerate() {
                    checker.budget.charge_work(1)?;
                    let point = TileScalarPointV29 {
                        function,
                        block,
                        operation,
                    };
                    let before = at;
                    let (erased, parts) = match &source.kind {
                        OperationKind::Execution(execution) => match execution {
                            Execution::MaskedTileLoadU32 { .. } => {
                                let load = *loads
                                    .get(next_load)
                                    .ok_or(ScopedTileFailureKindV29::ReplayMismatch)?;
                                require(load.point == point)?;
                                checker.row(
                                    Source::Operation(point),
                                    argument_sum_v1(&[
                                        16,
                                        argument_product_v1(usize::from(load.elements), 42)?,
                                    ])?,
                                )?;
                                checker.load(load, &mut at)?;
                                next_load = argument_sum_v1(&[next_load, 1])?;
                                (true, None)
                            }
                            Execution::FragmentIntoPartsU32 {
                                fragment,
                                lanes,
                                elements,
                            } => {
                                let load = fragment_load(
                                    graph,
                                    &index,
                                    &loads,
                                    function,
                                    *fragment,
                                    checker.budget,
                                )?;
                                require((load.lanes, load.elements) == (*lanes, *elements))?;
                                checker.row(
                                    Source::Operation(point),
                                    argument_product_v1(usize::from(*elements), 2)?,
                                )?;
                                checker.parts(load, source, &mut at)?;
                                (false, Some(usize::from(*elements)))
                            }
                            Execution::ContextIssue
                            | Execution::WorkgroupDerive { .. }
                            | Execution::TileIntoFragmentU32 { .. }
                            | Execution::ScopeEnd { .. } => {
                                checker.row(Source::Operation(point), 1)?;
                                checker.piece(Piece::Anchor(at), None, Stage::Erased)?;
                                (true, None)
                            }
                        },
                        _ => {
                            let actual = checker
                                .block(at.function, at.block)?
                                .operations
                                .get(at.operation)
                                .ok_or(ScopedTileFailureKindV29::ReplayMismatch)?;
                            require(actual == source)?;
                            checker.preserved(Source::Operation(point), Piece::Operation(at))?;
                            at.operation = argument_sum_v1(&[at.operation, 1])?;
                            (false, None)
                        }
                    };
                    for (result, definition) in source.results.iter().enumerate() {
                        checker.row(
                            Source::Result {
                                operation: point,
                                result,
                            },
                            1,
                        )?;
                        let (piece, component, stage) = if erased {
                            (Piece::ErasedValue(definition.id), None, Stage::Erased)
                        } else if let Some(elements) = parts {
                            let operation = TileScalarPointV29 {
                                operation: argument_sum_v1(&[before.operation, result])?,
                                ..before
                            };
                            let component = u32::try_from(result % elements)
                                .map_err(|_| ArgumentResourceV1::Arithmetic)?;
                            (
                                Piece::Result {
                                    operation,
                                    result: 0,
                                },
                                Some(component),
                                Stage::Parts,
                            )
                        } else {
                            (
                                Piece::Result {
                                    operation: before,
                                    result,
                                },
                                None,
                                Stage::Preserved,
                            )
                        };
                        checker.piece(piece, component, stage)?;
                    }
                }
                checker.end_block(at)?;
                require(checker.block(function, at.block)?.terminator == *terminator)?;
                let term = terminator
                    .as_ref()
                    .ok_or(ScopedTileFailureKindV29::ReplayMismatch)?;
                checker.preserved(
                    Source::Terminator { function, block },
                    Piece::Terminator {
                        function,
                        block: at.block,
                    },
                )?;
                let mut edge = 0;
                term.try_visit_edges_v1(|_, arguments| {
                    checker
                        .budget
                        .charge_work(argument_sum_v1(&[1, arguments.len()])?)?;
                    checker.preserved(
                        Source::Edge {
                            function,
                            block,
                            edge,
                        },
                        Piece::Edge {
                            function,
                            block: at.block,
                            edge,
                        },
                    )?;
                    edge = argument_sum_v1(&[edge, 1])?;
                    Ok::<_, ScopedTileFailureKindV29>(())
                })?;
            }
            require(next_load == load_end)?;
        }
        require(
            next_load == loads.len()
                && checker.row == relations.origins.len()
                && checker.piece == checker.end
                && checker.piece == relations.pieces.len(),
        )?;
        assert_origin_drop_v1(loads, budget).map_err(assert_error)?;
        index.release(budget).map_err(assert_error)
    }
}
