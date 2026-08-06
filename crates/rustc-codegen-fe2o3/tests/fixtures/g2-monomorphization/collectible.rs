#![allow(clippy::let_and_return)]

extern crate g2_shared_a;
extern crate g2_shared_b;

#[inline(never)]
fn generic_identity<T: Copy>(value: T) -> T {
    value
}

#[inline(never)]
fn const_bias<const BIAS: u32>(value: u32) -> u32 {
    value + BIAS
}

#[inline(never)]
fn recursive_sum<const LIMIT: u32>(value: u32) -> u32 {
    if value == LIMIT {
        value
    } else {
        value + recursive_sum::<LIMIT>(value + 1)
    }
}

#[unsafe(no_mangle)]
pub fn fe2o3_kernel_monomorphization(seed: u32) -> u32 {
    let duplicate_a = generic_identity(seed);
    let duplicate_b = generic_identity(seed + 1);
    let other_type = generic_identity(seed as u64) as u32;
    let const_a = const_bias::<7>(seed);
    let const_b = const_bias::<11>(seed);
    duplicate_a
        + duplicate_b
        + other_type
        + const_a
        + const_b
        + recursive_sum::<2>(0)
        + g2_shared_a::same_name(seed)
        + g2_shared_b::same_name(seed)
}

#[used]
#[allow(non_upper_case_globals, clippy::type_complexity)]
static __fe2o3_kernel_registration_monomorphization: (
    u64,
    u16,
    u16,
    &'static str,
    &'static str,
    fn(u32) -> u32,
) = (
    0x4e52_4b33_4f32_4546,
    1,
    1,
    "monomorphization",
    "monomorphization",
    fe2o3_kernel_monomorphization,
);

fn main() {}
