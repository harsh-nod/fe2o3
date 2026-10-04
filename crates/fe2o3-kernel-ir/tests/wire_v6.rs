use std::panic::{AssertUnwindSafe, catch_unwind};

use fe2o3_kernel_ir::*;

const CHECKED_ADD_I128_V6_GOLDEN_HEX: &str = include_str!("fixtures/checked_add_i128_v6.hex");
const LHS: ValueId = ValueId(0xa1b2_c3d4);
const RHS: ValueId = ValueId(0xe5f6_0718);

fn from_hex(text: &str) -> Vec<u8> {
    let compact = text
        .bytes()
        .filter(|byte| !byte.is_ascii_whitespace())
        .collect::<Vec<_>>();
    assert_eq!(compact.len() % 2, 0);
    compact
        .chunks_exact(2)
        .map(|pair| {
            let digit = |value: u8| match value {
                b'0'..=b'9' => value - b'0',
                b'a'..=b'f' => value - b'a' + 10,
                _ => panic!("invalid golden hex"),
            };
            (digit(pair[0]) << 4) | digit(pair[1])
        })
        .collect()
}

fn checked_fixture(operator: CheckedBinaryOperator, scalar: ScalarType) -> Module {
    let ty = Type::Scalar(scalar);
    let operation = Operation::checked_binary(
        ValueDef::new(ValueId(0x1020_3040), ty.clone()),
        ValueDef::new(ValueId(0x5060_7080), Type::BOOL),
        operator,
        LHS,
        RHS,
    );
    let mut block = BasicBlock::new(BlockId(0));
    block.operations.push(operation);
    block.terminator = Some(Terminator::Return { values: vec![] });

    let mut module = Module::new("checked-wire-v6");
    module.functions.push(Function::definition(
        "checked",
        Signature::new(vec![ty.clone(), ty], vec![]),
        vec![LHS, RHS],
        vec![block],
    ));
    module
}

fn operator_offset(bytes: &[u8], tag: u8) -> usize {
    let mut marker = vec![4, tag];
    marker.extend_from_slice(&LHS.0.to_le_bytes());
    marker.extend_from_slice(&RHS.0.to_le_bytes());
    let offsets = bytes
        .windows(marker.len())
        .enumerate()
        .filter_map(|(offset, window)| (window == marker).then_some(offset + 1))
        .collect::<Vec<_>>();
    assert_eq!(offsets.len(), 1, "checked binary marker must be unique");
    offsets[0]
}

#[test]
fn v6_is_deterministic_canonical_and_retains_every_checked_operator() {
    assert_eq!(KERNEL_IR_VERSION_V6, 6);
    assert_eq!(KERNEL_IR_DOMAIN_V6, b"FE2O3/KERNEL-IR/V6\0");
    let cases = [
        (CheckedBinaryOperator::Add, 11),
        (CheckedBinaryOperator::Subtract, 12),
        (CheckedBinaryOperator::Multiply, 13),
    ];
    let mut encodings = Vec::new();
    for (operator, tag) in cases {
        let module = checked_fixture(operator, ScalarType::I128);
        verify_module(&module).unwrap();
        let first = encode_module_v6(&module).unwrap();
        let second = encode_module_v6(&module).unwrap();
        assert_eq!(first, second);
        if operator == CheckedBinaryOperator::Add {
            assert_eq!(first, from_hex(CHECKED_ADD_I128_V6_GOLDEN_HEX));
        }
        assert_eq!(first[8..10], KERNEL_IR_VERSION_V6.to_le_bytes());
        assert_eq!(first[operator_offset(&first, tag)], tag);
        assert_eq!(decode_module_v6(&first).unwrap(), module);
        assert_eq!(
            encode_module_v6(&decode_module_v6(&first).unwrap()).unwrap(),
            first
        );
        encodings.push(first);
    }
    assert!(encodings.windows(2).all(|pair| pair[0] != pair[1]));
}

#[test]
fn v6_is_additive_and_frozen_v5_rejects_checked_arithmetic() {
    let checked = checked_fixture(CheckedBinaryOperator::Add, ScalarType::U32);
    assert_eq!(
        encode_module_v5(&checked),
        Err(KernelIrEncodeError::UnsupportedInVersion {
            version: KERNEL_IR_VERSION_V5,
            feature: "checked integer binary operation",
        })
    );

    let mut legacy = checked;
    let operation = &mut legacy.functions[0].body.as_mut().unwrap().blocks[0].operations[0];
    operation.results.pop();
    let OperationKind::Binary { op, .. } = &mut operation.kind else {
        panic!()
    };
    *op = BinaryOp::Add;
    let v5 = encode_module_v5(&legacy).unwrap();
    assert_eq!(decode_module_v6(&v5).unwrap(), legacy);
    assert_eq!(
        encode_module_v5(&decode_module_v6(&v5).unwrap()).unwrap(),
        v5
    );

    let v6 = encode_module_v6(&checked_fixture(
        CheckedBinaryOperator::Add,
        ScalarType::U32,
    ))
    .unwrap();
    assert_eq!(
        decode_module_v5(&v6),
        Err(KernelIrDecodeError::UnknownVersion(KERNEL_IR_VERSION_V6))
    );
}

#[test]
fn hostile_operator_version_and_result_mutations_fail_closed() {
    let module = checked_fixture(CheckedBinaryOperator::Multiply, ScalarType::I32);
    let bytes = encode_module_v6(&module).unwrap();
    let offset = operator_offset(&bytes, 13);

    let mut unknown = bytes.clone();
    unknown[offset] = 14;
    assert_eq!(
        decode_module_v6(&unknown),
        Err(KernelIrDecodeError::UnknownTag {
            kind: "binary operation",
            tag: 14,
        })
    );

    let mut downgraded = bytes.clone();
    downgraded[8..10].copy_from_slice(&KERNEL_IR_VERSION_V5.to_le_bytes());
    assert_eq!(
        decode_module_v6(&downgraded),
        Err(KernelIrDecodeError::UnknownTag {
            kind: "binary operation",
            tag: 13,
        })
    );

    let mut result_mutation = module;
    result_mutation.functions[0].body.as_mut().unwrap().blocks[0].operations[0].results[1].ty =
        Type::Scalar(ScalarType::U8);
    let result_bytes = encode_module_v6(&result_mutation).unwrap();
    let decoded = decode_module_v6(&result_bytes).unwrap();
    assert!(
        verify_module(&decoded)
            .unwrap_err()
            .contains(DiagnosticCode::TypeMismatch)
    );
}

#[test]
fn v6_decoder_is_bounded_exact_and_panic_total_under_mutation() {
    let module = checked_fixture(CheckedBinaryOperator::Subtract, ScalarType::U64);
    let bytes = encode_module_v6(&module).unwrap();

    for end in 0..bytes.len() {
        assert!(decode_module_v6(&bytes[..end]).is_err(), "prefix {end}");
    }
    let mut trailing = bytes.clone();
    trailing.push(0);
    assert_eq!(
        decode_module_v6(&trailing),
        Err(KernelIrDecodeError::TrailingBytes)
    );
    assert_eq!(
        decode_module_v6(&vec![0; MAX_MODULE_BYTES_V1 + 1]),
        Err(KernelIrDecodeError::TooLarge {
            max: MAX_MODULE_BYTES_V1,
        })
    );

    for byte in 0..bytes.len() {
        for bit in 0..8 {
            let mut mutated = bytes.clone();
            mutated[byte] ^= 1 << bit;
            let outcome = catch_unwind(AssertUnwindSafe(|| decode_module_v6(&mutated)));
            let decoded = outcome.unwrap_or_else(|_| panic!("decoder panicked at {byte}:{bit}"));
            if let Ok(decoded) = decoded {
                assert_eq!(encode_module_v6(&decoded).unwrap(), mutated);
            }
        }
    }
}

#[test]
fn v6_encoder_checks_operation_result_bounds_before_publication() {
    let mut module = checked_fixture(CheckedBinaryOperator::Add, ScalarType::U8);
    let operation = &mut module.functions[0].body.as_mut().unwrap().blocks[0].operations[0];
    operation.results = vec![ValueDef::new(ValueId(9), Type::BOOL); MAX_OPERATION_RESULTS_V1 + 1];
    assert_eq!(
        encode_module_v6(&module),
        Err(KernelIrEncodeError::LimitExceeded {
            field: "operation results",
            actual: MAX_OPERATION_RESULTS_V1 + 1,
            max: MAX_OPERATION_RESULTS_V1,
        })
    );
}
