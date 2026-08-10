//! OCP microscaling exponent storage values.
//!
//! [`MxScaleE8M0`] models the unsigned E8M0 scale encoding used by OCP MX
//! formats. It is a storage and conversion contract only. A target must also
//! admit the corresponding format through the canonical target capability
//! model before code can claim a native MX operation.

use core::fmt;

/// Why an `f32` cannot be represented exactly as an E8M0 scale.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum MxScaleConversionError {
    /// E8M0 has no zero encoding.
    Zero,
    /// E8M0 scales are unsigned.
    Negative,
    /// Infinity is not representable.
    Infinite,
    /// NaN must be constructed explicitly with [`MxScaleE8M0::NAN`].
    Nan,
    /// The finite value is not an exact power of two in the E8M0 range.
    NotPowerOfTwo,
}

impl fmt::Display for MxScaleConversionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Zero => "E8M0 has no zero encoding",
            Self::Negative => "E8M0 scales cannot be negative",
            Self::Infinite => "E8M0 has no infinity encoding",
            Self::Nan => "NaN requires the explicit E8M0 NaN encoding",
            Self::NotPowerOfTwo => "E8M0 requires an exactly representable power-of-two scale",
        })
    }
}

impl core::error::Error for MxScaleConversionError {}

/// An OCP MX unsigned E8M0 scale stored in exactly eight bits.
///
/// Encodings `0x00..=0xfe` represent `2^(bits - 127)`. `0xff` is the sole NaN
/// encoding. E8M0 has no zero, sign, infinity, or fractional significand.
#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct MxScaleE8M0(u8);

impl MxScaleE8M0 {
    /// Smallest finite scale, `2^-127`.
    pub const MIN: Self = Self(0x00);
    /// The value one.
    pub const ONE: Self = Self(0x7f);
    /// Largest finite scale, `2^127`.
    pub const MAX: Self = Self(0xfe);
    /// The sole NaN encoding.
    pub const NAN: Self = Self(0xff);

    /// Constructs a value from its exact E8M0 representation.
    pub const fn from_bits(bits: u8) -> Self {
        Self(bits)
    }

    /// Returns the exact E8M0 representation.
    pub const fn to_bits(self) -> u8 {
        self.0
    }

    /// Returns whether this is the sole E8M0 NaN encoding.
    pub const fn is_nan(self) -> bool {
        self.0 == 0xff
    }

    /// Returns the unbiased base-two exponent of a finite scale.
    pub const fn exponent(self) -> Option<i16> {
        if self.is_nan() {
            None
        } else {
            Some(self.0 as i16 - 127)
        }
    }

    /// Widens exactly to `f32`.
    ///
    /// The minimum scale `2^-127` is exactly representable as an `f32`
    /// subnormal. The NaN encoding widens to the canonical positive quiet NaN.
    pub const fn to_f32(self) -> f32 {
        if self.is_nan() {
            f32::from_bits(0x7fc0_0000)
        } else if self.0 == 0 {
            f32::from_bits(0x0040_0000)
        } else {
            f32::from_bits((self.0 as u32) << 23)
        }
    }

    /// Converts an exactly representable positive power of two to E8M0.
    ///
    /// This operation rejects rounding, saturation, zero, sign changes, and
    /// special-value canonicalization. Callers must choose those policies
    /// explicitly before constructing a scale.
    pub const fn try_from_f32(value: f32) -> Result<Self, MxScaleConversionError> {
        let bits = value.to_bits();
        let magnitude = bits & 0x7fff_ffff;
        let exponent = ((bits >> 23) & 0xff) as u8;
        let fraction = bits & 0x007f_ffff;

        if magnitude > 0x7f80_0000 {
            return Err(MxScaleConversionError::Nan);
        }
        if magnitude == 0x7f80_0000 {
            return Err(MxScaleConversionError::Infinite);
        }
        if bits & 0x8000_0000 != 0 {
            return Err(MxScaleConversionError::Negative);
        }
        if magnitude == 0 {
            return Err(MxScaleConversionError::Zero);
        }
        if exponent == 0 {
            return if fraction == 0x0040_0000 {
                Ok(Self::MIN)
            } else {
                Err(MxScaleConversionError::NotPowerOfTwo)
            };
        }
        if fraction != 0 {
            return Err(MxScaleConversionError::NotPowerOfTwo);
        }
        Ok(Self(exponent))
    }

    /// Returns host-rustc layout facts for ABI evidence.
    #[doc(hidden)]
    pub const fn __fe2o3_rust_layout_v1() -> (usize, usize) {
        (core::mem::size_of::<Self>(), core::mem::align_of::<Self>())
    }
}

impl TryFrom<f32> for MxScaleE8M0 {
    type Error = MxScaleConversionError;

    fn try_from(value: f32) -> Result<Self, Self::Error> {
        Self::try_from_f32(value)
    }
}

impl From<MxScaleE8M0> for f32 {
    fn from(value: MxScaleE8M0) -> Self {
        value.to_f32()
    }
}

impl fmt::Debug for MxScaleE8M0 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("MxScaleE8M0")
            .field(&self.to_f32())
            .finish()
    }
}

/// Four E8M0 scales packed into one 32-bit storage value.
///
/// Lane zero occupies bits 0..8 and lane three occupies bits 24..32. The lane
/// order is defined by bit position and is independent of host byte order.
#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct MxScaleE8M0x4(u32);

impl MxScaleE8M0x4 {
    /// Four NaN scales, used as the fail-closed default.
    pub const NAN: Self = Self(0xffff_ffff);

    /// Packs four scales in increasing lane-index order.
    pub const fn from_array(scales: [MxScaleE8M0; 4]) -> Self {
        Self(
            scales[0].to_bits() as u32
                | ((scales[1].to_bits() as u32) << 8)
                | ((scales[2].to_bits() as u32) << 16)
                | ((scales[3].to_bits() as u32) << 24),
        )
    }

    /// Constructs a packed value from its exact representation.
    pub const fn from_bits(bits: u32) -> Self {
        Self(bits)
    }

    /// Returns the exact packed representation.
    pub const fn to_bits(self) -> u32 {
        self.0
    }

    /// Returns one scale lane, or `None` for an out-of-bounds index.
    pub const fn lane(self, index: usize) -> Option<MxScaleE8M0> {
        if index < 4 {
            Some(MxScaleE8M0::from_bits((self.0 >> (index * 8)) as u8))
        } else {
            None
        }
    }

    /// Returns all scales in increasing lane-index order.
    pub const fn to_array(self) -> [MxScaleE8M0; 4] {
        [
            MxScaleE8M0::from_bits(self.0 as u8),
            MxScaleE8M0::from_bits((self.0 >> 8) as u8),
            MxScaleE8M0::from_bits((self.0 >> 16) as u8),
            MxScaleE8M0::from_bits((self.0 >> 24) as u8),
        ]
    }
}

impl Default for MxScaleE8M0x4 {
    fn default() -> Self {
        Self::NAN
    }
}

impl fmt::Debug for MxScaleE8M0x4 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("MxScaleE8M0x4")
            .field(&self.to_array())
            .finish()
    }
}
