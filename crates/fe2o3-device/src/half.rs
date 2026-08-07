//! IEEE 754 binary16 and bfloat16 storage values.
//!
//! These types have stable integer-backed layouts and deterministic
//! round-to-nearest, ties-to-even conversions. Their ordinary arithmetic is
//! explicitly widened through `f32`; it is a portable value operation, not a
//! claim that the selected GPU has native 16-bit arithmetic.

use core::fmt;
use core::ops::{Add, Div, Mul, Neg, Sub};

const SIGN_MASK: u16 = 0x8000;

/// An IEEE 754 binary16 value stored in exactly 16 bits.
#[derive(Clone, Copy, Default)]
#[repr(transparent)]
#[rustc_diagnostic_item = "fe2o3_device_f16_v1"]
pub struct F16(u16);

impl F16 {
    pub const ZERO: Self = Self(0x0000);
    pub const NEG_ZERO: Self = Self(0x8000);
    pub const ONE: Self = Self(0x3c00);
    pub const INFINITY: Self = Self(0x7c00);
    pub const NEG_INFINITY: Self = Self(0xfc00);
    pub const NAN: Self = Self(0x7e00);
    pub const MAX: Self = Self(0x7bff);
    pub const MIN: Self = Self(0xfbff);
    pub const MIN_POSITIVE: Self = Self(0x0400);
    pub const MIN_POSITIVE_SUBNORMAL: Self = Self(0x0001);

    /// Creates a value from its exact IEEE binary16 representation.
    pub const fn from_bits(bits: u16) -> Self {
        Self(bits)
    }

    /// Returns the exact IEEE binary16 representation.
    pub const fn to_bits(self) -> u16 {
        self.0
    }

    /// Narrows `value` using round-to-nearest, ties-to-even.
    ///
    /// NaNs retain their sign and the high payload bits and are quieted. A
    /// payload is never converted to infinity.
    pub const fn from_f32(value: f32) -> Self {
        let bits = value.to_bits();
        let sign = ((bits >> 16) as u16) & SIGN_MASK;
        let exponent = ((bits >> 23) & 0xff) as i32;
        let fraction = bits & 0x007f_ffff;

        if exponent == 0xff {
            if fraction == 0 {
                return Self(sign | 0x7c00);
            }
            let payload = ((fraction >> 13) as u16) | 0x0200;
            return Self(sign | 0x7c00 | payload);
        }

        let mut half_exponent = exponent - 127 + 15;
        if half_exponent >= 0x1f {
            return Self(sign | 0x7c00);
        }

        if half_exponent <= 0 {
            if half_exponent < -10 {
                return Self(sign);
            }
            let significand = fraction | 0x0080_0000;
            let rounded = round_shift_right_ties_even(significand, (14 - half_exponent) as u32);
            return Self(sign | rounded as u16);
        }

        let rounded_fraction = round_shift_right_ties_even(fraction, 13);
        if rounded_fraction == 0x0400 {
            half_exponent += 1;
            if half_exponent >= 0x1f {
                return Self(sign | 0x7c00);
            }
            return Self(sign | ((half_exponent as u16) << 10));
        }

        Self(sign | ((half_exponent as u16) << 10) | rounded_fraction as u16)
    }

    /// Widens exactly to `f32`.
    pub const fn to_f32(self) -> f32 {
        let sign = ((self.0 & SIGN_MASK) as u32) << 16;
        let exponent = (self.0 >> 10) & 0x1f;
        let mut fraction = self.0 & 0x03ff;

        let bits = if exponent == 0 {
            if fraction == 0 {
                sign
            } else {
                let mut unbiased_exponent = -14_i32;
                while (fraction & 0x0400) == 0 {
                    fraction <<= 1;
                    unbiased_exponent -= 1;
                }
                fraction &= 0x03ff;
                sign | (((unbiased_exponent + 127) as u32) << 23) | ((fraction as u32) << 13)
            }
        } else if exponent == 0x1f {
            sign | 0x7f80_0000 | ((fraction as u32) << 13)
        } else {
            sign | ((((exponent as i32) - 15 + 127) as u32) << 23) | ((fraction as u32) << 13)
        };

        f32::from_bits(bits)
    }

    pub const fn is_nan(self) -> bool {
        (self.0 & 0x7c00) == 0x7c00 && (self.0 & 0x03ff) != 0
    }

    pub const fn is_infinite(self) -> bool {
        (self.0 & 0x7fff) == 0x7c00
    }

    pub const fn is_finite(self) -> bool {
        (self.0 & 0x7c00) != 0x7c00
    }

    pub const fn is_subnormal(self) -> bool {
        (self.0 & 0x7c00) == 0 && (self.0 & 0x03ff) != 0
    }

    pub const fn is_sign_negative(self) -> bool {
        (self.0 & SIGN_MASK) != 0
    }

    pub const fn abs(self) -> Self {
        Self(self.0 & !SIGN_MASK)
    }

    /// Computes a fused `f32` multiply-add and narrows the result to binary16.
    ///
    /// The operation is intentionally named `widened`: it specifies the
    /// sequence used by a non-native fallback and does not assert native
    /// binary16 FMA semantics.
    pub fn mul_add_widened(self, multiplier: Self, addend: Self) -> Self {
        Self::from_f32(core::f32::math::mul_add(
            self.to_f32(),
            multiplier.to_f32(),
            addend.to_f32(),
        ))
    }
}

/// A bfloat16 value stored in exactly 16 bits.
#[derive(Clone, Copy, Default)]
#[repr(transparent)]
#[rustc_diagnostic_item = "fe2o3_device_bf16_v1"]
pub struct Bf16(u16);

impl Bf16 {
    pub const ZERO: Self = Self(0x0000);
    pub const NEG_ZERO: Self = Self(0x8000);
    pub const ONE: Self = Self(0x3f80);
    pub const INFINITY: Self = Self(0x7f80);
    pub const NEG_INFINITY: Self = Self(0xff80);
    pub const NAN: Self = Self(0x7fc0);
    pub const MAX: Self = Self(0x7f7f);
    pub const MIN: Self = Self(0xff7f);
    pub const MIN_POSITIVE: Self = Self(0x0080);
    pub const MIN_POSITIVE_SUBNORMAL: Self = Self(0x0001);

    /// Creates a value from its exact bfloat16 representation.
    pub const fn from_bits(bits: u16) -> Self {
        Self(bits)
    }

    /// Returns the exact bfloat16 representation.
    pub const fn to_bits(self) -> u16 {
        self.0
    }

    /// Narrows `value` using round-to-nearest, ties-to-even.
    ///
    /// NaNs retain their sign and high payload bits and are quieted. A payload
    /// held only in discarded bits is still represented as a NaN.
    pub const fn from_f32(value: f32) -> Self {
        let bits = value.to_bits();
        if (bits & 0x7f80_0000) == 0x7f80_0000 && (bits & 0x007f_ffff) != 0 {
            return Self(((bits >> 16) as u16) | 0x0040);
        }

        let retained_lsb = (bits >> 16) & 1;
        Self(((bits + 0x7fff + retained_lsb) >> 16) as u16)
    }

    /// Widens exactly to `f32`.
    pub const fn to_f32(self) -> f32 {
        f32::from_bits((self.0 as u32) << 16)
    }

    pub const fn is_nan(self) -> bool {
        (self.0 & 0x7f80) == 0x7f80 && (self.0 & 0x007f) != 0
    }

    pub const fn is_infinite(self) -> bool {
        (self.0 & 0x7fff) == 0x7f80
    }

    pub const fn is_finite(self) -> bool {
        (self.0 & 0x7f80) != 0x7f80
    }

    pub const fn is_subnormal(self) -> bool {
        (self.0 & 0x7f80) == 0 && (self.0 & 0x007f) != 0
    }

    pub const fn is_sign_negative(self) -> bool {
        (self.0 & SIGN_MASK) != 0
    }

    pub const fn abs(self) -> Self {
        Self(self.0 & !SIGN_MASK)
    }

    /// Computes a fused `f32` multiply-add and narrows the result to bfloat16.
    ///
    /// This is the documented non-native fallback sequence used by
    /// [`Bf16x2::mul_add_widened`].
    pub fn mul_add_widened(self, multiplier: Self, addend: Self) -> Self {
        Self::from_f32(core::f32::math::mul_add(
            self.to_f32(),
            multiplier.to_f32(),
            addend.to_f32(),
        ))
    }
}

/// Two bfloat16 lanes packed into one 32-bit ALU value.
///
/// Lane zero occupies bits 0..16 and lane one occupies bits 16..32.
#[derive(Clone, Copy, Default)]
#[repr(transparent)]
#[rustc_diagnostic_item = "fe2o3_device_bf16x2_v1"]
pub struct Bf16x2(u32);

impl Bf16x2 {
    pub const ZERO: Self = Self(0);

    pub const fn new(lane0: Bf16, lane1: Bf16) -> Self {
        Self((lane0.to_bits() as u32) | ((lane1.to_bits() as u32) << 16))
    }

    pub const fn from_bits(bits: u32) -> Self {
        Self(bits)
    }

    pub const fn to_bits(self) -> u32 {
        self.0
    }

    pub const fn lane0(self) -> Bf16 {
        Bf16::from_bits(self.0 as u16)
    }

    pub const fn lane1(self) -> Bf16 {
        Bf16::from_bits((self.0 >> 16) as u16)
    }

    pub const fn to_array(self) -> [Bf16; 2] {
        [self.lane0(), self.lane1()]
    }

    /// Performs independent fused `f32` multiply-adds and narrows each lane.
    ///
    /// This is a portable reference/fallback sequence. The device intrinsic
    /// surface in [`crate::math::DeviceMath`] remains target-gated and must be
    /// lowered only after the backend checks the selected architecture.
    pub fn mul_add_widened(self, multiplier: Self, addend: Self) -> Self {
        Self::new(
            self.lane0()
                .mul_add_widened(multiplier.lane0(), addend.lane0()),
            self.lane1()
                .mul_add_widened(multiplier.lane1(), addend.lane1()),
        )
    }
}

const fn round_shift_right_ties_even(value: u32, shift: u32) -> u32 {
    let truncated = value >> shift;
    let mask = (1_u32 << shift) - 1;
    let remainder = value & mask;
    let halfway = 1_u32 << (shift - 1);
    if remainder > halfway || (remainder == halfway && (truncated & 1) != 0) {
        truncated + 1
    } else {
        truncated
    }
}

macro_rules! impl_float_value {
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

        impl fmt::Debug for $ty {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter
                    .debug_tuple(stringify!($ty))
                    .field(&self.to_f32())
                    .finish()
            }
        }

        impl Add for $ty {
            type Output = Self;

            fn add(self, rhs: Self) -> Self::Output {
                Self::from_f32(self.to_f32() + rhs.to_f32())
            }
        }

        impl Sub for $ty {
            type Output = Self;

            fn sub(self, rhs: Self) -> Self::Output {
                Self::from_f32(self.to_f32() - rhs.to_f32())
            }
        }

        impl Mul for $ty {
            type Output = Self;

            fn mul(self, rhs: Self) -> Self::Output {
                Self::from_f32(self.to_f32() * rhs.to_f32())
            }
        }

        impl Div for $ty {
            type Output = Self;

            fn div(self, rhs: Self) -> Self::Output {
                Self::from_f32(self.to_f32() / rhs.to_f32())
            }
        }

        impl Neg for $ty {
            type Output = Self;

            fn neg(self) -> Self::Output {
                Self::from_bits(self.to_bits() ^ SIGN_MASK)
            }
        }
    };
}

impl_float_value!(F16);
impl_float_value!(Bf16);

impl PartialEq for Bf16x2 {
    fn eq(&self, other: &Self) -> bool {
        self.lane0() == other.lane0() && self.lane1() == other.lane1()
    }
}

impl fmt::Debug for Bf16x2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("Bf16x2")
            .field(&self.to_array())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::{Bf16, Bf16x2, F16};
    use core::mem::{align_of, size_of};

    #[test]
    fn representations_are_stable() {
        assert_eq!(size_of::<F16>(), 2);
        assert_eq!(align_of::<F16>(), 2);
        assert_eq!(size_of::<Bf16>(), 2);
        assert_eq!(align_of::<Bf16>(), 2);
        assert_eq!(size_of::<Bf16x2>(), 4);
        assert_eq!(align_of::<Bf16x2>(), 4);
    }

    #[test]
    fn f16_known_patterns_and_boundaries() {
        let cases = [
            (0.0, 0x0000),
            (-0.0, 0x8000),
            (1.0, 0x3c00),
            (-2.0, 0xc000),
            (65_504.0, 0x7bff),
            (2.0_f32.powi(-14), 0x0400),
            (2.0_f32.powi(-24), 0x0001),
            (f32::INFINITY, 0x7c00),
            (f32::NEG_INFINITY, 0xfc00),
        ];
        for (value, bits) in cases {
            assert_eq!(F16::from_f32(value).to_bits(), bits);
            assert_eq!(F16::from_bits(bits).to_f32().to_bits(), value.to_bits());
        }

        assert_eq!(F16::from_f32(f32::from_bits(0x3f80_1000)).to_bits(), 0x3c00);
        assert_eq!(F16::from_f32(f32::from_bits(0x3f80_3000)).to_bits(), 0x3c02);
        assert_eq!(F16::from_f32(2.0_f32.powi(-25)).to_bits(), 0x0000);
        assert_eq!(F16::from_f32(3.0 * 2.0_f32.powi(-25)).to_bits(), 0x0002);
    }

    #[test]
    fn f16_round_trips_every_bit_pattern() {
        for bits in 0_u16..=u16::MAX {
            let value = F16::from_bits(bits);
            let round_trip = F16::from_f32(value.to_f32()).to_bits();
            let expected = if value.is_nan() { bits | 0x0200 } else { bits };
            assert_eq!(round_trip, expected, "failed at {bits:#06x}");
        }
    }

    #[test]
    fn bf16_rounds_ties_even_and_preserves_nan_class() {
        assert_eq!(Bf16::from_f32(1.0).to_bits(), 0x3f80);
        assert_eq!(
            Bf16::from_f32(f32::from_bits(0x3f80_8000)).to_bits(),
            0x3f80
        );
        assert_eq!(
            Bf16::from_f32(f32::from_bits(0x3f81_8000)).to_bits(),
            0x3f82
        );
        assert!(Bf16::from_f32(f32::from_bits(0x7f80_0001)).is_nan());
        assert!(Bf16::from_f32(f32::from_bits(0xff80_0001)).is_nan());
    }

    #[test]
    fn bf16_round_trips_every_bit_pattern() {
        for bits in 0_u16..=u16::MAX {
            let value = Bf16::from_bits(bits);
            let round_trip = Bf16::from_f32(value.to_f32()).to_bits();
            let expected = if value.is_nan() { bits | 0x0040 } else { bits };
            assert_eq!(round_trip, expected, "failed at {bits:#06x}");
        }
    }

    #[test]
    fn arithmetic_has_explicit_widen_then_narrow_semantics() {
        let one = F16::ONE;
        let two = F16::from_f32(2.0);
        let three = F16::from_f32(3.0);
        assert_eq!((one + two).to_bits(), three.to_bits());
        assert_eq!((three - one).to_bits(), two.to_bits());
        assert_eq!((two * three).to_f32(), 6.0);
        assert_eq!((three / two).to_f32(), 1.5);
        assert_eq!((-one).to_bits(), 0xbc00);

        let a = Bf16::from_f32(1.5);
        let b = Bf16::from_f32(2.0);
        let c = Bf16::from_f32(-1.0);
        assert_eq!(a.mul_add_widened(b, c).to_f32(), 2.0);
    }

    #[test]
    fn packed_lane_order_and_fma_are_stable() {
        let lanes = Bf16x2::new(Bf16::from_f32(1.5), Bf16::from_f32(-2.25));
        assert_eq!(lanes.to_bits(), 0xc010_3fc0);
        assert_eq!(lanes.lane0().to_f32(), 1.5);
        assert_eq!(lanes.lane1().to_f32(), -2.25);

        let product = lanes.mul_add_widened(
            Bf16x2::new(Bf16::from_f32(2.0), Bf16::from_f32(-2.0)),
            Bf16x2::new(Bf16::from_f32(1.0), Bf16::from_f32(0.5)),
        );
        assert_eq!(product.lane0().to_f32(), 4.0);
        assert_eq!(product.lane1().to_f32(), 5.0);
    }

    #[test]
    fn classifiers_handle_signed_zero_subnormals_and_nan() {
        assert!(F16::NEG_ZERO.is_sign_negative());
        assert_eq!(F16::NEG_ZERO.abs().to_bits(), F16::ZERO.to_bits());
        assert!(F16::MIN_POSITIVE_SUBNORMAL.is_subnormal());
        assert!(F16::INFINITY.is_infinite());
        assert!(!F16::INFINITY.is_finite());
        assert!(F16::NAN.is_nan());

        assert!(Bf16::NEG_ZERO.is_sign_negative());
        assert_eq!(Bf16::NEG_ZERO.abs().to_bits(), Bf16::ZERO.to_bits());
        assert!(Bf16::MIN_POSITIVE_SUBNORMAL.is_subnormal());
        assert!(Bf16::INFINITY.is_infinite());
        assert!(!Bf16::INFINITY.is_finite());
        assert!(Bf16::NAN.is_nan());
    }
}
