//! Closed source MFMA profiles. This table does not authorize a target or policy.
use super::{Expansion, SemanticMfmaProfileV1, TrustedDeviceItem};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Format {
    Fp4,
    Fp8,
}

impl Format {
    pub(super) fn marker(self) -> TrustedDeviceItem {
        match self {
            Self::Fp4 => TrustedDeviceItem::Gfx950Fp4E2M1Format,
            Self::Fp8 => TrustedDeviceItem::Gfx950Fp8E4M3Format,
        }
    }

    pub(super) fn profile(self) -> SemanticMfmaProfileV1 {
        match self {
            Self::Fp4 => SemanticMfmaProfileV1::Fp4E2M1F32M16N16K128,
            Self::Fp8 => SemanticMfmaProfileV1::Fp8E4M3F32M16N16K128,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct MfmaFormats {
    pub lhs: Format,
    pub rhs: Format,
}

impl MfmaFormats {
    pub(super) fn for_expansion(expansion: Expansion) -> Option<Self> {
        let (lhs, rhs) = match expansion {
            Expansion::Gfx950Fp4MultiplyAccumulate => (Format::Fp4, Format::Fp4),
            Expansion::Gfx950Fp4Fp8MultiplyAccumulate => (Format::Fp4, Format::Fp8),
            Expansion::Gfx950Fp8MultiplyAccumulate => (Format::Fp8, Format::Fp8),
            _ => return None,
        };
        Some(Self { lhs, rhs })
    }

    pub(super) fn accumulator(self) -> Format {
        self.lhs
    }
}

#[test]
fn gfx950_matrix_homogeneous_and_mixed_profile_table_is_closed() {
    for (expansion, lhs, rhs) in [
        (
            Expansion::Gfx950Fp4MultiplyAccumulate,
            Format::Fp4,
            Format::Fp4,
        ),
        (
            Expansion::Gfx950Fp4Fp8MultiplyAccumulate,
            Format::Fp4,
            Format::Fp8,
        ),
        (
            Expansion::Gfx950Fp8MultiplyAccumulate,
            Format::Fp8,
            Format::Fp8,
        ),
    ] {
        let formats = MfmaFormats::for_expansion(expansion).unwrap();
        assert_eq!(formats, MfmaFormats { lhs, rhs });
        assert_eq!(formats.accumulator(), lhs);
    }
    for expansion in [
        Expansion::MatrixMultiplyAccumulate,
        Expansion::Gfx950MatrixContextCurrent,
        Expansion::Gfx950Fp4AccumulatorZero,
        Expansion::Gfx950Fp8AccumulatorZero,
        Expansion::Gfx950Fp4AccumulatorIntoValues,
        Expansion::Gfx950Fp8AccumulatorIntoValues,
        Expansion::Gfx950LdsTransposeReadB4,
        Expansion::Gfx950LdsTransposeReadB8,
    ] {
        assert_eq!(MfmaFormats::for_expansion(expansion), None);
    }
}
