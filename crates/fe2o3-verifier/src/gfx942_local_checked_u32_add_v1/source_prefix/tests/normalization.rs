use super::*;

fn statement(kind: SemanticStatementKindV1) -> SemanticStatementV1 {
    SemanticStatementV1::new(SemanticSourceProvenanceV1::unavailable(), kind)
}

fn assignment(
    destination: SemanticPlaceV1,
    result_type: SemanticTypeIdV1,
    value: SemanticRvalueKindV1,
) -> SemanticStatementV1 {
    statement(SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
        destination,
        SemanticRvalueV1::new(result_type, value),
    )))
}

#[test]
fn normalization_reads_real_type_shapes_and_rejects_bounds_sign_and_width() {
    let owner = owner(vec![], 1, 1, 1, 5);
    let source = owner.semantic().semantic();
    for index in [0, 1, 2, 3, 4, u32::MAX] {
        assert_eq!(normalize::is_u32(source.types(), ty(index)), index == 1);
    }
    let original = &source.types()[1];
    for shape in [
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
            signed: true,
            bits: 32,
        }),
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
            signed: false,
            bits: 64,
        }),
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Float { bits: 32 }),
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Char),
        SemanticTypeShapeV1::Opaque,
    ] {
        let changed = SemanticTypeDeclV1::new(
            original.identity(),
            original.layout_identity(),
            original.layout().clone(),
            shape,
        );
        assert!(!normalize::is_u32(&[changed], ty(0)));
    }
}

#[test]
fn normalization_checks_actual_places_and_constants_without_receipts() {
    let owner = owner(vec![], 1, 1, 1, 5);
    let source = owner.semantic().semantic();
    let locals = source.functions()[0].locals();
    let projected = SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(1),
        vec![SemanticProjectionV1::new(SemanticProjectionKindV1::Field(0), ty(1)).unwrap()],
        ty(1),
    )
    .unwrap();
    for invalid in [
        projected,
        place(0, 1),
        place(1, 2),
        place(1, u32::MAX),
        place(u32::MAX, 1),
    ] {
        assert_eq!(
            normalize::scalar_local(source.types(), locals, &invalid),
            Err(CheckedU32PrefixErrorV1::Source)
        );
    }
    assert_eq!(
        normalize::scalar_local(source.types(), locals, &place(1, 1)),
        Ok(1)
    );
    for value in [0, 1, 0x8000_0000, u32::MAX] {
        assert_eq!(
            normalize::scalar_constant(source.types(), &constant(value)),
            Some(value)
        );
    }
    for invalid in [
        copy(1),
        SemanticOperandV1::Move(place(1, 1)),
        SemanticOperandV1::Constant(SemanticConstantV1::new(
            ty(1),
            SemanticConstantValueV1::ZeroSized,
        )),
        SemanticOperandV1::Constant(SemanticConstantV1::new(
            ty(2),
            SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(1, 4).unwrap()),
        )),
        SemanticOperandV1::Constant(SemanticConstantV1::new(
            ty(1),
            SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(1, 8).unwrap()),
        )),
    ] {
        assert_eq!(normalize::scalar_constant(source.types(), &invalid), None);
    }
    assert!(SemanticScalarValueV1::new(1_u128 << 32, 4).is_err());
}

#[test]
fn normalization_rejects_effects_and_exact_statement_operation_mismatches() {
    let owner = owner(vec![], 1, 1, 1, 5);
    let source = owner.semantic().semantic();
    let locals = source.functions()[0].locals();
    for (stmt, expected_count) in [
        (nop(), 0),
        (assign(2, copy(1)), 0),
        (assign(2, constant(17)), 1),
    ] {
        for count in [0, 1, 2, u32::MAX] {
            assert_eq!(
                normalize::source_step(source.types(), locals, &stmt, count).is_ok(),
                count == expected_count
            );
        }
    }
    for invalid in [
        assign(2, SemanticOperandV1::Move(place(1, 1))),
        assignment(place(2, 1), ty(2), SemanticRvalueKindV1::Use(copy(1))),
        assignment(
            place(2, 1),
            ty(1),
            SemanticRvalueKindV1::Length(place(1, 1)),
        ),
        statement(SemanticStatementKindV1::StorageLive(
            SemanticLocalIdV1::from_index(2),
        )),
        statement(SemanticStatementKindV1::StorageDead(
            SemanticLocalIdV1::from_index(2),
        )),
        statement(SemanticStatementKindV1::Deinitialize(place(2, 1))),
        statement(SemanticStatementKindV1::Assume(copy(1))),
    ] {
        for count in [0, 1, 2] {
            assert!(matches!(
                normalize::source_step(source.types(), locals, &invalid, count),
                Err(CheckedU32PrefixErrorV1::Source)
            ));
        }
    }
}

#[test]
fn normalization_composes_with_fold_for_self_copy_uninitialized_reads_and_all_cells() {
    let owner = owner(vec![], 1, 1, 1, 5);
    let source = owner.semantic().semantic();
    let locals = source.functions()[0].locals();
    for first in [
        Origin::Uninitialized,
        Origin::Constant(0),
        Origin::Constant(u32::MAX),
    ] {
        for second in [
            Origin::Uninitialized,
            Origin::Constant(7),
            Origin::Constant(u32::MAX),
        ] {
            for source_local in 1..4 {
                for destination in 1..4 {
                    let mut state = [
                        Origin::Uninitialized,
                        first,
                        second,
                        Origin::Constant(11),
                        Origin::Uninitialized,
                    ];
                    let before = state;
                    let input = before[source_local as usize];
                    let step = normalize::source_step(
                        source.types(),
                        locals,
                        &assign(destination, copy(source_local)),
                        0,
                    )
                    .unwrap()
                    .unwrap();
                    let initialized = !matches!(input, Origin::Uninitialized);
                    assert_eq!(fold::fold(&mut state, &[step]), initialized);
                    let mut expected = before;
                    if initialized {
                        expected[destination as usize] = input;
                    }
                    assert_eq!(state, expected);
                }
            }
        }
    }
}

#[test]
fn normalization_retains_distinct_owner_live_constants_instead_of_expected_rows() {
    for value in [0, 17, u32::MAX] {
        let owner = owner(
            vec![assign(1, constant(value)), assign(2, copy(1))],
            2,
            1,
            0,
            4,
        );
        let relation =
            check_captured_checked_u32_prefix_v1(owner.checked_u32_add_capture_v1().unwrap())
                .unwrap();
        assert!(std::ptr::eq(relation.capture().owner(), &owner));
        assert_eq!(
            relation.operand_origin(),
            CheckedU32PrefixOriginV1::Constant(value)
        );
        let result = relation.evaluate(&[]).unwrap();
        assert_eq!((result.value, result.scc), value.overflowing_add(1));
    }
}
