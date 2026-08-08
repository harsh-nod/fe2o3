//! AMD FNUZ eight-bit floating-point storage values.
//!
//! [`Fp8E4M3Fnuz`] and [`Fp8E5M2Fnuz`] model the two FNUZ encodings
//! supported by `gfx942`. FNUZ uses one unsigned zero, reserves `0x80` as its
//! sole NaN, has no infinity, and gives the remaining 255 encodings finite
//! values. These are not the OCP E4M3 and E5M2 encodings.
//!
//! Conversions from `f32` use round-to-nearest, ties-to-even. Finite overflow
//! saturates to the largest finite value, matching the default
//! `__HIP_SATFINITE` behavior for ROCm's FNUZ types. Every `f32` NaN and both
//! infinities become the sole FNUZ NaN. Underflow and negative zero become the
//! unsigned FNUZ zero.
//!
//! These integer-backed value operations do not claim that a compiler has
//! selected a native `gfx942` conversion instruction.

use core::fmt;
use core::ops::Neg;

const FNUZ_NAN: u8 = 0x80;
const F32_ABS_MASK: u32 = 0x7fff_ffff;
const F32_EXPONENT_MASK: u32 = 0x7f80_0000;
const F32_FRACTION_MASK: u32 = 0x007f_ffff;
// This is the NaN bit pattern used by ROCm's software FNUZ widening path.
const F32_FNUZ_NAN: u32 = 0x7f80_0001;

/// A `gfx942` E4M3-FNUZ value stored in exactly eight bits.
///
/// The format has four exponent bits, three explicit fraction bits, and an
/// exponent bias of eight. Its finite range is `-240.0..=240.0`; the smallest
/// positive normal is `2^-7`, and the smallest positive subnormal is `2^-10`.
/// `0x00` is the only zero and `0x80` is the only NaN.
///
/// ```
/// use fe2o3_device::Fp8E4M3Fnuz;
///
/// let value = Fp8E4M3Fnuz::from_f32(1.5);
/// assert_eq!(value.to_bits(), 0x44);
/// assert_eq!(value.to_f32(), 1.5);
/// assert!(Fp8E4M3Fnuz::from_bits(0x80).is_nan());
/// ```
#[derive(Clone, Copy, Default)]
#[repr(transparent)]
pub struct Fp8E4M3Fnuz(u8);

impl Fp8E4M3Fnuz {
    /// Unsigned zero.
    pub const ZERO: Self = Self(0x00);
    /// The value one.
    pub const ONE: Self = Self(0x40);
    /// The format's sole NaN encoding.
    pub const NAN: Self = Self(FNUZ_NAN);
    /// Largest positive finite value, `240.0`.
    pub const MAX: Self = Self(0x7f);
    /// Most negative finite value, `-240.0`.
    pub const MIN: Self = Self(0xff);
    /// Smallest positive normal value, `2^-7`.
    pub const MIN_POSITIVE: Self = Self(0x08);
    /// Smallest positive subnormal value, `2^-10`.
    pub const MIN_POSITIVE_SUBNORMAL: Self = Self(0x01);

    /// Creates a value from its exact E4M3-FNUZ representation.
    ///
    /// Every bit pattern is valid. `0x80` represents NaN; every other bit
    /// pattern represents a finite number.
    pub const fn from_bits(bits: u8) -> Self {
        Self(bits)
    }

    /// Returns the exact E4M3-FNUZ representation.
    pub const fn to_bits(self) -> u8 {
        self.0
    }

    /// Narrows using round-to-nearest, ties-to-even and finite saturation.
    pub const fn from_f32(value: f32) -> Self {
        Self(f32_to_fnuz(value.to_bits(), 3, 8, 0x4370_0000))
    }

    /// Widens exactly to `f32`, with a canonical NaN for the sole NaN value.
    pub const fn to_f32(self) -> f32 {
        f32::from_bits(fnuz_to_f32(self.0, 3, 8))
    }

    /// Returns whether this value is the sole FNUZ NaN.
    pub const fn is_nan(self) -> bool {
        self.0 == FNUZ_NAN
    }

    /// Returns `false`; E4M3-FNUZ has no infinity encoding.
    pub const fn is_infinite(self) -> bool {
        false
    }

    /// Returns whether this value is finite.
    pub const fn is_finite(self) -> bool {
        !self.is_nan()
    }

    /// Returns whether this value is the unsigned FNUZ zero.
    pub const fn is_zero(self) -> bool {
        self.0 == 0
    }

    /// Returns whether this value is finite and subnormal.
    pub const fn is_subnormal(self) -> bool {
        let magnitude = self.0 & 0x7f;
        magnitude != 0 && magnitude < 0x08
    }

    /// Returns whether this value is finite and normal.
    pub const fn is_normal(self) -> bool {
        !self.is_nan() && (self.0 & 0x78) != 0
    }

    /// Returns whether the sign bit is set.
    ///
    /// The result is also true for the `0x80` NaN. FNUZ has no negative zero.
    pub const fn is_sign_negative(self) -> bool {
        (self.0 & 0x80) != 0
    }

    /// Returns the absolute value while preserving the sole NaN encoding.
    pub const fn abs(self) -> Self {
        if self.is_nan() {
            self
        } else {
            Self(self.0 & 0x7f)
        }
    }
}

/// A `gfx942` E5M2-FNUZ value stored in exactly eight bits.
///
/// The format has five exponent bits, two explicit fraction bits, and an
/// exponent bias of sixteen. Its finite range is `-57344.0..=57344.0`; the
/// smallest positive normal is `2^-15`, and the smallest positive subnormal is
/// `2^-17`. `0x00` is the only zero and `0x80` is the only NaN.
///
/// ```
/// use fe2o3_device::Fp8E5M2Fnuz;
///
/// let value = Fp8E5M2Fnuz::from_f32(1.5);
/// assert_eq!(value.to_bits(), 0x42);
/// assert_eq!(value.to_f32(), 1.5);
/// assert!(Fp8E5M2Fnuz::from_bits(0x80).is_nan());
/// ```
#[derive(Clone, Copy, Default)]
#[repr(transparent)]
pub struct Fp8E5M2Fnuz(u8);

impl Fp8E5M2Fnuz {
    /// Unsigned zero.
    pub const ZERO: Self = Self(0x00);
    /// The value one.
    pub const ONE: Self = Self(0x40);
    /// The format's sole NaN encoding.
    pub const NAN: Self = Self(FNUZ_NAN);
    /// Largest positive finite value, `57344.0`.
    pub const MAX: Self = Self(0x7f);
    /// Most negative finite value, `-57344.0`.
    pub const MIN: Self = Self(0xff);
    /// Smallest positive normal value, `2^-15`.
    pub const MIN_POSITIVE: Self = Self(0x04);
    /// Smallest positive subnormal value, `2^-17`.
    pub const MIN_POSITIVE_SUBNORMAL: Self = Self(0x01);

    /// Creates a value from its exact E5M2-FNUZ representation.
    ///
    /// Every bit pattern is valid. `0x80` represents NaN; every other bit
    /// pattern represents a finite number.
    pub const fn from_bits(bits: u8) -> Self {
        Self(bits)
    }

    /// Returns the exact E5M2-FNUZ representation.
    pub const fn to_bits(self) -> u8 {
        self.0
    }

    /// Narrows using round-to-nearest, ties-to-even and finite saturation.
    pub const fn from_f32(value: f32) -> Self {
        Self(f32_to_fnuz(value.to_bits(), 2, 16, 0x4760_0000))
    }

    /// Widens exactly to `f32`, with a canonical NaN for the sole NaN value.
    pub const fn to_f32(self) -> f32 {
        f32::from_bits(fnuz_to_f32(self.0, 2, 16))
    }

    /// Returns whether this value is the sole FNUZ NaN.
    pub const fn is_nan(self) -> bool {
        self.0 == FNUZ_NAN
    }

    /// Returns `false`; E5M2-FNUZ has no infinity encoding.
    pub const fn is_infinite(self) -> bool {
        false
    }

    /// Returns whether this value is finite.
    pub const fn is_finite(self) -> bool {
        !self.is_nan()
    }

    /// Returns whether this value is the unsigned FNUZ zero.
    pub const fn is_zero(self) -> bool {
        self.0 == 0
    }

    /// Returns whether this value is finite and subnormal.
    pub const fn is_subnormal(self) -> bool {
        let magnitude = self.0 & 0x7f;
        magnitude != 0 && magnitude < 0x04
    }

    /// Returns whether this value is finite and normal.
    pub const fn is_normal(self) -> bool {
        !self.is_nan() && (self.0 & 0x7c) != 0
    }

    /// Returns whether the sign bit is set.
    ///
    /// The result is also true for the `0x80` NaN. FNUZ has no negative zero.
    pub const fn is_sign_negative(self) -> bool {
        (self.0 & 0x80) != 0
    }

    /// Returns the absolute value while preserving the sole NaN encoding.
    pub const fn abs(self) -> Self {
        if self.is_nan() {
            self
        } else {
            Self(self.0 & 0x7f)
        }
    }
}

const fn f32_to_fnuz(bits: u32, mantissa_bits: u32, bias: i32, max_finite: u32) -> u8 {
    let magnitude = bits & F32_ABS_MASK;
    if magnitude >= F32_EXPONENT_MASK {
        return FNUZ_NAN;
    }
    if magnitude == 0 {
        return 0;
    }

    let sign = ((bits >> 24) & 0x80) as u8;
    if magnitude > max_finite {
        return sign | 0x7f;
    }

    let exponent = ((magnitude >> 23) & 0xff) as i32;
    let actual_exponent = exponent - 127;
    let significand = (magnitude & F32_FRACTION_MASK) | 0x0080_0000;
    let minimum_normal_exponent = 1 - bias;
    let encoded_magnitude = if actual_exponent < minimum_normal_exponent {
        let shift = (24 - bias - mantissa_bits as i32 - actual_exponent) as u32;
        let rounded = round_shift_right_ties_even(significand as u64, shift);
        if rounded == 0 {
            return 0;
        }
        rounded as u8
    } else {
        let mut encoded_exponent = actual_exponent + bias;
        let mut rounded = round_shift_right_ties_even(significand as u64, 23 - mantissa_bits);
        if rounded == (1_u64 << (mantissa_bits + 1)) {
            rounded >>= 1;
            encoded_exponent += 1;
        }
        ((encoded_exponent as u8) << mantissa_bits)
            | ((rounded as u8) & ((1_u8 << mantissa_bits) - 1))
    };

    sign | encoded_magnitude
}

const fn fnuz_to_f32(bits: u8, mantissa_bits: u32, bias: i32) -> u32 {
    if bits == FNUZ_NAN {
        return F32_FNUZ_NAN;
    }
    if bits == 0 {
        return 0;
    }

    let sign = ((bits as u32) & 0x80) << 24;
    let mantissa_mask = (1_u8 << mantissa_bits) - 1;
    let mut fraction = bits & mantissa_mask;
    let exponent = ((bits & 0x7f) >> mantissa_bits) as i32;

    if exponent == 0 {
        let mut actual_exponent = 1 - bias;
        while (fraction & (1_u8 << mantissa_bits)) == 0 {
            fraction <<= 1;
            actual_exponent -= 1;
        }
        fraction &= mantissa_mask;
        sign | (((actual_exponent + 127) as u32) << 23)
            | ((fraction as u32) << (23 - mantissa_bits))
    } else {
        sign | (((exponent - bias + 127) as u32) << 23)
            | ((fraction as u32) << (23 - mantissa_bits))
    }
}

const fn round_shift_right_ties_even(value: u64, shift: u32) -> u64 {
    if shift > 64 {
        return 0;
    }
    if shift == 64 {
        let halfway = 1_u64 << 63;
        return if value > halfway { 1 } else { 0 };
    }

    let truncated = value >> shift;
    let mask = (1_u64 << shift) - 1;
    let remainder = value & mask;
    let halfway = 1_u64 << (shift - 1);
    if remainder > halfway || (remainder == halfway && (truncated & 1) != 0) {
        truncated + 1
    } else {
        truncated
    }
}

macro_rules! impl_fnuz_value {
    ($ty:ty) => {
        impl From<f32> for $ty {
            fn from(value: f32) -> Self {
                Self::from_f32(value)
            }
        }

        impl From<$ty> for f32 {
            fn from(value: $ty) -> Self {
                value.to_f32()
            }
        }

        impl PartialEq for $ty {
            fn eq(&self, other: &Self) -> bool {
                self.to_f32() == other.to_f32()
            }
        }

        impl PartialOrd for $ty {
            fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
                self.to_f32().partial_cmp(&other.to_f32())
            }
        }

        impl Neg for $ty {
            type Output = Self;

            fn neg(self) -> Self::Output {
                if self.is_nan() || self.is_zero() {
                    self
                } else {
                    Self::from_bits(self.to_bits() ^ 0x80)
                }
            }
        }

        impl fmt::Debug for $ty {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter
                    .debug_tuple(stringify!($ty))
                    .field(&self.to_f32())
                    .finish()
            }
        }
    };
}

impl_fnuz_value!(Fp8E4M3Fnuz);
impl_fnuz_value!(Fp8E5M2Fnuz);
