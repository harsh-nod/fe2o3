#[inline(never)]
fn semantic_helper(value: u32) -> u32 {
    value | 1
}

#[unsafe(no_mangle)]
pub fn fe2o3_kernel_semantic_substitution(seed: u32) -> u32 {
    semantic_helper(seed)
}

#[used]
#[allow(non_upper_case_globals, clippy::type_complexity)]
static __fe2o3_kernel_registration_semantic_substitution: (
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
    "semantic_substitution",
    "semantic_substitution",
    fe2o3_kernel_semantic_substitution,
);

fn main() {}
