use std::collections::BTreeSet;

use fe2o3_kernel_ir::*;

const HEADER_BYTES: usize = 20;
const FULL_V1_GOLDEN_HEX: &str = include_str!("fixtures/full_v1.hex");
const G4_SYNC_V2_GOLDEN_HEX: &str = include_str!("fixtures/g4_sync_v2.hex");
const INTEGER_SWITCH_V2_GOLDEN_HEX: &str = include_str!("fixtures/integer_switch_v2.hex");
const WAVE_OPERATIONS_V2_GOLDEN_HEX: &str = include_str!("fixtures/wave_operations_v2.hex");
const MATRIX_V5_GOLDEN_HEX: &str = include_str!("fixtures/matrix_v5.hex");

// These are intentionally embedded here so V3 and V4 have independent frozen
// bytes without adding a fixture format or a non-test dependency.
const INLINE_ASSEMBLY_V3_GOLDEN_HEX: &str = "4645324f334b4900030000002e01000000000000160000006b69722d76332d696e6c696e652d617373656d626c79010000000000000000000000030000006164640200000002080208000000000102000000000000000100000001000000000000000000000001000000010000000200000002081501010101010101010101010101010101010101010101010101010101010101010102020202020202020202020202020202020202020202020202020202020202020303030303030303030303030303030303030303030303030303030303030303040404040404040404040404040404040404040404040404040404040404040409000000765f6164645f75333203000000020200000000020100000000020101000000030000000103050000000001040000000000000000";
const WIDE_SCALARS_V4_GOLDEN_HEX: &str = "4645324f334b4900040000005000000000000000130000006b69722d76342d776964652d7363616c617273010000000000000000000000040000007769646501000000020f0100000002100000000000";

type Encoder = fn(&Module) -> Result<Vec<u8>, KernelIrEncodeError>;
type Decoder = fn(&[u8]) -> Result<Module, KernelIrDecodeError>;

fn codecs(version: u16) -> (Encoder, Decoder) {
    match version {
        KERNEL_IR_VERSION_V1 => (encode_module_v1, decode_module_v1),
        KERNEL_IR_VERSION_V2 => (encode_module_v2, decode_module_v2),
        KERNEL_IR_VERSION_V3 => (encode_module_v3, decode_module_v3),
        KERNEL_IR_VERSION_V4 => (encode_module_v4, decode_module_v4),
        KERNEL_IR_VERSION_V5 => (encode_module_v5, decode_module_v5),
        _ => panic!("unsupported test wire version {version}"),
    }
}

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

fn to_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}

fn unique_offset(bytes: &[u8], marker: &[u8]) -> usize {
    let offsets = bytes
        .windows(marker.len())
        .enumerate()
        .filter_map(|(offset, window)| (window == marker).then_some(offset))
        .collect::<Vec<_>>();
    assert_eq!(offsets.len(), 1, "marker must occur exactly once");
    offsets[0]
}

fn assert_frozen_fixture(version: u16, golden_hex: &str, expected: Option<&Module>) {
    let golden = from_hex(golden_hex);
    assert_eq!(
        &golden[8..10],
        &version.to_le_bytes(),
        "fixture has the wrong version"
    );

    let (encode, decode) = codecs(version);
    let decoded = decode(&golden).expect("frozen fixture must remain decodable");
    if let Some(expected) = expected {
        assert_eq!(&decoded, expected);
    }
    assert_eq!(
        encode(&decoded).expect("decoded fixture must remain encodable"),
        golden,
        "V{version} changed bytes"
    );
    assert_eq!(
        encode(&decoded).expect("repeated encoding must succeed"),
        encode(&decoded).expect("repeated encoding must succeed"),
        "V{version} encoding is nondeterministic"
    );

    let latest = decode_module_v5(&golden).expect("V5 must continue to read every old fixture");
    assert_eq!(latest, decoded);
    assert_eq!(
        encode(&latest).expect("latest-decoded fixture must remain encodable"),
        golden,
        "V{version} changed after migration through the latest public model"
    );
}

fn inline_assembly_v3_module() -> Module {
    let assembly = InlineAssembly {
        target: InlineAssemblyTarget::AmdGpuGfx942,
        source: AssemblySourceIdentity::new([1; 32], [2; 32], [3; 32], [4; 32]),
        mnemonic: "v_add_u32".to_owned(),
        operands: vec![
            AssemblyOperand::output(0, AssemblyConstraint::Vgpr32),
            AssemblyOperand::input(ValueId(0), AssemblyConstraint::Vgpr32),
            AssemblyOperand::input(ValueId(1), AssemblyConstraint::Vgpr32),
        ],
        options: BTreeSet::from([
            AssemblyOption::NoMemory,
            AssemblyOption::Pure,
            AssemblyOption::NoStack,
        ]),
        declared_effects: BTreeSet::new(),
    };
    let mut block = BasicBlock::new(BlockId(0));
    block.operations.push(Operation::new(
        vec![ValueDef::new(ValueId(2), Type::Scalar(ScalarType::U32))],
        OperationKind::InlineAssembly(assembly),
    ));
    block.terminator = Some(Terminator::Return { values: vec![] });

    let mut module = Module::new("kir-v3-inline-assembly");
    module.functions.push(Function::definition(
        "add",
        Signature::new(
            vec![Type::Scalar(ScalarType::U32), Type::Scalar(ScalarType::U32)],
            vec![],
        ),
        vec![ValueId(0), ValueId(1)],
        vec![block],
    ));
    module
}

fn wide_scalars_v4_module() -> Module {
    let mut module = Module::new("kir-v4-wide-scalars");
    module.functions.push(Function::declaration(
        "wide",
        Signature::new(
            vec![Type::Scalar(ScalarType::I128)],
            vec![Type::Scalar(ScalarType::U128)],
        ),
    ));
    module
}

#[test]
fn every_frozen_wire_version_has_exact_golden_and_byte_roundtrip_guards() {
    let v3 = inline_assembly_v3_module();
    let v4 = wide_scalars_v4_module();
    let fixtures = [
        (KERNEL_IR_VERSION_V1, FULL_V1_GOLDEN_HEX, None),
        (KERNEL_IR_VERSION_V2, G4_SYNC_V2_GOLDEN_HEX, None),
        (KERNEL_IR_VERSION_V2, INTEGER_SWITCH_V2_GOLDEN_HEX, None),
        (KERNEL_IR_VERSION_V2, WAVE_OPERATIONS_V2_GOLDEN_HEX, None),
        (
            KERNEL_IR_VERSION_V3,
            INLINE_ASSEMBLY_V3_GOLDEN_HEX,
            Some(&v3),
        ),
        (KERNEL_IR_VERSION_V4, WIDE_SCALARS_V4_GOLDEN_HEX, Some(&v4)),
        (KERNEL_IR_VERSION_V5, MATRIX_V5_GOLDEN_HEX, None),
    ];

    for (version, golden, expected) in fixtures {
        assert_frozen_fixture(version, golden, expected);
    }
}

#[test]
fn v3_and_v4_feature_fixtures_are_stable_at_the_encoder_boundary() {
    assert_eq!(
        to_hex(&encode_module_v3(&inline_assembly_v3_module()).unwrap()),
        INLINE_ASSEMBLY_V3_GOLDEN_HEX
    );
    assert_eq!(
        to_hex(&encode_module_v4(&wide_scalars_v4_module()).unwrap()),
        WIDE_SCALARS_V4_GOLDEN_HEX
    );
}

#[test]
fn every_version_rejects_reordered_and_duplicate_canonical_sets() {
    let mut module = Module::new("m");
    module.required_capabilities = [TargetCapability::Float16, TargetCapability::BFloat16]
        .into_iter()
        .collect();
    let first_capability = HEADER_BYTES + 5 + 12;

    for version in KERNEL_IR_VERSION_V1..=KERNEL_IR_VERSION_V5 {
        let (encode, decode) = codecs(version);
        let encoded = encode(&module).unwrap();

        let mut reordered = encoded.clone();
        reordered.swap(first_capability, first_capability + 1);
        assert_eq!(
            decode(&reordered),
            Err(KernelIrDecodeError::NonCanonical),
            "V{version} accepted a reordered capability set"
        );

        let mut duplicate = encoded;
        duplicate[first_capability + 1] = duplicate[first_capability];
        assert_eq!(
            decode(&duplicate),
            Err(KernelIrDecodeError::NonCanonical),
            "V{version} accepted a duplicate capability"
        );
    }
}

#[test]
fn every_compatible_decoder_rejects_reordered_v2_integer_switch_payloads() {
    const FIRST_CASE_OFFSET: usize = 87;
    const CASE_RECORD_BYTES: usize = 13;

    let canonical = from_hex(INTEGER_SWITCH_V2_GOLDEN_HEX);
    let mut reordered = canonical;
    let first = reordered[FIRST_CASE_OFFSET..FIRST_CASE_OFFSET + CASE_RECORD_BYTES].to_vec();
    let second = reordered
        [FIRST_CASE_OFFSET + CASE_RECORD_BYTES..FIRST_CASE_OFFSET + 2 * CASE_RECORD_BYTES]
        .to_vec();
    reordered[FIRST_CASE_OFFSET..FIRST_CASE_OFFSET + CASE_RECORD_BYTES].copy_from_slice(&second);
    reordered[FIRST_CASE_OFFSET + CASE_RECORD_BYTES..FIRST_CASE_OFFSET + 2 * CASE_RECORD_BYTES]
        .copy_from_slice(&first);

    for (name, decode) in [
        ("V2", decode_module_v2 as Decoder),
        ("V3", decode_module_v3 as Decoder),
        ("V4", decode_module_v4 as Decoder),
        ("V5", decode_module_v5 as Decoder),
    ] {
        assert_eq!(
            decode(&reordered),
            Err(KernelIrDecodeError::NonCanonical),
            "{name} accepted reordered integer-switch cases"
        );
    }
}

#[test]
fn every_compatible_decoder_rejects_reordered_v3_inline_assembly_options() {
    let mut reordered = from_hex(INLINE_ASSEMBLY_V3_GOLDEN_HEX);
    let option_count = unique_offset(&reordered, &[3, 0, 0, 0, 1, 3, 5, 0, 0, 0, 0]);
    reordered.swap(option_count + 4, option_count + 6);

    for (name, decode) in [
        ("V3", decode_module_v3 as Decoder),
        ("V4", decode_module_v4 as Decoder),
        ("V5", decode_module_v5 as Decoder),
    ] {
        assert_eq!(
            decode(&reordered),
            Err(KernelIrDecodeError::NonCanonical),
            "{name} accepted reordered inline-assembly options"
        );
    }
}

#[test]
fn kernel_ir_wire_crate_remains_pliron_independent() {
    let manifest = include_str!("../Cargo.toml");
    let dependency_mentions_pliron = manifest.lines().any(|line| {
        let line = line.trim();
        !line.starts_with('#') && line.to_ascii_lowercase().contains("pliron")
    });
    assert!(
        !dependency_mentions_pliron,
        "fe2o3-kernel-ir must not acquire a Pliron dependency"
    );
}
