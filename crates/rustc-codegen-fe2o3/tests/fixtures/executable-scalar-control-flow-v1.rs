#![allow(clippy::let_underscore_untyped)]

#[inline(never)]
fn nested_match_helper(limit: u32) -> u32 {
    let mut outer = 0_u32;
    let mut sum = 0_u32;
    while outer < limit {
        let mut inner = 0_u32;
        while inner < limit {
            match inner {
                2 => {}
                _ => sum += inner,
            }
            inner += 1;
        }
        outer += 1;
    }
    sum
}

#[unsafe(no_mangle)]
pub fn fe2o3_kernel_scalar_control_flow_v1(limit: u32) {
    let _ = nested_match_helper(limit);
}

#[used]
#[allow(non_upper_case_globals)]
static __fe2o3_kernel_registration_scalar_control_flow_v1: (
    u64,
    u16,
    u16,
    &'static str,
    &'static str,
    fn(u32),
) = (
    0x4e52_4b33_4f32_4546,
    1,
    1,
    "scalar_control_flow_v1",
    "scalar_control_flow_v1",
    fe2o3_kernel_scalar_control_flow_v1,
);

fn main() {}
