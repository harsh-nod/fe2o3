//! Exact BF16 storage conversion for host references.

/// One BF16 storage value represented by all 16 bits.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct Bf16V1(u16);

/// Fail-closed BF16 conversion error.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Bf16ConversionErrorV1 {
    /// Source FP32 was NaN or infinity.
    NonFiniteInput,
    /// Rounding finite FP32 produced BF16 infinity.
    NonFiniteOutput,
}

impl Bf16V1 {
    /// Constructs from uninterpreted BF16 bits.
    pub const fn from_bits(bits: u16) -> Self {
        Self(bits)
    }

    /// Returns all BF16 bits.
    pub const fn to_bits(self) -> u16 {
        self.0
    }

    /// Converts exactly to FP32 by filling the low 16 bits with zero.
    pub const fn to_f32(self) -> f32 {
        f32::from_bits((self.0 as u32) << 16)
    }

    /// Returns whether the represented value is finite.
    pub const fn is_finite(self) -> bool {
        self.0 & 0x7f80 != 0x7f80
    }

    /// Converts finite FP32 to BF16 round-to-nearest, ties-to-even.
    pub fn from_f32_rne(value: f32) -> Result<Self, Bf16ConversionErrorV1> {
        if !value.is_finite() {
            return Err(Bf16ConversionErrorV1::NonFiniteInput);
        }
        let bits = value.to_bits();
        let rounded = bits.wrapping_add(0x7fff + ((bits >> 16) & 1));
        let output = Self((rounded >> 16) as u16);
        if !output.is_finite() {
            return Err(Bf16ConversionErrorV1::NonFiniteOutput);
        }
        Ok(output)
    }
}
