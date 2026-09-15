// Production hook tests: the read and its bounds predicate must share one value.
fn derived_read_index_fixture_v1(
    divisor: u64,
    stride: u64,
    mask: u64,
    stale: bool,
) -> (
    Vec<SemanticTypeDeclV1>,
    Vec<SemanticCallableDeclV1>,
    SemanticFunctionDeclV1,
) {
    let (types, callables, function) = typed_global_projection_fixture_v1();
    let mut locals = function.locals().to_vec();
    let first = locals.len() as u32;
    for index in 0..4 {
        locals.push(local(230 + index, U64_TYPE, SemanticLocalRoleV1::Temporary));
    }
    let mut blocks = function.blocks().to_vec();
    let original = &blocks[6];
    let SemanticTerminatorKindV1::Call(call) = original.terminator().kind() else {
        panic!()
    };
    let mut statements = original.statements().to_vec();
    for (offset, operation, left, right) in [
        (
            0,
            SemanticBinaryOpV1::Divide,
            typed_operand(6, U64_TYPE),
            typed_constant(U64_TYPE, divisor.into(), 8),
        ),
        (
            1,
            SemanticBinaryOpV1::Multiply,
            typed_operand(first, U64_TYPE),
            typed_constant(U64_TYPE, stride.into(), 8),
        ),
        (
            2,
            SemanticBinaryOpV1::BitAnd,
            typed_operand(6, U64_TYPE),
            typed_constant(U64_TYPE, mask.into(), 8),
        ),
        (
            3,
            SemanticBinaryOpV1::Add,
            typed_operand(first + 1, U64_TYPE),
            typed_operand(first + 2, U64_TYPE),
        ),
    ] {
        statements.push(typed_assignment(
            first + offset,
            U64_TYPE,
            SemanticRvalueKindV1::Binary {
                operation,
                left,
                right,
            },
        ));
    }
    if stale {
        statements.push(typed_assignment(
            first + 3,
            U64_TYPE,
            SemanticRvalueKindV1::Use(typed_operand(first + 2, U64_TYPE)),
        ));
    }
    let mut arguments = call.arguments().to_vec();
    arguments[1] = typed_operand(first + 3, U64_TYPE);
    blocks[6] = SemanticBasicBlockV1::new(
        original.identity(),
        original.source(),
        statements,
        SemanticTerminatorV1::new(
            original.terminator().source(),
            SemanticTerminatorKindV1::Call(
                SemanticDirectCallV1::new_callable(
                    call.callee(),
                    arguments,
                    call.destination().cloned(),
                    call.unwind(),
                )
                .unwrap(),
            ),
        ),
    )
    .unwrap();
    (
        types,
        callables,
        typed_global_fixture_with_body_v1(&function, locals, blocks),
    )
}

#[test]
fn derived_read_index_production_retains_exact_coordinate_guard_and_allocation() {
    for stride in [4, 16, 32] {
        let (types, callables, function) = derived_read_index_fixture_v1(64, stride, 63, false);
        let original = function.clone();
        let (projection, operations) =
            project_capability_index_fixture_with_launch(&types, &callables, &function, Some(1024))
                .unwrap();
        let read = projection.direct_read_effects[6].as_ref().unwrap();
        assert!(matches!(
            read.indices.as_slice(),
            [ProductionRankedValueV1::Local(_)]
        ));
        assert_eq!(read.comparisons.len(), 1);
        assert_eq!(read.comparisons[0].0, read.indices[0]);
        assert_eq!(
            projection.option_predicates[17],
            Some(GuardPredicateV1::for_access(read))
        );
        assert!(operations.iter().any(|op| matches!(op,
            ProductionRankedOperationV1::ViewInSpace {
                result, allocation_origin: 1, writable: false, dynamic_extents, ..
            } if *result == read.view && dynamic_extents == &[read.comparisons[0].1]
        )));
        for point in 0..1024 {
            assert_eq!(
                evaluate_projected_index(&operations, read.indices[0], point),
                (point / 64) * stride + (point & 63)
            );
        }
        assert_eq!(function, original);
    }
}

#[test]
fn derived_read_index_production_unknown_non_total_and_stale_keep_runtime_guard() {
    for (divisor, stride, mask, stale, launch) in [
        (0, 16, 63, false, Some(1024)),
        (64, u64::MAX, 63, false, Some(1024)),
        (64, 16, 10, false, Some(1024)),
        (64, 16, 63, true, Some(1024)),
        (64, 16, 63, false, None),
    ] {
        let (types, callables, function) =
            derived_read_index_fixture_v1(divisor, stride, mask, stale);
        let original = function.clone();
        let (projection, _) =
            project_capability_index_fixture_with_launch(&types, &callables, &function, launch)
                .unwrap();
        let read = projection.direct_read_effects[6].as_ref().unwrap();
        assert!(matches!(
            read.indices.as_slice(),
            [ProductionRankedValueV1::Argument(_)]
        ));
        assert_eq!(read.comparisons.len(), 1);
        assert_eq!(read.comparisons[0].0, read.indices[0]);
        assert_eq!(
            projection.option_predicates[17],
            Some(GuardPredicateV1::for_access(read))
        );
        assert_eq!(function, original);
    }
}

#[test]
fn derived_read_index_production_does_not_borrow_future_definition() {
    let (types, callables, function) = derived_read_index_fixture_v1(64, 16, 63, false);
    let mut blocks = function.blocks().to_vec();
    let mut read_statements = blocks[6].statements().to_vec();
    let future = read_statements.pop().unwrap();
    blocks[6] = block(236, read_statements, blocks[6].terminator().kind().clone());
    let mut later = blocks[7].statements().to_vec();
    later.push(future);
    blocks[7] = block(237, later, blocks[7].terminator().kind().clone());
    let function = typed_global_fixture_with_body_v1(&function, function.locals().to_vec(), blocks);
    let (projection, _) =
        project_capability_index_fixture_with_launch(&types, &callables, &function, Some(1024))
            .unwrap();
    let read = projection.direct_read_effects[6].as_ref().unwrap();
    assert!(matches!(
        read.indices.as_slice(),
        [ProductionRankedValueV1::Argument(_)]
    ));
    assert_eq!(read.comparisons[0].0, read.indices[0]);
}
