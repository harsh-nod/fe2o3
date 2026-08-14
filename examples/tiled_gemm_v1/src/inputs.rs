//! Deterministic, finite BF16 input generation.

use fe2o3_device::Bf16;

use crate::contract::ShapeV1;

/// Closed finite-normal input alphabet plus positive zero.
///
/// Values are encoded as BF16 bits directly. No host narrowing conversion is
/// involved in input generation.
pub const BF16_INPUT_PATTERN_V1: &[u16] = &[
    0x0000, // +0.0
    0x3d80, // +0.0625
    0xbd80, // -0.0625
    0x3e00, // +0.125
    0xbe00, // -0.125
    0x3e80, // +0.25
    0xbe80, // -0.25
    0x3f00, // +0.5
    0xbf00, // -0.5
    0x3f80, // +1.0
    0xbf80, // -1.0
    0x4000, // +2.0
    0xc000, // -2.0
    0x4040, // +3.0
    0xc040, // -3.0
    0x4080, // +4.0
];

const A_DOMAIN_V1: u64 = 0x5d7a_91c4_31e2_b80f;
const B_DOMAIN_V1: u64 = 0xa2c5_6e3b_ce1d_47f0;

/// Deterministic row-major BF16 inputs for one shape.
pub struct GeneratedInputsV1 {
    /// `A[M,K]` in row-major order.
    pub a: Vec<Bf16>,
    /// `B[K,N]` in row-major order.
    pub b: Vec<Bf16>,
}

impl GeneratedInputsV1 {
    /// Returns exact BF16 storage bits for `A`.
    pub fn a_bits(&self) -> Vec<u16> {
        self.a.iter().copied().map(Bf16::to_bits).collect()
    }

    /// Returns exact BF16 storage bits for `B`.
    pub fn b_bits(&self) -> Vec<u16> {
        self.b.iter().copied().map(Bf16::to_bits).collect()
    }
}

fn mix64(mut value: u64) -> u64 {
    value = value.wrapping_add(0x9e37_79b9_7f4a_7c15);
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

fn shape_identity(shape: ShapeV1) -> u64 {
    mix64(
        u64::from(shape.m())
            ^ u64::from(shape.n()).rotate_left(21)
            ^ u64::from(shape.k()).rotate_left(42),
    )
}

fn generated_value(shape: ShapeV1, seed: u64, domain: u64, index: usize) -> Bf16 {
    let selector = mix64(
        seed ^ shape_identity(shape) ^ domain ^ (index as u64).wrapping_mul(0xd6e8_feb8_6659_fd93),
    );
    Bf16::from_bits(BF16_INPUT_PATTERN_V1[selector as usize % BF16_INPUT_PATTERN_V1.len()])
}

/// Generates deterministic BF16 bit patterns for `A` and `B`.
///
/// The seed, complete shape, operand domain, and row-major linear index all
/// participate in selection from [`BF16_INPUT_PATTERN_V1`].
pub fn generate_inputs_v1(shape: ShapeV1, seed: u64) -> GeneratedInputsV1 {
    let a = (0..shape.a_elements())
        .map(|index| generated_value(shape, seed, A_DOMAIN_V1, index))
        .collect();
    let b = (0..shape.b_elements())
        .map(|index| generated_value(shape, seed, B_DOMAIN_V1, index))
        .collect();
    GeneratedInputsV1 { a, b }
}
