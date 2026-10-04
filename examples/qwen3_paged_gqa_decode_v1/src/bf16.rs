//! Exact BF16 storage conversion for the host models.

/// One BF16 storage value represented by all 16 bits.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct Bf16V1(u16);

/// Fail-closed BF16 conversion error.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Bf16ConversionErrorV1 {
    /// The source `f32` was NaN or infinite.
    NonFiniteInput,
    /// Rounding a finite `f32` produced BF16 infinity.
    NonFiniteOutput,
}

impl Bf16V1 {
    /// Constructs a value from an uninterpreted BF16 bit pattern.
    pub const fn from_bits(bits: u16) -> Self {
        Self(bits)
    }

    /// Returns all BF16 bits.
    pub const fn to_bits(self) -> u16 {
        self.0
    }

    /// Converts BF16 exactly to `f32` by placing it in the high 16 bits.
    pub const fn to_f32(self) -> f32 {
        f32::from_bits((self.0 as u32) << 16)
    }

    /// Returns whether the represented value is finite.
    pub const fn is_finite(self) -> bool {
        self.0 & 0x7f80 != 0x7f80
    }

    /// Converts finite `f32` to BF16 with round-to-nearest, ties-to-even.
    pub fn from_f32_rne(value: f32) -> Result<Self, Bf16ConversionErrorV1> {
        if !value.is_finite() {
            return Err(Bf16ConversionErrorV1::NonFiniteInput);
        }
        let bits = value.to_bits();
        let retained_lsb = (bits >> 16) & 1;
        let rounded = bits.wrapping_add(0x7fff + retained_lsb);
        let output = Self((rounded >> 16) as u16);
        if !output.is_finite() {
            return Err(Bf16ConversionErrorV1::NonFiniteOutput);
        }
        Ok(output)
    }
}
