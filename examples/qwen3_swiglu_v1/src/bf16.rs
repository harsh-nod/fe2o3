//! Minimal exact BF16 storage conversion used by the host models.

/// One BF16 storage value represented by its complete bit pattern.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct Bf16V1(u16);

/// Fail-closed BF16 conversion errors.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Bf16ConversionErrorV1 {
    /// The source `f32` was NaN or infinite.
    NonFiniteInput,
    /// Rounding a finite `f32` produced a BF16 infinity.
    NonFiniteOutput,
}

impl Bf16V1 {
    /// Constructs a value from an uninterpreted BF16 bit pattern.
    pub const fn from_bits(bits: u16) -> Self {
        Self(bits)
    }

    /// Returns the complete BF16 bit pattern.
    pub const fn to_bits(self) -> u16 {
        self.0
    }

    /// Converts BF16 to `f32` exactly by placing its bits in the high half.
    pub const fn to_f32(self) -> f32 {
        f32::from_bits((self.0 as u32) << 16)
    }

    /// Returns whether this bit pattern represents a finite number.
    pub const fn is_finite(self) -> bool {
        self.0 & 0x7f80 != 0x7f80
    }

    /// Rounds a finite `f32` to BF16 using round-to-nearest, ties-to-even.
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
