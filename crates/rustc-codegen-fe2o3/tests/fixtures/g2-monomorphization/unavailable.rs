extern crate g2_unavailable_helper;

#[inline(never)]
fn local_bridge(seed: u32) -> u32 {
    g2_unavailable_helper::unavailable(seed)
}

#[unsafe(no_mangle)]
pub fn fe2o3_kernel_unavailable(seed: u32) -> u32 {
    local_bridge(seed)
}

#[used]
#[allow(non_upper_case_globals, clippy::type_complexity)]
static __fe2o3_kernel_registration_unavailable: (
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
    "unavailable",
    "unavailable",
    fe2o3_kernel_unavailable,
);

fn main() {}
