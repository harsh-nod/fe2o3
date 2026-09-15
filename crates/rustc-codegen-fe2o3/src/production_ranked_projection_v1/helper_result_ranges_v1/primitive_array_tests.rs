use super::super::{Engine, Scalar, State, Value, WorkingCells};
use super::*;

const ARRAY: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(7);

fn array_types(
    element: SemanticTypeIdV1,
    length: u64,
    stride: u64,
    count: u64,
) -> Vec<SemanticTypeDeclV1> {
    let mut types = types();
    types[U32.index() as usize] = SemanticTypeDeclV1::new(
        types[0].identity(),
        types[0].layout_identity(),
        SemanticTypeLayoutV1::new(Some(4), 4).unwrap(),
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
            signed: false,
            bits: 32,
        }),
    );
    types.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([70; 32]),
        SemanticLayoutIdentityV1::from_sha256([71; 32]),
        SemanticTypeLayoutV1::with_exact_rustc_layout(
            stride * count,
            4,
            SemanticFieldsShapeV1::array(stride, count),
            SemanticRustcVariantsV1::Single { index: 0 },
            SemanticBackendReprV1::memory(true),
            None,
            false,
            None,
            4,
            0,
            SemanticTypeLayoutDetailsV1::None,
        )
        .unwrap(),
        SemanticTypeShapeV1::Array { element, length },
    ));
    types
}

fn array(values: &[u32]) -> SemanticOperandV1 {
    SemanticOperandV1::Constant(SemanticConstantV1::new(
        ARRAY,
        SemanticConstantValueV1::Bytes(
            SemanticConstantBytesV1::new(values.iter().flat_map(|v| v.to_le_bytes()).collect())
                .unwrap(),
        ),
    ))
}

fn indexed(local: u32, kind: SemanticProjectionKindV1, ty: SemanticTypeIdV1) -> SemanticOperandV1 {
    SemanticOperandV1::Copy(
        SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(local),
            vec![SemanticProjectionV1::new(kind, ty).unwrap()],
            ty,
        )
        .unwrap(),
    )
}

fn rebuild(
    template: &SemanticFunctionDeclV1,
    locals: Vec<SemanticLocalDeclV1>,
    blocks: Vec<SemanticBasicBlockV1>,
) -> SemanticFunctionDeclV1 {
    SemanticFunctionDeclV1::new(
        template.identity(),
        template.role(),
        template.item_definition_identity(),
        template.monomorphization_identity(),
        template.generic_type_arguments_identity(),
        template.const_generic_arguments_identity(),
        template.source(),
        template.abi().clone(),
        locals,
        template.entry(),
        blocks,
    )
    .unwrap()
}

fn append_array_locals(template: &SemanticFunctionDeclV1) -> Vec<SemanticLocalDeclV1> {
    let mut locals = template.locals().to_vec();
    for (index, ty) in [(14, ARRAY), (15, U64), (16, ARRAY)] {
        locals.push(SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1::from_sha256([index + 70; 32]),
            ty,
            SemanticLocalRoleV1::Temporary,
            SemanticSourceProvenanceV1::unavailable(),
        ));
    }
    locals
}

#[test]
fn helper_result_array_constants_require_exact_primitive_layout_and_bytes() {
    let function = fixture(Mutation::None);
    for (element, length, stride, count, bytes, accepted) in [
        (U32, 3, 4, 3, vec![4, 1, 1], true),
        (I64, 3, 4, 3, vec![4, 1, 1], false),
        (BOOL, 3, 4, 3, vec![4, 1, 1], false),
        (U64, 3, 4, 3, vec![4, 1, 1], false),
        (PTR, 3, 4, 3, vec![4, 1, 1], false),
        (U32, 3, 8, 3, vec![4, 1, 1], false),
        (U32, 3, 4, 2, vec![4, 1, 1], false),
        (U32, 3, 4, 3, vec![4, 1], false),
        (U32, 3, 4, 3, vec![4, 1, 1, 1], false),
        (U32, 9, 4, 9, vec![4; 9], false),
    ] {
        let types = array_types(element, length, stride, count);
        let mut work = 0;
        let mut cells = WorkingCells::new(1000);
        let engine = Engine {
            types: &types,
            function: &function,
            escaped: vec![false; function.locals().len()],
            work: &mut work,
            cells: &mut cells,
        };
        let value = engine.operand(&State::new(), &array(&bytes));
        assert_eq!(
            value != Value::Unknown,
            accepted,
            "{element:?}/{length}/{stride}/{count}"
        );
        if accepted {
            assert_eq!(
                value,
                Value::Fields(vec![Value::exact(4), Value::exact(1), Value::exact(1)])
            );
        }
    }
}

#[test]
fn helper_result_array_index_is_exact_live_typed_and_in_bounds() {
    let types = array_types(U32, 3, 4, 3);
    let template = fixture(Mutation::None);
    let function = rebuild(
        &template,
        append_array_locals(&template),
        template.blocks().to_vec(),
    );
    let mut work = 0;
    let mut cells = WorkingCells::new(1000);
    let mut engine = Engine {
        types: &types,
        function: &function,
        escaped: vec![false; function.locals().len()],
        work: &mut work,
        cells: &mut cells,
    };
    let mut state = State::from([(
        14,
        Value::Fields(vec![Value::exact(4), Value::exact(256), Value::exact(1)]),
    )]);
    let index = indexed(
        14,
        SemanticProjectionKindV1::Index(SemanticLocalIdV1::from_index(15)),
        U32,
    );
    for (value, expected) in [
        (Value::exact(0), Some(4)),
        (Value::exact(1), Some(256)),
        (Value::exact(3), None),
        (Value::Unknown, None),
        (
            Value::Scalar(
                Scalar {
                    lo: 0,
                    hi: 1,
                    stamp: None,
                },
                None,
            ),
            None,
        ),
    ] {
        state.insert(15, value);
        assert_eq!(
            engine.operand(&state, &index).scalar().map(|s| s.lo),
            expected
        );
    }
    state.insert(15, Value::exact(0));
    engine.escaped[15] = true;
    assert_eq!(engine.operand(&state, &index), Value::Unknown);
    engine.escaped[15] = false;
    for (kind, ty, expected) in [
        (
            SemanticProjectionKindV1::ConstantIndex {
                offset: 1,
                minimum_length: 3,
                from_end: false,
            },
            U32,
            Some(256),
        ),
        (
            SemanticProjectionKindV1::ConstantIndex {
                offset: 1,
                minimum_length: 3,
                from_end: true,
            },
            U32,
            Some(1),
        ),
        (
            SemanticProjectionKindV1::ConstantIndex {
                offset: 0,
                minimum_length: 3,
                from_end: true,
            },
            U32,
            None,
        ),
        (
            SemanticProjectionKindV1::ConstantIndex {
                offset: 0,
                minimum_length: 4,
                from_end: false,
            },
            U32,
            None,
        ),
        (SemanticProjectionKindV1::Field(0), U32, None),
        (
            SemanticProjectionKindV1::Index(SemanticLocalIdV1::from_index(15)),
            U64,
            None,
        ),
        (
            SemanticProjectionKindV1::Index(SemanticLocalIdV1::from_index(3)),
            U32,
            None,
        ),
        (
            SemanticProjectionKindV1::Index(SemanticLocalIdV1::from_index(10)),
            U32,
            None,
        ),
    ] {
        state.insert(3, Value::exact(0));
        state.insert(10, Value::exact(0));
        assert_eq!(
            engine
                .operand(&state, &indexed(14, kind, ty))
                .scalar()
                .map(|s| s.lo),
            expected
        );
    }
    engine.escaped[14] = true;
    assert_eq!(engine.operand(&state, &index), Value::Unknown);
}

fn array_helper(mutation: u8) -> SemanticFunctionDeclV1 {
    let template = fixture(Mutation::None);
    let mut blocks = template.blocks().to_vec();
    let index = || {
        indexed(
            14,
            SemanticProjectionKindV1::Index(SemanticLocalIdV1::from_index(15)),
            U32,
        )
    };
    let mut entry = blocks[0].statements().to_vec();
    entry[0] = assign(2, U32, SemanticRvalueKindV1::Use(constant(U32, 16)));
    entry.insert(
        0,
        assign(15, U64, SemanticRvalueKindV1::Use(constant(U64, 0))),
    );
    entry.insert(
        0,
        assign(14, ARRAY, SemanticRvalueKindV1::Use(array(&[4, 1, 1]))),
    );
    blocks[0] = block(0, entry, blocks[0].terminator().kind().clone());
    let mut compare = Vec::new();
    match mutation {
        1 => compare.push(assign(
            14,
            ARRAY,
            SemanticRvalueKindV1::Use(array(&[u32::MAX, 1, 1])),
        )),
        2 => compare.push(assign(15, U64, SemanticRvalueKindV1::Use(constant(U64, 3)))),
        3 => compare.push(statement(SemanticStatementKindV1::StorageDead(
            SemanticLocalIdV1::from_index(14),
        ))),
        4 => compare.push(assign(
            16,
            ARRAY,
            SemanticRvalueKindV1::Use(moved(14, ARRAY)),
        )),
        _ => {}
    }
    compare.push(assign(
        3,
        BOOL,
        SemanticRvalueKindV1::Binary {
            operation: SemanticBinaryOpV1::GreaterThan,
            left: copy(1, U32),
            right: index(),
        },
    ));
    blocks[1] = block(
        1,
        compare,
        if mutation == 5 {
            goto(2)
        } else {
            blocks[1].terminator().kind().clone()
        },
    );
    let mut math = blocks[2].statements().to_vec();
    math[0] = assign(
        4,
        U32,
        SemanticRvalueKindV1::Binary {
            operation: SemanticBinaryOpV1::Divide,
            left: index(),
            right: copy(2, U32),
        },
    );
    math.insert(
        0,
        assign(15, U64, SemanticRvalueKindV1::Use(constant(U64, 0))),
    );
    math.insert(
        0,
        assign(14, ARRAY, SemanticRvalueKindV1::Use(array(&[256, 1, 1]))),
    );
    blocks[2] = block(2, math, goto(3));
    let checked = |a, b| {
        SemanticRvalueKindV1::CheckedBinary(SemanticCheckedBinaryRvalueV1::new(
            SemanticCheckedBinaryOpV1::Multiply,
            a,
            b,
        ))
    };
    let assert = |result, left, factor, target| SemanticTerminatorKindV1::Assert {
        condition: field(result, 1, BOOL),
        expected: false,
        message: SemanticAssertMessageV1::Overflow {
            operation: SemanticBinaryOpV1::Multiply,
            left,
            right: constant(U64, factor),
        },
        target: edge(SemanticEdgeRoleV1::AssertSuccess, target),
        unwind: SemanticUnwindActionV1::Unreachable,
    };
    let mut first = blocks[8].statements().to_vec();
    first[1] = assign(12, PAIR, checked(copy(11, U64), constant(U64, 4)));
    blocks[8] = block(8, first, assert(12, copy(11, U64), 4, 10));
    blocks.push(block(
        10,
        vec![
            assign(5, U64, SemanticRvalueKindV1::Use(field(12, 0, U64))),
            assign(7, PAIR, checked(copy(5, U64), constant(U64, 16))),
        ],
        assert(7, moved(5, U64), 16, 9),
    ));
    if mutation == 6 {
        blocks[9] = block(9, vec![], goto(1));
    }
    rebuild(&template, append_array_locals(&template), blocks)
}

#[test]
fn helper_result_array_guard_propagates_through_checked_products_without_assert_assumptions() {
    let types = array_types(U32, 3, 4, 3);
    for mutation in 0..=6 {
        let function = array_helper(mutation);
        let SemanticStatementKindV1::Assign(assignment) =
            function.blocks()[10].statements()[1].kind()
        else {
            panic!()
        };
        let SemanticRvalueKindV1::CheckedBinary(checked) = assignment.value().kind() else {
            panic!()
        };
        let proof = HelperResultRangesV1::analyze(&types, &function, &mut 0).unwrap();
        let range = proof.at(&types, &function, checked.left(), 10, 1);
        assert_eq!(
            range.map(|r| (r.minimum, r.maximum)) == Some((64, 256)),
            mutation == 0,
            "mutation {mutation}: {range:?}"
        );
    }
}

#[test]
fn helper_result_array_analysis_uses_shared_work_and_peak_storage_owner() {
    let types = array_types(U32, 3, 4, 3);
    let function = array_helper(0);
    let mut work = MAX_PROJECTED_LOOP_GRAPH_WORK_V1;
    assert!(matches!(
        HelperResultRangesV1::analyze(&types, &function, &mut work),
        Err(ProductionRankedProjectionErrorV1::Unsupported(
            "uniform induction CFG analysis exceeds its work limit"
        ))
    ));
    let mut cells = WorkingCells::new(super::super::MAX_CELLS);
    let mut work = 0;
    let facts =
        HelperResultRangesV1::analyze_with_cells(&types, &function, &mut work, &mut cells).unwrap();
    assert!(work > 0);
    assert_eq!(cells.live, facts.operands.retained_cells().unwrap());
    let mut exact = WorkingCells::new(cells.peak);
    HelperResultRangesV1::analyze_with_cells(&types, &function, &mut 0, &mut exact).unwrap();
    let mut short = WorkingCells::new(cells.peak - 1);
    assert!(matches!(
        HelperResultRangesV1::analyze_with_cells(&types, &function, &mut 0, &mut short),
        Err(ProductionRankedProjectionErrorV1::Unsupported(
            super::super::STORAGE_LIMIT
        ))
    ));
}

#[test]
fn helper_result_array_copy_move_and_projected_overwrite_preserve_no_stale_source() {
    let types = array_types(U32, 3, 4, 3);
    let template = fixture(Mutation::None);
    let function = rebuild(
        &template,
        append_array_locals(&template),
        template.blocks().to_vec(),
    );
    let mut work = 0;
    let mut cells = WorkingCells::new(super::super::MAX_CELLS);
    let mut state = State::new();
    cells
        .reserve(super::super::SCRATCH_CELLS + super::super::state_nodes(&state))
        .unwrap();
    let mut engine = Engine {
        types: &types,
        function: &function,
        escaped: vec![false; function.locals().len()],
        work: &mut work,
        cells: &mut cells,
    };
    let mut facts = HelperResultRangesV1 {
        types: &types,
        function: &function,
        operands: super::super::retained_facts_v1::FactBuilderV1::new(engine.cells, engine.work)
            .unwrap(),
        retained: None,
    };
    let statements = [
        assign(14, ARRAY, SemanticRvalueKindV1::Use(array(&[4, 256, 1]))),
        assign(16, ARRAY, SemanticRvalueKindV1::Use(copy(14, ARRAY))),
        assign(14, ARRAY, SemanticRvalueKindV1::Use(array(&[7, 256, 1]))),
        assign(15, U64, SemanticRvalueKindV1::Use(constant(U64, 0))),
    ];
    for (i, statement) in statements.iter().enumerate() {
        engine
            .statement(&mut state, statement.kind(), (0, i), &mut facts)
            .unwrap();
    }
    let projection = SemanticProjectionKindV1::Index(SemanticLocalIdV1::from_index(15));
    assert_eq!(
        engine
            .operand(&state, &indexed(14, projection, U32))
            .scalar()
            .unwrap()
            .lo,
        7
    );
    assert_eq!(
        engine
            .operand(&state, &indexed(16, projection, U32))
            .scalar()
            .unwrap()
            .lo,
        4
    );
    let moved = assign(14, ARRAY, SemanticRvalueKindV1::Use(moved(16, ARRAY)));
    engine
        .statement(&mut state, moved.kind(), (0, 4), &mut facts)
        .unwrap();
    assert_eq!(
        engine
            .operand(&state, &indexed(14, projection, U32))
            .scalar()
            .unwrap()
            .lo,
        4
    );
    assert_eq!(
        engine.operand(&state, &indexed(16, projection, U32)),
        Value::Unknown
    );
    let SemanticOperandV1::Copy(destination) = indexed(14, projection, U32) else {
        panic!()
    };
    let overwrite = statement(SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
        destination,
        SemanticRvalueV1::new(U32, SemanticRvalueKindV1::Use(constant(U32, 8))),
    )));
    engine
        .statement(&mut state, overwrite.kind(), (0, 5), &mut facts)
        .unwrap();
    assert_eq!(
        engine.operand(&state, &indexed(14, projection, U32)),
        Value::Unknown
    );
    assert_eq!(
        engine.cells.live,
        super::super::SCRATCH_CELLS
            + super::super::state_nodes(&state)
            + facts.operands.retained_cells().unwrap()
    );
}
