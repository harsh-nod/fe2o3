#[unsafe(no_mangle)]
pub fn fe2o3_kernel_malformed(seed: u32) -> u32 {
    seed
}

#[used]
#[allow(non_upper_case_globals, clippy::type_complexity)]
static __fe2o3_kernel_registration_malformed: (
    u64,
    u16,
    u16,
    &'static str,
    &'static str,
    fn(u32) -> u32,
) = (
    0,
    1,
    1,
    "malformed",
    "malformed",
    fe2o3_kernel_malformed,
);

fn main() {}
