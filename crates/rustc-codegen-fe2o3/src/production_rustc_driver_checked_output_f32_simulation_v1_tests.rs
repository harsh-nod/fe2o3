//! Test oracle only. Expected bits are independent of simulator evaluation.
use super::*;
use fe2o3_kernel_ir::{BinaryOp, OperationKind, UnaryOp};

pub(super) const NUMERICAL_POLICY: &str =
    "ieee-f32-rne-bit-vectors-negate-exact-divide-nan-class-v1";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Expected {
    Bits(u32),
    Nan,
}

#[derive(Clone, Copy)]
struct Vector {
    label: &'static str,
    left: u32,
    right: u32,
    expected: Expected,
}

fn vectors(case: Case) -> Vec<Vector> {
    let rows: &[(&str, u32, u32, Expected)] = match case {
        Case::F32Negate => &[
            ("positive-zero", 0, 0, Expected::Bits(0x8000_0000)),
            ("negative-zero", 0x8000_0000, 0, Expected::Bits(0)),
            (
                "positive-finite",
                0x3fc0_0000,
                0,
                Expected::Bits(0xbfc0_0000),
            ),
            (
                "negative-finite",
                0xbf80_0000,
                0,
                Expected::Bits(0x3f80_0000),
            ),
            ("min-subnormal", 1, 0, Expected::Bits(0x8000_0001)),
            ("min-normal", 0x0080_0000, 0, Expected::Bits(0x8080_0000)),
            (
                "positive-infinity",
                0x7f80_0000,
                0,
                Expected::Bits(0xff80_0000),
            ),
            (
                "negative-infinity",
                0xff80_0000,
                0,
                Expected::Bits(0x7f80_0000),
            ),
            (
                "quiet-nan-payload",
                0x7fc0_0042,
                0,
                Expected::Bits(0xffc0_0042),
            ),
            (
                "signaling-nan-payload",
                0x7f80_0042,
                0,
                Expected::Bits(0xff80_0042),
            ),
        ],
        Case::F32Divide => &[
            (
                "finite",
                0x40c0_0000,
                0x4000_0000,
                Expected::Bits(0x4040_0000),
            ),
            (
                "round-one-third",
                0x3f80_0000,
                0x4040_0000,
                Expected::Bits(0x3eaa_aaab),
            ),
            ("positive-zero", 0, 0x3f80_0000, Expected::Bits(0)),
            (
                "negative-zero",
                0x8000_0000,
                0x3f80_0000,
                Expected::Bits(0x8000_0000),
            ),
            (
                "divide-positive-zero",
                0x3f80_0000,
                0,
                Expected::Bits(0x7f80_0000),
            ),
            (
                "divide-negative-zero",
                0x3f80_0000,
                0x8000_0000,
                Expected::Bits(0xff80_0000),
            ),
            (
                "negative-over-infinity",
                0xbf80_0000,
                0x7f80_0000,
                Expected::Bits(0x8000_0000),
            ),
            (
                "infinity-over-negative",
                0x7f80_0000,
                0xbf80_0000,
                Expected::Bits(0xff80_0000),
            ),
            (
                "overflow",
                0x7f7f_ffff,
                0x3f00_0000,
                Expected::Bits(0x7f80_0000),
            ),
            (
                "normal-to-subnormal",
                0x0080_0000,
                0x4000_0000,
                Expected::Bits(0x0040_0000),
            ),
            ("underflow-even-zero", 1, 0x4000_0000, Expected::Bits(0)),
            ("subnormal-even-two", 3, 0x4000_0000, Expected::Bits(2)),
            ("zero-over-zero", 0, 0, Expected::Nan),
            (
                "infinity-over-infinity",
                0x7f80_0000,
                0x7f80_0000,
                Expected::Nan,
            ),
            ("quiet-nan-input", 0x7fc0_0042, 0x3f80_0000, Expected::Nan),
            (
                "signaling-nan-input",
                0x7f80_0042,
                0x3f80_0000,
                Expected::Nan,
            ),
        ],
        _ => unreachable!("only F32 source oracle cases"),
    };
    rows.iter()
        .map(|&(label, left, right, expected)| Vector {
            label,
            left,
            right,
            expected,
        })
        .collect()
}

fn scenario(vector: Vector, len: usize) -> Result<Scenario, SourceFailure> {
    let (output, backing) = guarded_buffer(0, &vec![SENTINEL; len], AccessMode::ReadWrite)?;
    let bits = match vector.expected {
        Expected::Bits(bits) => bits,
        Expected::Nan => 0x7fc0_0000,
    };
    let expected = guarded_buffer(0, &vec![f32::from_bits(bits); len], AccessMode::ReadWrite)?.1;
    let scalar = |bits: u32| {
        ScalarBitsV1::new(ScalarType::F32, bits.into(), TARGET)
            .map(SimulationArgumentV1::Scalar)
            .map_err(failure)
    };
    Ok(Scenario {
        label: format!("{}-len-{len}", vector.label),
        active: len,
        arguments: vec![output, scalar(vector.left)?, scalar(vector.right)?],
        backings: vec![backing],
        expected: vec![expected],
        output_elements: len,
        written_elements: len,
    })
}

pub(super) fn scenarios(case: Case) -> Result<Vec<Scenario>, SourceFailure> {
    let vectors = vectors(case);
    let mut scenarios = vec![scenario(vectors[0], 0)?];
    for (index, vector) in vectors.into_iter().enumerate() {
        scenarios.push(scenario(vector, [1, 63, 64, 65][index % 4])?);
    }
    Ok(scenarios)
}

pub(super) fn require_abi(
    module: &AdmittedSimulationModuleV1,
    case: Case,
) -> Result<&Kernel, SourceFailure> {
    let [kernel] = module.module().kernels.as_slice() else {
        return Err(failure("F32 oracle requires one actual output root"));
    };
    let function = module
        .module()
        .function(&kernel.entry)
        .ok_or_else(|| failure("actual F32 entry missing"))?;
    let [Type::Slice(output), left, right] = function.signature.parameters.as_slice() else {
        return Err(failure(
            "F32 oracle requires output slice and two scalar ABI inputs",
        ));
    };
    if output.address_space != AddressSpace::Global
        || output.element.as_ref() != &Type::F32
        || output.access != AccessMode::ReadWrite
        || left != &Type::F32
        || right != &Type::F32
        || !function.signature.results.is_empty()
        || kernel.domain.rank() != 1
    {
        return Err(failure("actual O differs from exact F32 scalar/output ABI"));
    }
    let mut operations = 0;
    for operation in module
        .module()
        .functions
        .iter()
        .filter_map(|function| function.body.as_ref())
        .flat_map(|body| &body.blocks)
        .flat_map(|block| &block.operations)
    {
        let selected = matches!(
            (&operation.kind, case),
            (
                OperationKind::Unary {
                    op: UnaryOp::Negate,
                    ..
                },
                Case::F32Negate
            ) | (
                OperationKind::Binary {
                    op: BinaryOp::Divide,
                    ..
                },
                Case::F32Divide
            )
        );
        if selected && matches!(operation.results.as_slice(), [value] if value.ty == Type::F32) {
            operations += 1;
        }
    }
    if operations != 1 {
        return Err(failure(
            "actual O must retain exactly one requested F32 operation",
        ));
    }
    Ok(kernel)
}

pub(super) fn check_backings(
    case: Case,
    actual: &[SharedBufferV1],
    expected: &[SharedBufferV1],
) -> Result<(), SourceFailure> {
    if case != Case::F32Divide {
        return super::check_backings(actual, expected);
    }
    let ([actual], [expected]) = (actual, expected) else {
        return Err(failure("F32 division oracle requires exactly one backing"));
    };
    let (actual_buffer, expected_buffer) = (&actual.buffer, &expected.buffer);
    if actual.id != expected.id
        || actual_buffer.element() != expected_buffer.element()
        || actual_buffer.access() != expected_buffer.access()
        || actual_buffer.alignment() != expected_buffer.alignment()
        || actual_buffer.initialized() != expected_buffer.initialized()
        || actual_buffer.bytes().len() != expected_buffer.bytes().len()
        || expected_buffer.bytes().len() < 2 * GUARD_ELEMENTS * 4
        || !expected_buffer.bytes().len().is_multiple_of(4)
    {
        return Err(failure(
            "F32 division changed backing metadata or initialization",
        ));
    }
    let suffix = expected_buffer.bytes().len() - GUARD_ELEMENTS * 4;
    for (ordinal, (actual, expected)) in actual_buffer
        .bytes()
        .chunks_exact(4)
        .zip(expected_buffer.bytes().chunks_exact(4))
        .enumerate()
    {
        let offset = ordinal * 4;
        let expected_bits = u32::from_le_bytes(expected.try_into().unwrap());
        let nan_output = offset >= GUARD_ELEMENTS * 4
            && offset < suffix
            && f32::from_bits(expected_bits).is_nan();
        if nan_output {
            let actual_bits = u32::from_le_bytes(actual.try_into().unwrap());
            if !f32::from_bits(actual_bits).is_nan() {
                return Err(failure("F32 division result is not the expected NaN class"));
            }
        } else if actual != expected {
            return Err(failure(
                "F32 division differs from reference bits or output canaries",
            ));
        }
    }
    Ok(())
}

pub(super) fn check_native(case: Case, llvm: &str) -> Result<(), SourceFailure> {
    let opcode = match case {
        Case::F32Negate => "fneg",
        Case::F32Divide => "fdiv",
        _ => return Ok(()),
    };
    if llvm.matches(&format!(" = {opcode} float ")).count() != 1 {
        return Err(SourceFailure::new(
            SourceStage::NativeHandoff,
            "actual native output must contain exactly one strict F32 operation",
        ));
    }
    for attribute in [
        "\"denormal-fp-math-f32\"=\"ieee,ieee\"",
        "\"unsafe-fp-math\"=\"false\"",
        "\"no-infs-fp-math\"=\"false\"",
        "\"no-nans-fp-math\"=\"false\"",
        "\"no-signed-zeros-fp-math\"=\"false\"",
        "\"approx-func-fp-math\"=\"false\"",
        "\"fp-contract\"=\"off\"",
    ] {
        if !llvm.contains(attribute) {
            return Err(SourceFailure::new(
                SourceStage::NativeHandoff,
                format!("actual native F32 output lacks {attribute}"),
            ));
        }
    }
    for flag in [
        " fast ",
        " reassoc ",
        " arcp ",
        " afn ",
        " nnan ",
        " ninf ",
        " nsz ",
        " contract ",
    ] {
        if llvm.contains(flag) {
            return Err(SourceFailure::new(
                SourceStage::NativeHandoff,
                format!("actual native F32 output has forbidden flag {flag}"),
            ));
        }
    }
    Ok(())
}

#[test]
fn source_f32_vectors_keep_signed_zero_nan_and_rounding_expectations_independent() {
    assert_eq!(scenarios(Case::F32Negate).unwrap().len(), 11);
    assert_eq!(scenarios(Case::F32Divide).unwrap().len(), 17);
    for case in [Case::F32Negate, Case::F32Divide] {
        let scenarios = scenarios(case).unwrap();
        assert_eq!(scenarios[0].written_elements, 0);
        for len in [0, 1, 63, 64, 65] {
            assert!(
                scenarios
                    .iter()
                    .any(|scenario| scenario.output_elements == len)
            );
        }
        for scenario in &scenarios {
            assert_eq!(scenario.written_elements, scenario.output_elements);
            check_backings(case, &scenario.expected, &scenario.expected).unwrap();
            assert_eq!(scenario.arguments.len(), 3);
        }
    }
    let division = vectors(Case::F32Divide);
    assert_eq!(
        division
            .iter()
            .filter(|vector| vector.expected == Expected::Nan)
            .count(),
        4
    );
    assert_eq!(division[1].expected, Expected::Bits(0x3eaa_aaab));
    assert_eq!(division[11].expected, Expected::Bits(2));
}

fn change_buffer(backing: &SharedBufferV1, offset: usize, bits: u32) -> SharedBufferV1 {
    let mut bytes = backing.buffer.bytes().to_vec();
    bytes[offset..offset + 4].copy_from_slice(&bits.to_le_bytes());
    SharedBufferV1 {
        id: backing.id,
        buffer: BufferArgumentV1::new(
            backing.buffer.element(),
            backing.buffer.access(),
            backing.buffer.alignment(),
            bytes,
            backing.buffer.initialized().to_vec(),
            TARGET,
        )
        .unwrap(),
    }
}

#[test]
fn source_f32_nan_class_never_weakens_finite_zero_canary_or_negation_checks() {
    let vector = vectors(Case::F32Divide)[12];
    let expected = scenario(vector, 1).unwrap().expected;
    let changed = change_buffer(&expected[0], GUARD_ELEMENTS * 4, 0xffc0_1234);
    check_backings(Case::F32Divide, &[changed], &expected).unwrap();
    for (offset, bits) in [
        (0, 0x7fc0_0000),
        (GUARD_ELEMENTS * 4, 0x7f80_0000),
        ((GUARD_ELEMENTS + 1) * 4, 0x7fc0_0000),
    ] {
        let changed = change_buffer(&expected[0], offset, bits);
        assert!(check_backings(Case::F32Divide, &[changed], &expected).is_err());
    }
    for (case, vector, replacement) in [
        (Case::F32Divide, vectors(Case::F32Divide)[0], 0x7fc0_0000),
        (Case::F32Divide, vectors(Case::F32Divide)[2], 0x8000_0000),
        (Case::F32Negate, vectors(Case::F32Negate)[8], 0xffc0_0043),
    ] {
        let expected = scenario(vector, 1).unwrap().expected;
        let changed = change_buffer(&expected[0], GUARD_ELEMENTS * 4, replacement);
        assert!(check_backings(case, &[changed], &expected).is_err());
    }
}

#[test]
fn source_f32_native_observation_rejects_opcode_and_fast_math_mutations() {
    // Inert text tests exercise diagnostic checks only, not native custody.
    let attributes = "\"denormal-fp-math-f32\"=\"ieee,ieee\" \"unsafe-fp-math\"=\"false\" \"no-infs-fp-math\"=\"false\" \"no-nans-fp-math\"=\"false\" \"no-signed-zeros-fp-math\"=\"false\" \"approx-func-fp-math\"=\"false\" \"fp-contract\"=\"off\"";
    for (case, opcode) in [(Case::F32Negate, "fneg"), (Case::F32Divide, "fdiv")] {
        let operands = if case == Case::F32Negate {
            "%left"
        } else {
            "%left, %right"
        };
        let llvm = format!("%value = {opcode} float {operands}\n{attributes}");
        check_native(case, &llvm).unwrap();
        assert!(check_native(case, &llvm.replace(opcode, "fadd")).is_err());
        assert!(check_native(case, &llvm.replace(" float ", " fast float ")).is_err());
        assert!(
            check_native(
                case,
                &llvm.replace("ieee,ieee", "preserve-sign,preserve-sign")
            )
            .is_err()
        );
    }
}

#[test]
fn source_f32_nan_class_preserves_backing_identity_and_initialization_checks() {
    let expected = scenario(vectors(Case::F32Divide)[12], 1).unwrap().expected;
    for change in 0..4 {
        let original = &expected[0].buffer;
        let mut initialized = original.initialized().to_vec();
        if change == 0 {
            initialized[GUARD_ELEMENTS * 4] = false;
        }
        let buffer = BufferArgumentV1::new(
            original.element(),
            if change == 1 {
                AccessMode::ReadOnly
            } else {
                original.access()
            },
            if change == 2 {
                original.alignment() * 2
            } else {
                original.alignment()
            },
            original.bytes().to_vec(),
            initialized,
            TARGET,
        )
        .unwrap();
        let mut actual = vec![SharedBufferV1 {
            id: expected[0].id,
            buffer,
        }];
        if change == 3 {
            actual[0].id = BufferBackingIdV1(1);
        }
        assert!(check_backings(Case::F32Divide, &actual, &expected).is_err());
    }
}

#[test]
fn source_f32_oracle_refuses_an_unrelated_verified_buffer_only_abi() {
    // This component fixture has no source custody; it only tests ABI refusal.
    let canonical = component_canonical(false);
    for case in [Case::F32Negate, Case::F32Divide] {
        let error = observe(&canonical, case).unwrap_err();
        assert_eq!(error.stage, SourceStage::Simulation);
        assert!(error.detail.contains("two scalar ABI inputs"));
    }
}
