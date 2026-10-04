//! Test-only exact integer oracle; no compiler selector or artifact authority.
use super::*;
use fe2o3_kernel_ir::{BinaryOp, CheckedBinaryOperator, OperationKind};

pub(super) const NUMERICAL_POLICY: &str = "integer-saturation-i128-reference-exact-bytes-v1";

macro_rules! integer_cases {
    ($(($variant:ident, $scalar:ident, $signed:literal, $bits:literal, $name:literal, $add:literal, $sub:literal)),* $(,)?) => {
        #[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
        #[serde(rename_all = "kebab-case")]
        pub(in super::super) enum Integer { $($variant),* }
        impl Integer {
            pub(in super::super) const ALL: [Self; 10] = [$(Self::$variant),*];
            pub(in super::super) fn rust_name(self) -> &'static str { match self { $(Self::$variant => $name),* } }
            fn scalar(self) -> ScalarType { match self { $(Self::$variant => ScalarType::$scalar),* } }
            fn signed(self) -> bool { match self { $(Self::$variant => $signed),* } }
            fn bits(self) -> u32 { match self { $(Self::$variant => $bits),* } }
        }
        impl OperationCase {
            pub(super) fn name(self) -> &'static str { match (self.integer, self.subtract) {
                $((Integer::$variant, false) => $add, (Integer::$variant, true) => $sub),*
            } }
            pub(super) fn parse(name: &str) -> Option<Self> { match name {
                $($add => Some(Self { integer: Integer::$variant, subtract: false }),
                  $sub => Some(Self { integer: Integer::$variant, subtract: true })),*,
                _ => None,
            } }
        }
    }
}

integer_cases! {
    (I8, I8, true, 8, "i8", "sat-add-i8", "sat-sub-i8"),
    (I16, I16, true, 16, "i16", "sat-add-i16", "sat-sub-i16"),
    (I32, I32, true, 32, "i32", "sat-add-i32", "sat-sub-i32"),
    (I64, I64, true, 64, "i64", "sat-add-i64", "sat-sub-i64"),
    (Isize, I64, true, 64, "isize", "sat-add-isize", "sat-sub-isize"),
    (U8, U8, false, 8, "u8", "sat-add-u8", "sat-sub-u8"),
    (U16, U16, false, 16, "u16", "sat-add-u16", "sat-sub-u16"),
    (U32, U32, false, 32, "u32", "sat-add-u32", "sat-sub-u32"),
    (U64, U64, false, 64, "u64", "sat-add-u64", "sat-sub-u64"),
    (Usize, U64, false, 64, "usize", "sat-add-usize", "sat-sub-usize"),
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(in super::super) struct OperationCase {
    pub(in super::super) integer: Integer,
    pub(in super::super) subtract: bool,
}

fn bounds(integer: Integer) -> (i128, i128) {
    if integer.signed() {
        (
            -(1_i128 << (integer.bits() - 1)),
            (1_i128 << (integer.bits() - 1)) - 1,
        )
    } else {
        (0, (1_i128 << integer.bits()) - 1)
    }
}

fn encode(integer: Integer, value: i128) -> u128 {
    (value as u128) & ((1_u128 << integer.bits()) - 1)
}

fn expected(case: OperationCase, left: i128, right: i128) -> u128 {
    // All admitted operands are <=64 bits, so the full mathematical sum or
    // difference fits i128. This does not reuse host saturating_* or KIR logic.
    let value = if case.subtract {
        left - right
    } else {
        left + right
    };
    let (min, max) = bounds(case.integer);
    encode(case.integer, value.clamp(min, max))
}

fn vectors(case: OperationCase) -> Vec<(i128, i128)> {
    let (min, max) = bounds(case.integer);
    let mut rows = vec![
        (0, 0),
        (max, 1),
        (min, 1),
        (max, max),
        (min, min),
        (max, min),
        (min, max),
        (1, 1),
        (max - 1, 1),
        (max - 1, 2),
    ];
    if case.integer.signed() {
        rows.extend([(min, -1), (-1, 1), (1, -1), (2, -3)]);
    }
    rows
}

fn guarded_integer(
    integer: Integer,
    value: u128,
    len: usize,
) -> Result<(SimulationArgumentV1, SharedBufferV1), SourceFailure> {
    let width = (integer.bits() / 8) as usize;
    let mut bytes = vec![0xa5; (len + 2 * GUARD_ELEMENTS) * width];
    // Distinct prefix/suffix guards, including narrow integers, are checked as
    // complete backing bytes; they are never supplied as part of the view.
    bytes[(len + GUARD_ELEMENTS) * width..].fill(0x5a);
    for cell in
        bytes[GUARD_ELEMENTS * width..(GUARD_ELEMENTS + len) * width].chunks_exact_mut(width)
    {
        cell.copy_from_slice(&value.to_le_bytes()[..width]);
    }
    let buffer = BufferArgumentV1::new(
        integer.scalar(),
        AccessMode::ReadWrite,
        width as u32,
        bytes.clone(),
        vec![true; bytes.len()],
        TARGET,
    )
    .map_err(failure)?;
    let view = BufferViewArgumentV1::new(
        BufferBackingIdV1(0),
        integer.scalar(),
        AccessMode::ReadWrite,
        width as u32,
        GUARD_ELEMENTS * width,
        len,
        TARGET,
    )
    .map_err(failure)?;
    Ok((
        SimulationArgumentV1::BufferView(view),
        SharedBufferV1 {
            id: BufferBackingIdV1(0),
            buffer,
        },
    ))
}

fn scenario(
    case: OperationCase,
    ordinal: usize,
    left: i128,
    right: i128,
    len: usize,
) -> Result<Scenario, SourceFailure> {
    let scalar = |value| {
        ScalarBitsV1::new(case.integer.scalar(), encode(case.integer, value), TARGET)
            .map(SimulationArgumentV1::Scalar)
            .map_err(failure)
    };
    let (output, backing) = guarded_integer(case.integer, 0x37, len)?;
    let expected = guarded_integer(case.integer, expected(case, left, right), len)?.1;
    Ok(Scenario {
        label: format!("integer-boundary-{ordinal}-len-{len}"),
        active: len,
        arguments: vec![output, scalar(left)?, scalar(right)?],
        backings: vec![backing],
        expected: vec![expected],
        output_elements: len,
        written_elements: len,
    })
}

pub(super) fn scenarios(case: OperationCase) -> Result<Vec<Scenario>, SourceFailure> {
    let mut result = vec![scenario(case, 0, 0, 0, 0)?];
    for (ordinal, (left, right)) in vectors(case).into_iter().enumerate() {
        result.push(scenario(
            case,
            ordinal + 1,
            left,
            right,
            [1, 63, 64, 65][ordinal % 4],
        )?);
    }
    Ok(result)
}

pub(super) fn require_abi(
    module: &AdmittedSimulationModuleV1,
    case: OperationCase,
) -> Result<&Kernel, SourceFailure> {
    let [kernel] = module.module().kernels.as_slice() else {
        return Err(failure("integer oracle requires one actual O root"));
    };
    let function = module
        .module()
        .function(&kernel.entry)
        .ok_or_else(|| failure("integer actual O entry missing"))?;
    let [Type::Slice(output), left, right] = function.signature.parameters.as_slice() else {
        return Err(failure("integer oracle output/scalar ABI arity"));
    };
    let scalar = Type::Scalar(case.integer.scalar());
    if output.address_space != AddressSpace::Global
        || output.element.as_ref() != &scalar
        || output.access != AccessMode::ReadWrite
        || left != &scalar
        || right != &scalar
        || !function.signature.results.is_empty()
        || kernel.domain.rank() != 1
    {
        return Err(failure(
            "actual O differs from requested exact signed/width integer ABI",
        ));
    }
    let expected_op = if case.subtract {
        CheckedBinaryOperator::Subtract
    } else {
        CheckedBinaryOperator::Add
    };
    let mut arithmetic = 0;
    let mut selects = 0;
    for operation in module
        .module()
        .functions
        .iter()
        .filter_map(|function| function.body.as_ref())
        .flat_map(|body| &body.blocks)
        .flat_map(|block| &block.operations)
    {
        if let OperationKind::Binary {
            op: BinaryOp::Checked(op),
            ..
        } = &operation.kind
            && *op == expected_op
            && matches!(operation.results.as_slice(), [value, flag]
                if value.ty == scalar && flag.ty == Type::BOOL)
        {
            arithmetic += 1;
        }
        if matches!(operation.kind, OperationKind::Select { .. })
            && matches!(operation.results.as_slice(), [value] if value.ty == scalar)
        {
            selects += 1;
        }
    }
    if arithmetic != 1 || selects != if case.integer.signed() { 2 } else { 1 } {
        return Err(failure(
            "actual O must retain exact checked arithmetic and saturation selects",
        ));
    }
    Ok(kernel)
}

pub(super) fn check_native(case: OperationCase, llvm: &str) -> Result<(), SourceFailure> {
    let name = format!(
        "llvm.{}{}.with.overflow.i{}",
        if case.integer.signed() { "s" } else { "u" },
        if case.subtract { "sub" } else { "add" },
        case.integer.bits()
    );
    if !llvm.contains(&name) || !llvm.contains("select i1") {
        return Err(failure(
            "native integer saturation lacks typed checked arithmetic/selects",
        ));
    }
    Ok(())
}

#[test]
fn widened_reference_covers_exhaustive_eight_bit_pairs() {
    for subtract in [false, true] {
        let signed = OperationCase {
            integer: Integer::I8,
            subtract,
        };
        let unsigned = OperationCase {
            integer: Integer::U8,
            subtract,
        };
        for left in i8::MIN..=i8::MAX {
            for right in i8::MIN..=i8::MAX {
                let host = if subtract {
                    left.saturating_sub(right)
                } else {
                    left.saturating_add(right)
                };
                assert_eq!(
                    expected(signed, left.into(), right.into()),
                    u128::from(host as u8)
                );
            }
        }
        for left in u8::MIN..=u8::MAX {
            for right in u8::MIN..=u8::MAX {
                let host = if subtract {
                    left.saturating_sub(right)
                } else {
                    left.saturating_add(right)
                };
                assert_eq!(
                    expected(unsigned, left.into(), right.into()),
                    u128::from(host)
                );
            }
        }
    }
}

#[test]
fn every_case_roundtrips_and_boundary_backings_are_exact() {
    for integer in Integer::ALL {
        for subtract in [false, true] {
            let case = OperationCase { integer, subtract };
            assert_eq!(OperationCase::parse(case.name()), Some(case));
            let rows = scenarios(case).unwrap();
            assert_eq!(rows.len(), if integer.signed() { 15 } else { 11 });
            assert_eq!(rows[0].output_elements, 0);
            for row in rows {
                assert_eq!(
                    row.backings[0].buffer.initialized(),
                    row.expected[0].buffer.initialized()
                );
                let width = (integer.bits() / 8) as usize;
                let bytes = row.expected[0].buffer.bytes();
                assert!(
                    bytes[..GUARD_ELEMENTS * width]
                        .iter()
                        .all(|byte| *byte == 0xa5)
                );
                assert!(
                    bytes[(GUARD_ELEMENTS + row.output_elements) * width..]
                        .iter()
                        .all(|byte| *byte == 0x5a)
                );
            }
        }
    }
    for name in [
        "sat-add-u128",
        "sat-sub-f32",
        "sat-add-bool",
        "sat-add-u32 ",
        "sat-mul-u32",
    ] {
        assert!(OperationCase::parse(name).is_none());
    }
}

#[test]
fn native_missing_wrong_opcode_or_wrong_width_refuses() {
    let case = OperationCase {
        integer: Integer::I64,
        subtract: true,
    };
    for llvm in [
        "",
        "llvm.usub.with.overflow.i64 select i1",
        "llvm.sadd.with.overflow.i64 select i1",
        "llvm.ssub.with.overflow.i32 select i1",
        "llvm.ssub.with.overflow.i64",
    ] {
        assert!(check_native(case, llvm).is_err());
    }
    check_native(case, "llvm.ssub.with.overflow.i64 select i1").unwrap();
}

#[test]
fn oracle_rejects_wrapping_results_canary_changes_and_lost_initialization() {
    let case = OperationCase {
        integer: Integer::U64,
        subtract: false,
    };
    let row = scenario(case, 1, u64::MAX.into(), 1, 1).unwrap();
    let original = &row.expected[0].buffer;
    for mutation in 0..5 {
        let mut bytes = original.bytes().to_vec();
        let mut initialized = original.initialized().to_vec();
        match mutation {
            0 => bytes[GUARD_ELEMENTS * 8..(GUARD_ELEMENTS + 1) * 8].fill(0),
            1 => bytes[0] ^= 1,
            2 => *bytes.last_mut().unwrap() ^= 1,
            3 => initialized[GUARD_ELEMENTS * 8] = false,
            _ => {}
        }
        let actual = vec![SharedBufferV1 {
            id: BufferBackingIdV1(if mutation == 4 { 1 } else { 0 }),
            buffer: BufferArgumentV1::new(
                original.element(),
                original.access(),
                original.alignment(),
                bytes,
                initialized,
                TARGET,
            )
            .unwrap(),
        }];
        assert!(
            super::check_backings(&actual, &row.expected).is_err(),
            "mutation {mutation}"
        );
    }
}

#[test]
fn oracle_refuses_an_unrelated_genuine_verified_abi() {
    // This checks the simulation boundary only, not Rust/source custody.
    let canonical = component_canonical(false);
    let error = observe(
        &canonical,
        Case::SaturatingInteger(OperationCase {
            integer: Integer::U32,
            subtract: true,
        }),
    )
    .unwrap_err();
    assert_eq!(error.stage, SourceStage::Simulation);
    assert!(error.detail.contains("output/scalar ABI arity"));
}
