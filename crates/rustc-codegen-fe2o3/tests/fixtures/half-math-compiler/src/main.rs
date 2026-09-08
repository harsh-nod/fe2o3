use fe2o3_device::{Bf16, Bf16x2, F16, KernelContext, StrictIeee};
use fe2o3_macros::kernel;

#[inline(never)]
fn f16_add(lhs: F16, rhs: F16) -> F16 {
    lhs + rhs
}

#[kernel]
pub fn half_math_kernel(
    context: KernelContext<'_>,
    f16_lhs: F16,
    f16_rhs: F16,
    bf16_lhs: Bf16,
    bf16_rhs: Bf16,
    packed: Bf16x2,
    scalar: f32,
) {
    let _f16_sum = f16_add(f16_lhs, f16_rhs);
    let _bf16_quotient = bf16_lhs / bf16_rhs;
    let bf16_from_scalar = Bf16::from_f32(scalar);
    let _bf16_round_trip = bf16_from_scalar.to_f32();
    let _bf16_bits_round_trip = Bf16::from_bits(bf16_lhs.to_bits());
    let f16_from_scalar = F16::from_f32(scalar);
    let _scalar_round_trip = f16_from_scalar.to_f32();

    let math = context.math();
    let policy = context.numerical_policy::<StrictIeee>();
    let math = math.with_numerical_policy(&policy);
    let _root = math.sqrt_f32(scalar);
    let _sine = math.sin_f32(scalar);
    let _packed_fma = math.mul_add_bf16x2(packed, packed, packed);
}

fn main() {}
