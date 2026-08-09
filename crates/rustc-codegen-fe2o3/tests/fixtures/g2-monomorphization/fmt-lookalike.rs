extern crate g2_fmt_lookalike;

fn identity(value: u32) -> u32 {
    value
}

#[unsafe(no_mangle)]
pub fn fe2o3_kernel_fmt_lookalike(seed: u32) -> u32 {
    g2_fmt_lookalike::fmt::hidden(identity, seed)
}

#[used]
#[allow(non_upper_case_globals, clippy::type_complexity)]
static __fe2o3_kernel_registration_fmt_lookalike: (
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
    "fmt_lookalike",
    "fmt_lookalike",
    fe2o3_kernel_fmt_lookalike,
);

fn main() {}
