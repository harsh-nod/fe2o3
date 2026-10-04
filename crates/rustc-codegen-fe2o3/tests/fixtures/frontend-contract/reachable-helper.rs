#![allow(dead_code)]

const CONTRACT: &[u8] = &[
    70, 69, 50, 79, 51, 75, 70, 0, 1, 0, 2, 0, 32, 0, 0, 0, 0, 0, 0, 0, 1, 0, 1,
    0, 25, 0, 0, 0, 0, 0, 0, 0,
];

#[inline(never)]
unsafe fn helper() {
    let value = 9_u64;
    unsafe {
        core::arch::asm!(
            "/* {value} */",
            value = in(reg) value,
            options(nomem, nostack, preserves_flags)
        );
    }
}

#[unsafe(export_name = "fe2o3_kernel_reachable_helper")]
pub unsafe fn kernel() {
    unsafe { helper() }
}

#[used]
static __fe2o3_kernel_registration_reachable_helper: (
    u64,
    u16,
    u16,
    &'static str,
    &'static str,
    unsafe fn(),
) = (
    0x4e52_4b33_4f32_4546,
    1,
    1,
    "reachable_helper",
    "reachable_helper",
    kernel,
);

#[used]
static __fe2o3_kernel_frontend_contract_v1_reachable_helper: (
    u64,
    u16,
    u16,
    &'static str,
    &'static [u8],
    unsafe fn(),
) = (
    0x4146_4b33_4f32_4546,
    1,
    1,
    "reachable_helper",
    CONTRACT,
    kernel,
);
