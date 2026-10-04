use super::*;

#[test]
fn original_float_operators_have_exact_width_class_and_closed_distinct_codes() {
    use SemanticBinaryOpV1 as B;
    let operations = [
        (B::Add, 11),
        (B::Subtract, 12),
        (B::Multiply, 13),
        (B::Divide, 14),
        (B::Remainder, 15),
        (B::Equal, 16),
        (B::NotEqual, 17),
        (B::LessThan, 18),
        (B::LessOrEqual, 19),
        (B::GreaterThan, 20),
        (B::GreaterOrEqual, 21),
    ];
    for (operation, code) in operations {
        let operator = Operator::float_binary(operation).unwrap();
        assert_eq!(operator.code(), code);
        for width in [32, 64] {
            let input = ScalarV30::Float { width };
            assert_eq!(
                operator.result(input).unwrap(),
                if code >= 16 { ScalarV30::Bool } else { input }
            );
        }
        for input in [
            ScalarV30::Unit,
            ScalarV30::Bool,
            ScalarV30::Integer {
                width: 32,
                signed: false,
            },
            ScalarV30::Integer {
                width: 64,
                signed: true,
            },
            ScalarV30::Float { width: 16 },
            ScalarV30::Float { width: 128 },
        ] {
            assert!(operator.result(input).is_err());
        }
    }
    for operation in [B::BitAnd, B::BitOr, B::BitXor, B::ShiftLeft, B::ShiftRight] {
        assert!(Operator::float_binary(operation).is_err());
    }
    for width in [32, 64] {
        let input = ScalarV30::Float { width };
        assert!(Operator::Not.result(input).is_err());
        assert!(Operator::Binary(OperatorV30::Equal).result(input).is_err());
        assert!(Operator::Float(22).result(input).is_err());
        assert!(Operator::Float(9).result(input).is_err());
        assert_eq!(Operator::Float(10).result(input).unwrap(), input);
    }
}

#[test]
fn floating_interpretation_is_preserved_and_not_an_integer_or_fast_math_axiom() {
    let runtime = include_str!("mixed_optimizer_float_values_v52.vrs");
    assert!(runtime.contains("(execution.ieee_operators)("));
    assert!(runtime.contains("operation, input_bits, output_bits, a, b, 0int"));
    assert!(runtime.contains("None => MemoryValueV30::Undefined"));
    assert!(runtime.contains("10 <= operation <= 21"));
    for forbidden in [
        "assume(",
        "external_body",
        "uninterp",
        "a + b",
        "a * b",
        "a - b",
    ] {
        assert!(
            !runtime.contains(forbidden),
            "unexpected numerical authority: {forbidden}"
        );
    }
    let source = include_str!("original_semantic_mir_source_scalar_operands_v48.vrs");
    let left = source
        .find("let left = invocation_source_byte_evaluate_v36")
        .unwrap();
    let right = source
        .find("let right = invocation_source_byte_evaluate_v36(left.source")
        .unwrap();
    let value = source[right..]
        .find("event, left.value, right.value")
        .unwrap()
        + right;
    assert!(left < right && right < value);
}
