//! Test-only Rust-as reference for the exact admitted optimized graph.
use super::*;
use fe2o3_kernel_ir::{CastKind, OperationKind};

pub(super) const NUMERICAL_POLICY: &str = "rust-as-integer-f32-rne-saturating-exact-bytes-v1";

macro_rules! integers {
    ($(($variant:ident, $ty:ident, $bits:literal, $signed:literal, $name:literal, $up:literal, $down:literal)),* $(,)?) => {
        #[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
        #[serde(rename_all = "kebab-case")]
        pub(in super::super) enum Integer { $($variant),* }
        impl Integer {
            pub(in super::super) const ALL: [Self; 8] = [$(Self::$variant),*];
            pub(in super::super) fn name(self) -> &'static str { match self { $(Self::$variant => $name),* } }
            fn scalar(self) -> ScalarType { match self { $(Self::$variant => ScalarType::$variant),* } }
            fn width(self) -> u32 { match self { $(Self::$variant => $bits),* } }
            fn signed(self) -> bool { match self { $(Self::$variant => $signed),* } }
            fn to_f32(self, input: u128) -> u128 { match self { $(Self::$variant => ((input as $ty) as f32).to_bits().into()),* } }
            fn cast_from_f32(self, input: f32) -> u128 { let bits = match self { $(Self::$variant => (input as $ty) as u128),* }; bits & self.mask() }
        }
        impl OperationCase {
            pub(in super::super) fn name(self) -> &'static str { match (self.integer, self.to_integer) {
                $((Integer::$variant, false) => $up, (Integer::$variant, true) => $down),*
            } }
            pub(super) fn parse(name: &str) -> Option<Self> { match name {
                $($up => Some(Self { integer: Integer::$variant, to_integer: false }),
                  $down => Some(Self { integer: Integer::$variant, to_integer: true })),*,
                _ => None,
            } }
        }
    }
}

integers! {
    (I8, i8, 8, true, "i8", "i8-to-f32", "f32-to-i8"),
    (U8, u8, 8, false, "u8", "u8-to-f32", "f32-to-u8"),
    (I16, i16, 16, true, "i16", "i16-to-f32", "f32-to-i16"),
    (U16, u16, 16, false, "u16", "u16-to-f32", "f32-to-u16"),
    (I32, i32, 32, true, "i32", "i32-to-f32", "f32-to-i32"),
    (U32, u32, 32, false, "u32", "u32-to-f32", "f32-to-u32"),
    (I64, i64, 64, true, "i64", "i64-to-f32", "f32-to-i64"),
    (U64, u64, 64, false, "u64", "u64-to-f32", "f32-to-u64"),
}

impl Integer {
    fn mask(self) -> u128 {
        (1_u128 << self.width()) - 1
    }
    fn bounds(self) -> (i128, i128) {
        if self.signed() {
            (
                -(1_i128 << (self.width() - 1)),
                (1_i128 << (self.width() - 1)) - 1,
            )
        } else {
            (0, self.mask() as i128)
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(in super::super) struct OperationCase {
    pub(in super::super) integer: Integer,
    pub(in super::super) to_integer: bool,
}

impl OperationCase {
    fn types(self) -> (ScalarType, ScalarType) {
        if self.to_integer {
            (ScalarType::F32, self.integer.scalar())
        } else {
            (self.integer.scalar(), ScalarType::F32)
        }
    }
    fn kind(self) -> CastKind {
        if self.to_integer {
            CastKind::FloatToInteger
        } else {
            CastKind::IntegerToFloat
        }
    }
    fn expected(self, input: u128) -> u128 {
        if self.to_integer {
            self.integer.cast_from_f32(f32::from_bits(input as u32))
        } else {
            self.integer.to_f32(input)
        }
    }
}

fn vectors(case: OperationCase) -> Vec<u128> {
    let (min, max) = case.integer.bounds();
    if !case.to_integer {
        let mut values = vec![min, min + 1, 0, 1, max - 1, max];
        for value in [
            -1,
            16_777_215,
            16_777_216,
            16_777_217,
            16_777_218,
            16_777_219,
            -16_777_217,
            -16_777_219,
        ] {
            if (min..=max).contains(&value) {
                values.push(value);
            }
        }
        return values
            .into_iter()
            .map(|n| (n as u128) & case.integer.mask())
            .collect();
    }
    let mut values = vec![
        0,
        0x8000_0000,
        1,
        0x8000_0001,
        0x3fe0_0000,
        0xbfe0_0000,
        0x7f80_0000,
        0xff80_0000,
        0x7fc0_0042,
        0xffc0_0042,
        0x7f80_0042,
        0x7f7f_ffff,
        0xff7f_ffff,
    ];
    for edge in [min as f32, max as f32] {
        let bits = edge.to_bits();
        values.push(bits);
        if bits > 0 {
            values.push(bits - 1);
        }
        values.push(bits + 1);
    }
    values.into_iter().map(u128::from).collect()
}

fn backing(
    ty: ScalarType,
    value: u128,
    len: usize,
) -> Result<(SimulationArgumentV1, SharedBufferV1), SourceFailure> {
    let width = usize::from(ty.bit_width().unwrap() / 8);
    let mut bytes = vec![0xa5; (len + 2 * GUARD_ELEMENTS) * width];
    bytes[(len + GUARD_ELEMENTS) * width..].fill(0x5a);
    for cell in
        bytes[GUARD_ELEMENTS * width..(GUARD_ELEMENTS + len) * width].chunks_exact_mut(width)
    {
        cell.copy_from_slice(&value.to_le_bytes()[..width]);
    }
    let buffer = BufferArgumentV1::new(
        ty,
        AccessMode::ReadWrite,
        width as u32,
        bytes.clone(),
        vec![true; bytes.len()],
        TARGET,
    )
    .map_err(failure)?;
    let view = BufferViewArgumentV1::new(
        BufferBackingIdV1(0),
        ty,
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

pub(super) fn scenarios(case: OperationCase) -> Result<Vec<Scenario>, SourceFailure> {
    let (input_ty, output_ty) = case.types();
    let values = std::iter::once(0).chain(vectors(case));
    values
        .enumerate()
        .map(|(ordinal, input)| {
            let len = if ordinal == 0 {
                0
            } else {
                [1, 63, 64, 65][(ordinal - 1) % 4]
            };
            let (output, initial) = backing(output_ty, 0x37, len)?;
            let expected = backing(output_ty, case.expected(input), len)?.1;
            Ok(Scenario {
                label: format!("cast-bits-{input:016x}-{ordinal}-len-{len}"),
                active: len,
                arguments: vec![
                    output,
                    SimulationArgumentV1::Scalar(
                        ScalarBitsV1::new(input_ty, input, TARGET).map_err(failure)?,
                    ),
                ],
                backings: vec![initial],
                expected: vec![expected],
                output_elements: len,
                written_elements: len,
            })
        })
        .collect()
}

pub(super) fn require_abi(
    module: &AdmittedSimulationModuleV1,
    case: OperationCase,
) -> Result<&Kernel, SourceFailure> {
    let [kernel] = module.module().kernels.as_slice() else {
        return Err(failure("numeric cast requires one actual O root"));
    };
    let function = module
        .module()
        .function(&kernel.entry)
        .ok_or_else(|| failure("numeric cast actual O entry"))?;
    let [Type::Slice(output), input] = function.signature.parameters.as_slice() else {
        return Err(failure("numeric cast output/scalar ABI"));
    };
    let (input_ty, output_ty) = case.types();
    if output.address_space != AddressSpace::Global
        || output.element.as_ref() != &Type::Scalar(output_ty)
        || output.access != AccessMode::ReadWrite
        || input != &Type::Scalar(input_ty)
        || !function.signature.results.is_empty()
        || kernel.domain.rank() != 1
    {
        return Err(failure("numeric cast exact signed/width ABI"));
    }
    let mut count = 0;
    for operation in module
        .module()
        .functions
        .iter()
        .filter_map(|f| f.body.as_ref())
        .flat_map(|body| &body.blocks)
        .flat_map(|block| &block.operations)
    {
        if let OperationKind::Cast {
            kind: kind @ (CastKind::IntegerToFloat | CastKind::FloatToInteger),
            to,
            ..
        } = &operation.kind
        {
            if *kind != case.kind()
                || *to != Type::Scalar(output_ty)
                || !matches!(operation.results.as_slice(), [value] if value.ty == Type::Scalar(output_ty))
            {
                return Err(failure("numeric cast unexpected actual O conversion"));
            }
            count += 1;
        }
    }
    if count != 1 {
        return Err(failure(
            "numeric cast must retain one dynamic actual O conversion",
        ));
    }
    Ok(kernel)
}

pub(super) fn check_native(case: OperationCase, llvm: &str) -> Result<(), SourceFailure> {
    let expected = if case.to_integer {
        format!(
            "@llvm.fpto{}i.sat.i{}.f32(float ",
            if case.integer.signed() { "s" } else { "u" },
            case.integer.width()
        )
    } else {
        format!(
            " = {}itofp i{} ",
            if case.integer.signed() { "s" } else { "u" },
            case.integer.width()
        )
    };
    if !llvm.contains(&expected) {
        return Err(failure("numeric cast exact native conversion missing"));
    }
    for flag in [
        " fast ", " nnan ", " ninf ", " nsz ", " fptosi ", " fptoui ",
    ] {
        if llvm.contains(flag) {
            return Err(failure(
                "numeric cast unchecked or relaxed native conversion",
            ));
        }
    }
    Ok(())
}

#[test]
fn cast_oracle_covers_rounding_saturation_sign_zero_nan_and_exact_rosters() {
    for integer in Integer::ALL {
        for to_integer in [false, true] {
            let case = OperationCase {
                integer,
                to_integer,
            };
            assert_eq!(OperationCase::parse(case.name()), Some(case));
            let rows = scenarios(case).unwrap();
            assert_eq!(rows[0].active, 0);
            assert!(rows.iter().any(|row| row.active == 65));
            assert!(check_native(case, "").is_err());
            if to_integer {
                for bits in [0, 0x8000_0000, 0x7fc0_0042, 0x7f80_0042, 0xffc0_0042] {
                    assert_eq!(case.expected(bits), 0);
                }
                let (min, max) = integer.bounds();
                assert_eq!(case.expected(0x7f80_0000), max as u128 & integer.mask());
                assert_eq!(case.expected(0xff80_0000), min as u128 & integer.mask());
            }
        }
    }
    assert_eq!(
        Integer::U64.to_f32(16_777_217),
        16_777_216_f32.to_bits() as u128
    );
    assert_eq!(
        Integer::U64.to_f32(16_777_219),
        16_777_220_f32.to_bits() as u128
    );
    assert_ne!(
        Integer::I32.to_f32(u32::MAX.into()),
        Integer::U32.to_f32(u32::MAX.into())
    );
    assert_ne!(
        Integer::U8.cast_from_f32(256.0),
        Integer::U16.cast_from_f32(256.0)
    );
}

#[test]
fn native_oracle_rejects_wrong_sign_width_kind_and_unchecked_conversion() {
    for integer in Integer::ALL {
        let down = OperationCase {
            integer,
            to_integer: true,
        };
        let exact = format!(
            "%r = call i{} @llvm.fpto{}i.sat.i{}.f32(float %x)",
            integer.width(),
            if integer.signed() { "s" } else { "u" },
            integer.width()
        );
        check_native(down, &exact).unwrap();
        for hostile in [
            exact.replace(".sat.", "."),
            exact.replace("f32", "f64"),
            exact.replace(
                if integer.signed() { "fptosi" } else { "fptoui" },
                if integer.signed() { "fptoui" } else { "fptosi" },
            ),
            format!("{exact}\n%bad = fptoui float %x to i32"),
        ] {
            assert!(check_native(down, &hostile).is_err());
        }
    }
}
