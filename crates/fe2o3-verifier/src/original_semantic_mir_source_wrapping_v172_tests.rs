use super::*;
use fe2o3_mir_model::semantic_mir_v1::*;

const LIMIT: usize = 512 * 1024 * 1024;

#[derive(Clone, Copy)]
struct Case {
    width: u32,
    signed: bool,
    operator: Operator,
    source: SemanticBinaryOpV1,
    left: u128,
    right: u128,
}

fn cases() -> Vec<Case> {
    let mut cases = Vec::new();
    for width in [8, 16, 32, 64] {
        let maximum = (1u128 << width) - 1;
        for signed in [false, true] {
            for (operator, source, left, right) in [
                (Operator::WrappingAdd, SemanticBinaryOpV1::Add, maximum, 1),
                (
                    Operator::WrappingSubtract,
                    SemanticBinaryOpV1::Subtract,
                    0,
                    1,
                ),
                (
                    Operator::WrappingMultiply,
                    SemanticBinaryOpV1::Multiply,
                    maximum,
                    2,
                ),
            ] {
                cases.push(Case {
                    width,
                    signed,
                    operator,
                    source,
                    left,
                    right,
                });
            }
            if signed {
                cases.push(Case {
                    width,
                    signed,
                    operator: Operator::WrappingAdd,
                    source: SemanticBinaryOpV1::Add,
                    left: (1u128 << (width - 1)) - 1,
                    right: 1,
                });
                cases.push(Case {
                    width,
                    signed,
                    operator: Operator::WrappingMultiply,
                    source: SemanticBinaryOpV1::Multiply,
                    left: 1u128 << (width - 1),
                    right: maximum,
                });
            }
        }
    }
    cases
}

fn program(work: usize, storage: usize) -> (Result<()>, usize, usize, usize) {
    let expected = cases();
    let input = expected.clone();
    super::super::super::super::invocations::tests::run_source_transform(
        work,
        storage,
        move |types, functions| {
            let function = functions.last_mut().unwrap();
            let mut locals = function.locals().to_vec();
            let mut blocks = function.blocks().to_vec();
            let mut statements = blocks[0].statements().to_vec();
            for (index, case) in input.iter().enumerate() {
                let tag = [160 + index as u8; 32];
                let ty = TypeId::from_index(types.len() as u32);
                let bytes = u64::from(case.width / 8);
                types.push(SemanticTypeDeclV1::new(
                    SemanticTypeIdentityV1::from_sha256(tag),
                    SemanticLayoutIdentityV1::from_sha256(tag),
                    SemanticTypeLayoutV1::new_with_backend_repr(
                        Some(bytes),
                        bytes,
                        BackendRepr::scalar(BackendScalar::initialized(
                            BackendPrimitive::integer(case.signed, case.width as u16, bytes),
                            SemanticScalarValidityRangeV1::new(0, (1u128 << case.width) - 1),
                        )),
                        false,
                    )
                    .unwrap(),
                    Shape::Scalar(SemanticScalarTypeV1::Integer {
                        bits: case.width as u16,
                        signed: case.signed,
                    }),
                ));
                let local = SemanticLocalIdV1::from_index(locals.len() as u32);
                locals.push(SemanticLocalDeclV1::new(
                    SemanticLocalIdentityV1::from_sha256(tag),
                    ty,
                    SemanticLocalRoleV1::Temporary,
                    function.source(),
                ));
                let operand = |value| {
                    Operand::Constant(SemanticConstantV1::new(
                        ty,
                        SemanticConstantValueV1::Scalar(
                            SemanticScalarValueV1::new(value, bytes as u8).unwrap(),
                        ),
                    ))
                };
                statements.push(SemanticStatementV1::new(
                    function.source(),
                    Statement::Assign(SemanticAssignmentV1::new(
                        Place::new(local, vec![], ty).unwrap(),
                        SemanticRvalueV1::new(
                            ty,
                            Rvalue::Binary {
                                operation: case.source,
                                left: operand(case.left),
                                right: operand(case.right),
                            },
                        ),
                    )),
                ));
            }
            blocks[0] = SemanticBasicBlockV1::new(
                blocks[0].identity(),
                blocks[0].source(),
                statements,
                blocks[0].terminator().clone(),
            )
            .unwrap();
            *function = SemanticFunctionDeclV1::new(
                function.identity(),
                function.role(),
                function.item_definition_identity(),
                function.monomorphization_identity(),
                function.generic_type_arguments_identity(),
                function.const_generic_arguments_identity(),
                function.source(),
                function.abi().clone(),
                locals,
                function.entry(),
                blocks,
            )
            .unwrap();
        },
        |plan, out| {
            super::super::super::source_function::tests::with_slots(plan, out, |slots, out| {
                for root in 0..2 {
                    for instance in 1..=2 {
                        let body = SourceByteBody::derive(plan, slots, root, instance, out)?;
                        let operations: Vec<_> = body
                            .events
                            .iter()
                            .filter_map(|event| match event {
                                Event::ScalarOperands(operation)
                                    if operation.operator.wrapping() =>
                                {
                                    Some(operation)
                                }
                                _ => None,
                            })
                            .collect();
                        assert_eq!(operations.len(), expected.len());
                        for (operation, case) in operations.iter().zip(&expected) {
                            assert_eq!(operation.operator, case.operator);
                            assert_eq!(
                                operation.input,
                                ScalarV30::Integer {
                                    width: case.width,
                                    signed: case.signed
                                }
                            );
                            assert_eq!(operation.output, operation.input);
                            assert_eq!(operation.left, Value::Constant(case.left));
                            assert_eq!(operation.right, Some(Value::Constant(case.right)));
                            assert!(matches!(operation.destination, Destination::Local(_)));
                        }
                    }
                }
                let mut generated =
                    super::super::super::source_function::SourceByteProgram::derive(
                        plan, slots, out,
                    )?;
                generated.emit(out)?;
                for code in [22, 23, 24] {
                    assert!(out.text.contains(&format!("operation: {code}int")));
                }
                Ok(())
            })
        },
    )
}

#[test]
fn original_wrapping_all_widths_signedness_and_boundary_operands_use_ordered_events() {
    let result = program(LIMIT, LIMIT);
    result.0.unwrap();
    assert_eq!(result.2, 37);
}

#[test]
fn original_wrapping_boundary_oracle_matches_modular_equations() {
    macro_rules! expected {
        ($case:expr, $ty:ty) => {{
            let left = $case.left as $ty;
            let right = $case.right as $ty;
            (match $case.operator {
                Operator::WrappingAdd => left.wrapping_add(right),
                Operator::WrappingSubtract => left.wrapping_sub(right),
                Operator::WrappingMultiply => left.wrapping_mul(right),
                _ => unreachable!(),
            }) as u128
        }};
    }
    // This is a differential equation oracle, not execution of the Verus model.
    for case in cases() {
        let modulus = 1u128 << case.width;
        let actual = match case.operator {
            Operator::WrappingAdd => (case.left + case.right) % modulus,
            Operator::WrappingSubtract => (case.left + modulus - case.right) % modulus,
            Operator::WrappingMultiply => (case.left * case.right) % modulus,
            _ => unreachable!(),
        };
        let expected = match (case.width, case.signed) {
            (8, false) => expected!(case, u8),
            (8, true) => expected!(case, i8),
            (16, false) => expected!(case, u16),
            (16, true) => expected!(case, i16),
            (32, false) => expected!(case, u32),
            (32, true) => expected!(case, i32),
            (64, false) => expected!(case, u64),
            (64, true) => expected!(case, i64),
            _ => unreachable!(),
        } & (modulus - 1);
        assert_eq!(actual, expected);
    }
}

#[test]
fn original_wrapping_generation_has_exact_and_one_short_resource_limits() {
    let measured = program(LIMIT, LIMIT);
    measured.0.unwrap();
    let exact = program(measured.1, measured.3);
    exact.0.unwrap();
    assert_eq!(
        (exact.1, exact.2, exact.3),
        (measured.1, measured.2, measured.3)
    );
    for (work, storage, work_limit) in [
        (measured.1 - 1, measured.3, true),
        (measured.1, measured.3 - 1, false),
    ] {
        let result = program(work, storage);
        let error = result.0.unwrap_err();
        let mut cause: &(dyn std::error::Error + 'static) = &error;
        let resource = loop {
            if let Some(resource) = cause.downcast_ref::<Resource>() {
                break resource;
            }
            cause = cause.source().expect("typed resource cause");
        };
        match resource {
            Resource::Work(value) if work_limit => {
                assert_eq!((value.limit(), value.actual()), (work, measured.1))
            }
            Resource::Storage(value) if !work_limit => {
                assert_eq!((value.limit(), value.actual()), (storage, measured.3))
            }
            other => panic!("unexpected resource {other:?}"),
        }
        assert_eq!(result.2, measured.2);
    }
}

#[test]
fn original_wrapping_operator_codes_are_distinct_from_float_and_partial_arithmetic() {
    for (operator, code) in [
        (Operator::WrappingAdd, 22),
        (Operator::WrappingSubtract, 23),
        (Operator::WrappingMultiply, 24),
    ] {
        assert_eq!(operator.code(), code);
        for width in [8, 16, 32, 64] {
            for signed in [false, true] {
                let input = ScalarV30::Integer { width, signed };
                assert_eq!(operator.result(input).unwrap(), input);
            }
        }
        for input in [
            ScalarV30::Unit,
            ScalarV30::Bool,
            ScalarV30::Float { width: 32 },
            ScalarV30::Integer {
                width: 128,
                signed: false,
            },
        ] {
            assert!(operator.result(input).is_err());
        }
    }
    let model = include_str!("original_semantic_mir_source_scalar_operands_v48.vrs");
    for equation in [
        "(left + right) % modulus",
        "(left + modulus - right) % modulus",
        "(left * right) % modulus",
    ] {
        assert_eq!(model.matches(equation).count(), 1);
    }
    assert!(model.contains("if 22 <= event.operation <= 24"));
    assert!(model.contains("10 <= event.operation < 22"));
    assert!(model.contains("event.output_bits == event.input_bits && event.right.is_some()"));
    for forbidden in ["assume(", "admit(", "external_body"] {
        assert!(!model.contains(forbidden));
    }
}
