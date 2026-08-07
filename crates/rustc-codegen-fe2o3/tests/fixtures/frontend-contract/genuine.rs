#![allow(dead_code)]

const CONTRACT: &[u8] = &[
    70, 69, 50, 79, 51, 75, 70, 0, 1, 0, 3, 0, 64, 0, 0, 0, 0, 0, 0, 0, 3, 0, 0,
    0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0,
    0, 0, 0, 0, 0, 1, 0, 1, 0, 25, 0, 0, 0, 0, 0, 0, 0,
];

#[unsafe(export_name = "fe2o3_kernel_genuine")]
pub unsafe fn kernel() {
    let value = 7_u64;
    unsafe {
        core::arch::asm!(
            "/* {value} */",
            value = in(reg) value,
            options(nomem, nostack, preserves_flags)
        );
    }
}

#[used]
static __fe2o3_kernel_registration_genuine: (
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
    "genuine",
    "genuine",
    kernel,
);

#[used]
static __fe2o3_kernel_frontend_contract_v1_genuine: (
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
    "genuine",
    CONTRACT,
    kernel,
);
