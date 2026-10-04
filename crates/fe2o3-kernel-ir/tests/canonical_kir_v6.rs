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
                _ => panic!("invalid frozen hex"),
            };
            (digit(pair[0]) << 4) | digit(pair[1])
        })
        .collect()
}

fn checked_fixture(operator: CheckedBinaryOperator, scalar: ScalarType) -> Module {
    let ty = Type::Scalar(scalar);
    let checked = Operation::checked_binary(
        ValueDef::new(ValueId(0x1020_3040), ty.clone()),
        ValueDef::new(ValueId(0x5060_7080), Type::BOOL),
        operator,
        LHS,
        RHS,
    );
    let mut block = BasicBlock::new(BlockId(0));
    block.operations.push(checked);
    block.terminator = Some(Terminator::Return { values: vec![] });

    let mut module = Module::new("canonical-checked-v6");
    module.functions.push(Function::definition(
        "checked",
        Signature::new(vec![ty.clone(), ty], vec![]),
        vec![LHS, RHS],
        vec![block],
    ));
    module
}

fn legacy_fixture() -> Module {
    let mut module = checked_fixture(CheckedBinaryOperator::Add, ScalarType::U32);
    let operation = &mut module.functions[0].body.as_mut().unwrap().blocks[0].operations[0];
    operation.results.pop();
    let OperationKind::Binary { op, .. } = &mut operation.kind else {
        unreachable!("fixture starts with a binary operation")
    };
    *op = BinaryOp::Add;
    module
}

#[test]
fn exact_v6_owner_is_deterministic_revalidates_and_consumes_bytes() {
    let first = VerifiedCanonicalKernelIrV6::from_module(checked_fixture(
        CheckedBinaryOperator::Add,
        ScalarType::I128,
    ))
    .unwrap();
    let second = VerifiedCanonicalKernelIrV6::from_module(checked_fixture(
        CheckedBinaryOperator::Add,
        ScalarType::I128,
    ))
    .unwrap();
    assert_eq!(first.canonical_bytes(), second.canonical_bytes());
    assert_eq!(first.identity(), second.identity());
    assert_eq!(
        first.identity().canonical_length(),
        first.canonical_bytes().len() as u64
    );
    first.revalidate().unwrap();

    let expected_bytes = first.canonical_bytes().to_vec();
    let original_pointer = expected_bytes.as_ptr();
    let recovered = VerifiedCanonicalKernelIrV6::from_canonical_bytes(expected_bytes).unwrap();
    assert_eq!(recovered.canonical_bytes().as_ptr(), original_pointer);
    assert_eq!(recovered.identity(), first.identity());
    assert_eq!(recovered.into_canonical_bytes(), first.canonical_bytes());
}

#[test]
fn exact_v6_owner_covers_all_checked_integer_types_and_operators() {
    let scalar_types = [
        ScalarType::I8,
        ScalarType::I16,
        ScalarType::I32,
        ScalarType::I64,
        ScalarType::I128,
        ScalarType::U8,
        ScalarType::U16,
        ScalarType::U32,
        ScalarType::U64,
        ScalarType::U128,
        ScalarType::Index,
    ];
    let operators = [
        CheckedBinaryOperator::Add,
        CheckedBinaryOperator::Subtract,
        CheckedBinaryOperator::Multiply,
    ];

    for scalar in scalar_types {
        for operator in operators {
            let owner = VerifiedCanonicalKernelIrV6::from_module(checked_fixture(operator, scalar))
                .unwrap_or_else(|error| panic!("{scalar:?} {operator:?} failed: {error}"));
            owner.revalidate().unwrap();
            assert_eq!(
                &owner.canonical_bytes()[8..10],
                &KERNEL_IR_VERSION_V6.to_le_bytes()
            );
        }
    }
}

#[test]
fn exact_v6_owner_rejects_every_v1_through_v5_encoding() {
    let module = legacy_fixture();
    let versions = [
        (1, encode_module_v1(&module).unwrap()),
        (2, encode_module_v2(&module).unwrap()),
        (3, encode_module_v3(&module).unwrap()),
        (4, encode_module_v4(&module).unwrap()),
        (5, encode_module_v5(&module).unwrap()),
    ];
    for (version, bytes) in versions {
        assert_eq!(
            VerifiedCanonicalKernelIrV6::from_canonical_bytes(bytes),
            Err(VerifiedCanonicalKernelIrErrorV6::NotExactV6 { version })
        );
    }
}

#[test]
fn exact_v6_owner_rejects_semantically_invalid_modules_and_bytes() {
    assert!(matches!(
        VerifiedCanonicalKernelIrV6::from_module(Module::new("")),
        Err(VerifiedCanonicalKernelIrErrorV6::Verification(_))
    ));

    let invalid_bytes = encode_module_v6(&Module::new("")).unwrap();
    assert!(decode_module_v6(&invalid_bytes).is_ok());
    assert!(matches!(
        VerifiedCanonicalKernelIrV6::from_canonical_bytes(invalid_bytes),
        Err(VerifiedCanonicalKernelIrErrorV6::Verification(_))
    ));

    let mut invalid_checked = checked_fixture(CheckedBinaryOperator::Multiply, ScalarType::U64);
    invalid_checked.functions[0].body.as_mut().unwrap().blocks[0].operations[0].results[1].ty =
        Type::Scalar(ScalarType::U8);
    let invalid_checked = encode_module_v6(&invalid_checked).unwrap();
    assert!(matches!(
        VerifiedCanonicalKernelIrV6::from_canonical_bytes(invalid_checked),
        Err(VerifiedCanonicalKernelIrErrorV6::Verification(_))
    ));
}

#[test]
fn checked_i128_v6_wire_and_authoritative_identity_are_frozen() {
    let bytes = from_hex(CHECKED_ADD_I128_V6_GOLDEN_HEX);
    assert_eq!(bytes.len(), 139);
    let owner = VerifiedCanonicalKernelIrV6::from_canonical_bytes(bytes.clone()).unwrap();
    assert_eq!(owner.canonical_bytes(), bytes);
    assert_eq!(owner.identity().canonical_length(), 139);
    assert_eq!(
        owner.identity().digest(),
        &[
            0x8c, 0x05, 0xb6, 0x8d, 0x18, 0x1e, 0x8c, 0xbc, 0x54, 0x28, 0x64, 0xff, 0x81, 0x32,
            0xdb, 0x7d, 0x83, 0x41, 0x62, 0x26, 0x94, 0x49, 0xab, 0x2e, 0xf1, 0x41, 0xab, 0x42,
            0xf5, 0xfb, 0x12, 0x9e,
        ]
    );
}

#[test]
fn exact_v6_bytes_reject_noncanonical_truncation_trailing_and_unknown_versions() {
    let owner = VerifiedCanonicalKernelIrV6::from_module(checked_fixture(
        CheckedBinaryOperator::Subtract,
        ScalarType::U64,
    ))
    .unwrap();
    let bytes = owner.canonical_bytes();
    for end in 0..bytes.len() {
        assert!(
            VerifiedCanonicalKernelIrV6::from_canonical_bytes(bytes[..end].to_vec()).is_err(),
            "prefix {end}"
        );
    }

    let mut trailing = bytes.to_vec();
    trailing.push(0);
    assert_eq!(
        VerifiedCanonicalKernelIrV6::from_canonical_bytes(trailing),
        Err(VerifiedCanonicalKernelIrErrorV6::Decode(
            KernelIrDecodeError::TrailingBytes
        ))
    );

    let mut unknown = bytes.to_vec();
    unknown[8..10].copy_from_slice(&7_u16.to_le_bytes());
    assert_eq!(
        VerifiedCanonicalKernelIrV6::from_canonical_bytes(unknown),
        Err(VerifiedCanonicalKernelIrErrorV6::NotExactV6 { version: 7 })
    );

    let mut capabilities = Module::new("m");
    capabilities.required_capabilities = [TargetCapability::Float16, TargetCapability::BFloat16]
        .into_iter()
        .collect();
    let encoded = encode_module_v6(&capabilities).unwrap();
    let first_capability = 20 + 5 + 12;
    let mut reordered = encoded;
    reordered.swap(first_capability, first_capability + 1);
    assert_eq!(
        VerifiedCanonicalKernelIrV6::from_canonical_bytes(reordered),
        Err(VerifiedCanonicalKernelIrErrorV6::Decode(
            KernelIrDecodeError::NonCanonical
        ))
    );
}

#[test]
fn exact_v6_owner_enforces_wire_and_in_memory_module_limits() {
    assert_eq!(
        VerifiedCanonicalKernelIrV6::from_canonical_bytes(vec![0; MAX_MODULE_BYTES_V1 + 1]),
        Err(VerifiedCanonicalKernelIrErrorV6::Decode(
            KernelIrDecodeError::TooLarge {
                max: MAX_MODULE_BYTES_V1
            }
        ))
    );

    let mut oversized_id = checked_fixture(CheckedBinaryOperator::Add, ScalarType::U8);
    oversized_id.id = ModuleId::new("x".repeat(MAX_TEXT_BYTES_V1 + 1));
    assert_eq!(
        VerifiedCanonicalKernelIrV6::from_module(oversized_id),
        Err(VerifiedCanonicalKernelIrErrorV6::Encode(
            KernelIrEncodeError::LimitExceeded {
                field: "module ID",
                actual: MAX_TEXT_BYTES_V1 + 1,
                max: MAX_TEXT_BYTES_V1,
            }
        ))
    );

    let mut oversized_results = checked_fixture(CheckedBinaryOperator::Add, ScalarType::U8);
    oversized_results.functions[0].body.as_mut().unwrap().blocks[0].operations[0].results =
        vec![ValueDef::new(ValueId(9), Type::BOOL); MAX_OPERATION_RESULTS_V1 + 1];
    assert_eq!(
        VerifiedCanonicalKernelIrV6::from_module(oversized_results),
        Err(VerifiedCanonicalKernelIrErrorV6::Encode(
            KernelIrEncodeError::LimitExceeded {
                field: "operation results",
                actual: MAX_OPERATION_RESULTS_V1 + 1,
                max: MAX_OPERATION_RESULTS_V1,
            }
        ))
    );
}

#[test]
fn exact_v6_owner_is_panic_total_and_identity_sensitive_under_mutation() {
    let owner = VerifiedCanonicalKernelIrV6::from_module(checked_fixture(
        CheckedBinaryOperator::Multiply,
        ScalarType::I32,
    ))
    .unwrap();
    let baseline_bytes = owner.canonical_bytes().to_vec();
    let baseline_identity = *owner.identity();

    for byte in 0..baseline_bytes.len() {
        for bit in 0..8 {
            let mut mutated = baseline_bytes.clone();
            mutated[byte] ^= 1 << bit;
            let outcome = catch_unwind(AssertUnwindSafe(|| {
                VerifiedCanonicalKernelIrV6::from_canonical_bytes(mutated)
            }));
            let admitted = outcome
                .unwrap_or_else(|_| panic!("owner admission panicked at byte {byte} bit {bit}"));
            if let Ok(admitted) = admitted {
                admitted.revalidate().unwrap();
                assert_ne!(admitted.identity(), &baseline_identity);
                assert_ne!(admitted.canonical_bytes(), baseline_bytes);
            }
        }
    }
}
