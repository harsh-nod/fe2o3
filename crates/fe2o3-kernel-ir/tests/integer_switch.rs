use fe2o3_kernel_ir::*;

const GOLDEN_HEX: &str = include_str!("fixtures/integer_switch_v2.hex");
const TERMINATOR_TAG_OFFSET: usize = 78;
const CASE_COUNT_OFFSET: usize = 83;
const FIRST_CASE_OFFSET: usize = 87;
const CASE_RECORD_BYTES: usize = 13;

fn switch_module(selector_ty: Type, cases: Vec<IntegerSwitchCase>) -> Module {
    let mut entry = BasicBlock::new(BlockId(0));
    entry.terminator = Some(Terminator::IntegerSwitch {
        selector: ValueId(0),
        cases,
        default_target: BlockId(3),
        default_arguments: vec![],
    });

    let mut blocks = vec![entry];
    for id in 1..=3 {
        let mut block = BasicBlock::new(BlockId(id));
        block.terminator = Some(Terminator::Return { values: vec![] });
        blocks.push(block);
    }

    let mut module = Module::new("m");
    module.functions.push(Function::definition(
        "f",
        Signature::new(vec![selector_ty], vec![]),
        vec![ValueId(0)],
        blocks,
    ));
    module
}

fn canonical_module() -> Module {
    switch_module(
        Type::Scalar(ScalarType::I32),
        vec![
            IntegerSwitchCase {
                value: Constant::I32(-7),
                target: BlockId(1),
                arguments: vec![],
            },
            IntegerSwitchCase {
                value: Constant::I32(42),
                target: BlockId(2),
                arguments: vec![],
            },
        ],
    )
}

fn terminator_mut(module: &mut Module) -> &mut Terminator {
    module.functions[0].body.as_mut().unwrap().blocks[0]
        .terminator
        .as_mut()
        .unwrap()
}

fn cases_mut(module: &mut Module) -> &mut Vec<IntegerSwitchCase> {
    let Terminator::IntegerSwitch { cases, .. } = terminator_mut(module) else {
        panic!("expected integer switch")
    };
    cases
}

fn to_hex(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write;
        write!(output, "{byte:02x}").unwrap();
    }
    output
}

fn from_hex(input: &str) -> Vec<u8> {
    let compact: String = input
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect();
    assert_eq!(compact.len() % 2, 0);
    compact
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).expect("valid fixture hex")
        })
        .collect()
}

#[test]
fn reports_operands_and_successors_in_canonical_edge_order() {
    let terminator = Terminator::IntegerSwitch {
        selector: ValueId(1),
        cases: vec![
            IntegerSwitchCase {
                value: Constant::U16(3),
                target: BlockId(10),
                arguments: vec![ValueId(2), ValueId(3)],
            },
            IntegerSwitchCase {
                value: Constant::U16(9),
                target: BlockId(11),
                arguments: vec![ValueId(4)],
            },
        ],
        default_target: BlockId(12),
        default_arguments: vec![ValueId(5)],
    };

    assert_eq!(
        terminator.operands(),
        vec![ValueId(1), ValueId(2), ValueId(3), ValueId(4), ValueId(5)]
    );
    assert_eq!(
        terminator.successors(),
        vec![BlockId(10), BlockId(11), BlockId(12)]
    );
}

#[test]
fn verifies_a_typed_sorted_integer_switch() {
    verify_module(&canonical_module()).expect("canonical integer switch should verify");
}

#[test]
fn verifier_rejects_duplicate_and_unsorted_cases_deterministically() {
    let mut duplicate = canonical_module();
    cases_mut(&mut duplicate)[1].value = Constant::I32(-7);
    let duplicate_errors = verify_module(&duplicate).unwrap_err();
    assert_eq!(
        duplicate_errors
            .diagnostics()
            .iter()
            .filter(|diagnostic| diagnostic.code == DiagnosticCode::DuplicateSwitchCase)
            .map(|diagnostic| diagnostic.message.as_str())
            .collect::<Vec<_>>(),
        vec!["integer switch case I32(-7) appears more than once"]
    );

    let mut unsorted = canonical_module();
    cases_mut(&mut unsorted).swap(0, 1);
    let unsorted_errors = verify_module(&unsorted).unwrap_err();
    assert_eq!(
        unsorted_errors
            .diagnostics()
            .iter()
            .filter(|diagnostic| diagnostic.code == DiagnosticCode::UnsortedSwitchCase)
            .map(|diagnostic| diagnostic.message.as_str())
            .collect::<Vec<_>>(),
        vec!["integer switch case I32(-7) is not greater than previous case I32(42)"]
    );
}

#[test]
fn verifier_rejects_noninteger_and_mismatched_case_types() {
    let mismatch = switch_module(
        Type::Scalar(ScalarType::I32),
        vec![IntegerSwitchCase {
            value: Constant::U32(7),
            target: BlockId(1),
            arguments: vec![],
        }],
    );
    let mismatch_errors = verify_module(&mismatch).unwrap_err();
    assert!(mismatch_errors.contains(DiagnosticCode::TypeMismatch));

    let noninteger = switch_module(
        Type::F32,
        vec![IntegerSwitchCase {
            value: Constant::F32Bits(0),
            target: BlockId(1),
            arguments: vec![],
        }],
    );
    let noninteger_errors = verify_module(&noninteger).unwrap_err();
    assert_eq!(
        noninteger_errors
            .diagnostics()
            .iter()
            .filter(|diagnostic| diagnostic.code == DiagnosticCode::InvalidOperandType)
            .count(),
        2
    );
}

#[test]
fn verifier_checks_every_case_and_default_edge() {
    let mut module = canonical_module();
    cases_mut(&mut module)[0].target = BlockId(98);
    let Terminator::IntegerSwitch { default_target, .. } = terminator_mut(&mut module) else {
        unreachable!()
    };
    *default_target = BlockId(99);

    let errors = verify_module(&module).unwrap_err();
    assert_eq!(
        errors
            .diagnostics()
            .iter()
            .filter(|diagnostic| diagnostic.code == DiagnosticCode::InvalidBranchTarget)
            .count(),
        2
    );

    let mut entry = BasicBlock::new(BlockId(0));
    entry.terminator = Some(Terminator::IntegerSwitch {
        selector: ValueId(0),
        cases: vec![IntegerSwitchCase {
            value: Constant::I32(1),
            target: BlockId(1),
            arguments: vec![ValueId(1)],
        }],
        default_target: BlockId(2),
        default_arguments: vec![],
    });
    let mut case_target = BasicBlock::new(BlockId(1));
    case_target
        .parameters
        .push(ValueDef::new(ValueId(10), Type::Scalar(ScalarType::U32)));
    case_target.terminator = Some(Terminator::Return { values: vec![] });
    let mut default_target = BasicBlock::new(BlockId(2));
    default_target
        .parameters
        .push(ValueDef::new(ValueId(11), Type::Scalar(ScalarType::I32)));
    default_target.terminator = Some(Terminator::Return { values: vec![] });
    let mut edge_module = Module::new("edges");
    edge_module.functions.push(Function::definition(
        "f",
        Signature::new(
            vec![Type::Scalar(ScalarType::I32), Type::Scalar(ScalarType::I32)],
            vec![],
        ),
        vec![ValueId(0), ValueId(1)],
        vec![entry, case_target, default_target],
    ));
    let edge_errors = verify_module(&edge_module).unwrap_err();
    assert!(edge_errors.contains(DiagnosticCode::BranchArgumentType));
    assert!(edge_errors.contains(DiagnosticCode::BranchArgumentCount));
}

#[test]
fn v2_golden_roundtrip_is_deterministic_and_v1_remains_unsupported() {
    let module = canonical_module();
    let encoded = encode_module_v2(&module).expect("encode V2 integer switch");

    assert_eq!(to_hex(&encoded), GOLDEN_HEX.trim());
    assert_eq!(encode_module_v2(&module).unwrap(), encoded);
    assert_eq!(decode_module_v2(&encoded).unwrap(), module);
    assert_eq!(
        encode_module_v1(&module),
        Err(KernelIrEncodeError::UnsupportedInVersion {
            version: KERNEL_IR_VERSION_V1,
            feature: "typed integer switch terminator",
        })
    );
}

#[test]
fn latest_decoder_preserves_the_legacy_v1_switch() {
    let mut entry = BasicBlock::new(BlockId(0));
    entry.terminator = Some(Terminator::Switch {
        selector: ValueId(0),
        cases: vec![SwitchCase {
            value: 7,
            target: BlockId(1),
            arguments: vec![],
        }],
        default_target: BlockId(1),
        default_arguments: vec![],
    });
    let mut target = BasicBlock::new(BlockId(1));
    target.terminator = Some(Terminator::Return { values: vec![] });
    let mut module = Module::new("legacy");
    module.functions.push(Function::definition(
        "f",
        Signature::new(vec![Type::Scalar(ScalarType::U64)], vec![]),
        vec![ValueId(0)],
        vec![entry, target],
    ));

    let bytes = encode_module_v1(&module).unwrap();
    assert_eq!(decode_module_v1(&bytes).unwrap(), module);
    assert_eq!(decode_module_v2(&bytes).unwrap(), module);
}

#[test]
fn v1_decoder_rejects_a_forged_v2_integer_switch_tag() {
    let mut forged_v1 = encode_module_v2(&canonical_module()).unwrap();
    forged_v1[8..10].copy_from_slice(&KERNEL_IR_VERSION_V1.to_le_bytes());

    assert_eq!(
        decode_module_v1(&forged_v1),
        Err(KernelIrDecodeError::UnknownTag {
            kind: "terminator",
            tag: 6,
        })
    );
    assert_eq!(
        decode_module_v2(&forged_v1),
        Err(KernelIrDecodeError::UnknownTag {
            kind: "terminator",
            tag: 6,
        })
    );
}

#[test]
fn encoder_and_decoder_reject_noncanonical_case_order() {
    let mut duplicate_model = canonical_module();
    cases_mut(&mut duplicate_model)[1].value = Constant::I32(-7);
    assert_eq!(
        encode_module_v2(&duplicate_model),
        Err(KernelIrEncodeError::NonCanonical {
            field: "integer switch cases"
        })
    );

    let mut unsorted_model = canonical_module();
    cases_mut(&mut unsorted_model).swap(0, 1);
    assert_eq!(
        encode_module_v2(&unsorted_model),
        Err(KernelIrEncodeError::NonCanonical {
            field: "integer switch cases"
        })
    );

    let encoded = encode_module_v2(&canonical_module()).unwrap();
    assert_eq!(encoded[TERMINATOR_TAG_OFFSET], 6);

    let mut reversed = encoded.clone();
    let first = reversed[FIRST_CASE_OFFSET..FIRST_CASE_OFFSET + CASE_RECORD_BYTES].to_vec();
    let second = reversed
        [FIRST_CASE_OFFSET + CASE_RECORD_BYTES..FIRST_CASE_OFFSET + 2 * CASE_RECORD_BYTES]
        .to_vec();
    reversed[FIRST_CASE_OFFSET..FIRST_CASE_OFFSET + CASE_RECORD_BYTES].copy_from_slice(&second);
    reversed[FIRST_CASE_OFFSET + CASE_RECORD_BYTES..FIRST_CASE_OFFSET + 2 * CASE_RECORD_BYTES]
        .copy_from_slice(&first);
    assert_eq!(
        decode_module_v2(&reversed),
        Err(KernelIrDecodeError::NonCanonical)
    );

    let mut duplicate = encoded;
    let first_value = duplicate[FIRST_CASE_OFFSET..FIRST_CASE_OFFSET + 5].to_vec();
    duplicate[FIRST_CASE_OFFSET + CASE_RECORD_BYTES..FIRST_CASE_OFFSET + CASE_RECORD_BYTES + 5]
        .copy_from_slice(&first_value);
    assert_eq!(
        decode_module_v2(&duplicate),
        Err(KernelIrDecodeError::NonCanonical)
    );
}

#[test]
fn rejects_truncation_trailing_bytes_and_unknown_tags() {
    let encoded = encode_module_v2(&canonical_module()).unwrap();
    for length in 0..encoded.len() {
        assert!(
            decode_module_v2(&encoded[..length]).is_err(),
            "accepted truncation at {length}"
        );
    }

    let mut trailing = encoded.clone();
    trailing.push(0);
    let length = u32::try_from(trailing.len()).unwrap();
    trailing[12..16].copy_from_slice(&length.to_le_bytes());
    assert_eq!(
        decode_module_v2(&trailing),
        Err(KernelIrDecodeError::TrailingBytes)
    );

    let mut unknown_terminator = encoded.clone();
    unknown_terminator[TERMINATOR_TAG_OFFSET] = 0xff;
    assert_eq!(
        decode_module_v2(&unknown_terminator),
        Err(KernelIrDecodeError::UnknownTag {
            kind: "terminator",
            tag: 0xff,
        })
    );

    let mut unknown_constant = encoded;
    unknown_constant[FIRST_CASE_OFFSET] = 0xff;
    assert_eq!(
        decode_module_v2(&unknown_constant),
        Err(KernelIrDecodeError::UnknownTag {
            kind: "constant",
            tag: 0xff,
        })
    );
}

#[test]
fn enforces_integer_switch_resource_bounds_before_allocation() {
    let too_many_cases = (0..=MAX_INTEGER_SWITCH_CASES_V2)
        .map(|value| IntegerSwitchCase {
            value: Constant::U64(value as u64),
            target: BlockId(1),
            arguments: vec![],
        })
        .collect();
    let oversized = switch_module(Type::Scalar(ScalarType::U64), too_many_cases);
    assert_eq!(
        encode_module_v2(&oversized),
        Err(KernelIrEncodeError::LimitExceeded {
            field: "integer switch cases",
            actual: MAX_INTEGER_SWITCH_CASES_V2 + 1,
            max: MAX_INTEGER_SWITCH_CASES_V2,
        })
    );

    let mut oversized_arguments = canonical_module();
    let Terminator::IntegerSwitch {
        default_arguments, ..
    } = terminator_mut(&mut oversized_arguments)
    else {
        unreachable!()
    };
    *default_arguments = vec![ValueId(0); MAX_VALUE_ARGUMENTS_V1 + 1];
    assert_eq!(
        encode_module_v2(&oversized_arguments),
        Err(KernelIrEncodeError::LimitExceeded {
            field: "integer switch default arguments",
            actual: MAX_VALUE_ARGUMENTS_V1 + 1,
            max: MAX_VALUE_ARGUMENTS_V1,
        })
    );

    let mut forged_count = encode_module_v2(&canonical_module()).unwrap();
    forged_count[CASE_COUNT_OFFSET..CASE_COUNT_OFFSET + 4]
        .copy_from_slice(&((MAX_INTEGER_SWITCH_CASES_V2 + 1) as u32).to_le_bytes());
    assert_eq!(
        decode_module_v2(&forged_count),
        Err(KernelIrDecodeError::LimitExceeded {
            field: "integer switch cases",
            actual: MAX_INTEGER_SWITCH_CASES_V2 + 1,
            max: MAX_INTEGER_SWITCH_CASES_V2,
        })
    );
}

#[test]
fn golden_fixture_is_canonical_v2() {
    let bytes = from_hex(GOLDEN_HEX);
    assert_eq!(bytes[8..10], KERNEL_IR_VERSION_V2.to_le_bytes());
    let module = decode_module_v2(&bytes).expect("decode independent V2 fixture");
    verify_module(&module).expect("fixture should verify");
    assert_eq!(encode_module_v2(&module).unwrap(), bytes);
}
