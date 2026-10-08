//! OCP E2M1 FP4 representation and explicit numerical conversion policy.
//!
//! Based on OCP Microscaling Formats (MX) v1.0, section 5.3.3. This is an
//! unscaled element representation, not an MX block or an MFMA evaluator.
//! There are sixteen finite encodings, including two signed zeros.
//!
//! The named narrowing policy is round-to-nearest, ties-to-even, followed by
//! sign-preserving saturation (including infinities). NaNs are refused: E2M1
//! has no NaN encoding and the specification leaves their conversion policy
//! to the implementation. No gfx942 FNUZ semantics or hardware conversion
//! behavior is inferred.
//!
//! All value operations use fixed-size integer data and are available in
//! no_std. They confer no target, lane, LDS, ABI, compiler or launch capability.

use core::fmt;
use core::ops::Neg;

/// Failure to construct one valid E2M1 representation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Fp4E2M1Error {
    /// Bits outside the low nibble are set; they are not silently discarded.
    InvalidEncoding(u8),
    /// E2M1 has no NaN representation.
    Nan,
}

impl fmt::Display for Fp4E2M1Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidEncoding(bits) => {
                write!(formatter, "E2M1 encoding exceeds four bits: {bits:#04x}")
            }
            Self::Nan => formatter.write_str("E2M1 RNE-saturating conversion refuses NaN"),
        }
    }
}
impl core::error::Error for Fp4E2M1Error {}

/// One unscaled OCP E2M1 FP4 element in the low nibble of a byte.
///
/// The sign is bit 3, exponent bits are 2..1 (bias 1), and fraction is bit 0.
/// Positive magnitudes are 0, 0.5, 1, 1.5, 2, 3, 4 and 6. Negative zero is
/// retained. The remaining four storage bits are always zero.
///
/// This type is distinct from the compiler-issued gfx950 matrix format marker.
/// Its layout is ordinary one-byte storage, not a new admitted device ABI.
///
/// ```
/// use fe2o3_device::Fp4E2M1Ocp;
///
/// let value = Fp4E2M1Ocp::try_from_f32_rne_saturating(2.5).unwrap();
/// assert_eq!(value.to_bits(), 4); // tie goes to even: 2 rather than 3
/// assert_eq!(value.to_f32(), 2.0);
/// assert!(Fp4E2M1Ocp::try_from_bits(0x10).is_err());
/// ```
#[derive(Clone, Copy, Default)]
#[repr(transparent)]
pub struct Fp4E2M1Ocp(u8);

impl Fp4E2M1Ocp {
    /// Positive zero.
    pub const ZERO: Self = Self(0);
    /// Negative zero.
    pub const NEG_ZERO: Self = Self(8);
    /// Positive one.
    pub const ONE: Self = Self(2);
    /// Maximum finite positive value, 6.
    pub const MAX: Self = Self(7);
    /// Minimum finite value, -6.
    pub const MIN: Self = Self(15);
    /// Smallest positive normal, 1.
    pub const MIN_POSITIVE: Self = Self(2);
    /// Smallest positive subnormal, 0.5.
    pub const MIN_POSITIVE_SUBNORMAL: Self = Self(1);

    /// Accepts exactly the sixteen nibble encodings; upper bits refuse.
    pub const fn try_from_bits(bits: u8) -> Result<Self, Fp4E2M1Error> {
        if bits < 16 {
            Ok(Self(bits))
        } else {
            Err(Fp4E2M1Error::InvalidEncoding(bits))
        }
    }

    /// Returns the exact nibble, with all upper bits zero.
    pub const fn to_bits(self) -> u8 {
        self.0
    }

    /// Widens exactly to f32, including signed zero and the subnormal 0.5.
    ///
    /// Every E2M1 value is exactly representable as an f32 normal or zero.
    pub const fn to_f32(self) -> f32 {
        const MAGNITUDES: [u32; 8] = [
            0,
            0x3f00_0000,
            0x3f80_0000,
            0x3fc0_0000,
            0x4000_0000,
            0x4040_0000,
            0x4080_0000,
            0x40c0_0000,
        ];
        let sign = ((self.0 & 8) as u32) << 28;
        f32::from_bits(sign | MAGNITUDES[(self.0 & 7) as usize])
    }

    /// Narrows with explicit RNE, finite/infinite saturation, and NaN refusal.
    ///
    /// Both input zero signs survive. Underflow rounds to zero with the input
    /// sign; exact midpoint ties choose the even low significand bit. All
    /// infinities and finite overflows saturate to signed 6. No floating-point
    /// arithmetic, allocation, ambient rounding mode or denormal mode is used.
    /// The fixed search inspects at most seven exact f32 midpoint bit patterns.
    pub const fn try_from_f32_rne_saturating(value: f32) -> Result<Self, Fp4E2M1Error> {
        let bits = value.to_bits();
        let magnitude = bits & 0x7fff_ffff;
        if magnitude > 0x7f80_0000 {
            return Err(Fp4E2M1Error::Nan);
        }
        let sign = ((bits >> 28) as u8) & 8;
        // Midpoints between each adjacent positive E2M1 value:
        // 0.25, 0.75, 1.25, 1.75, 2.5, 3.5, 5.0.
        const MIDPOINTS: [u32; 7] = [
            0x3e80_0000,
            0x3f40_0000,
            0x3fa0_0000,
            0x3fe0_0000,
            0x4020_0000,
            0x4060_0000,
            0x40a0_0000,
        ];
        let mut lower = 0;
        while lower < MIDPOINTS.len() {
            let midpoint = MIDPOINTS[lower];
            if magnitude < midpoint || (magnitude == midpoint && lower & 1 == 0) {
                return Ok(Self(sign | lower as u8));
            }
            lower += 1;
        }
        Ok(Self(sign | 7))
    }

    /// Every representable value is finite.
    pub const fn is_finite(self) -> bool {
        true
    }
    /// E2M1 has no NaN encoding.
    pub const fn is_nan(self) -> bool {
        false
    }
    /// E2M1 has no infinity encoding.
    pub const fn is_infinite(self) -> bool {
        false
    }
    /// True for either signed zero.
    pub const fn is_zero(self) -> bool {
        self.0 & 7 == 0
    }
    /// True for either sign of the subnormal 0.5.
    pub const fn is_subnormal(self) -> bool {
        self.0 & 7 == 1
    }
    /// True for finite, nonzero, nonsubnormal values.
    pub const fn is_normal(self) -> bool {
        self.0 & 7 >= 2
    }
    /// Returns the stored sign, including negative zero.
    pub const fn is_sign_negative(self) -> bool {
        self.0 & 8 != 0
    }
    /// Clears only the sign bit.
    pub const fn abs(self) -> Self {
        Self(self.0 & 7)
    }
}

impl From<Fp4E2M1Ocp> for f32 {
    fn from(value: Fp4E2M1Ocp) -> Self {
        value.to_f32()
    }
}
impl Neg for Fp4E2M1Ocp {
    type Output = Self;
    fn neg(self) -> Self {
        Self(self.0 ^ 8)
    }
}
impl PartialEq for Fp4E2M1Ocp {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0 || (self.is_zero() && other.is_zero())
    }
}
impl Eq for Fp4E2M1Ocp {}
impl PartialOrd for Fp4E2M1Ocp {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        self.to_f32().partial_cmp(&other.to_f32())
    }
}
impl fmt::Debug for Fp4E2M1Ocp {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("Fp4E2M1Ocp")
            .field(&self.to_f32())
            .finish()
    }
}

/// Eight E2M1 nibbles packed into one u32, element zero in bits 0..4.
///
/// "Element" here is a storage position, not a GPU wave lane or fragment map.
/// Packing is defined by bit position, independently of host byte order. The
/// existing gfx950 matrix views consume one source byte per logical element:
/// use to_unpacked_bytes, not the four dense bytes of this u32, for those views.
///
/// Equality compares numeric element values (thus +0 equals -0); use to_bits
/// when exact packed representation equality is required.
#[derive(Clone, Copy, Default)]
#[repr(transparent)]
pub struct Fp4E2M1Ocpx8(u32);

impl Fp4E2M1Ocpx8 {
    /// Eight positive zeros.
    pub const ZERO: Self = Self(0);

    /// Packs eight already-checked scalar representations.
    pub const fn from_array(values: [Fp4E2M1Ocp; 8]) -> Self {
        let mut packed = 0;
        let mut index = 0;
        while index < 8 {
            packed |= (values[index].to_bits() as u32) << (index * 4);
            index += 1;
        }
        Self(packed)
    }

    /// Every 32-bit pattern contains eight valid E2M1 nibbles.
    pub const fn from_bits(bits: u32) -> Self {
        Self(bits)
    }
    /// Returns the exact packed bits.
    pub const fn to_bits(self) -> u32 {
        self.0
    }

    /// Reads one storage element; an index outside 0..8 returns None.
    pub const fn element(self, index: usize) -> Option<Fp4E2M1Ocp> {
        if index < 8 {
            Some(Fp4E2M1Ocp(((self.0 >> (index * 4)) & 15) as u8))
        } else {
            None
        }
    }

    /// Returns all eight checked scalar representations in increasing order.
    pub const fn to_array(self) -> [Fp4E2M1Ocp; 8] {
        let mut values = [Fp4E2M1Ocp::ZERO; 8];
        let mut index = 0;
        while index < 8 {
            values[index] = Fp4E2M1Ocp(((self.0 >> (index * 4)) & 15) as u8);
            index += 1;
        }
        values
    }

    /// Produces eight bytes, one checked low-nibble encoding per element.
    pub const fn to_unpacked_bytes(self) -> [u8; 8] {
        let mut values = [0; 8];
        let mut index = 0;
        while index < 8 {
            values[index] = ((self.0 >> (index * 4)) & 15) as u8;
            index += 1;
        }
        values
    }

    /// Checks all eight source bytes before returning their packed value.
    ///
    /// Returns the first invalid encoding, without discarding any upper bits.
    pub const fn try_from_unpacked_bytes(bytes: [u8; 8]) -> Result<Self, Fp4E2M1Error> {
        let mut packed = 0;
        let mut index = 0;
        while index < 8 {
            if bytes[index] >= 16 {
                return Err(Fp4E2M1Error::InvalidEncoding(bytes[index]));
            }
            packed |= (bytes[index] as u32) << (index * 4);
            index += 1;
        }
        Ok(Self(packed))
    }
}

impl PartialEq for Fp4E2M1Ocpx8 {
    fn eq(&self, other: &Self) -> bool {
        self.to_array() == other.to_array()
    }
}
impl Eq for Fp4E2M1Ocpx8 {}
impl fmt::Debug for Fp4E2M1Ocpx8 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("Fp4E2M1Ocpx8")
            .field(&self.to_array())
            .finish()
    }
}
