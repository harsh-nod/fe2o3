#![allow(dead_code)]

#[inline(never)]
unsafe fn helper() {
    let value = 11_u64;
    unsafe {
        core::arch::asm!(
            "/* {value} */",
            value = in(reg) value,
            options(nomem, nostack, preserves_flags)
        );
    }
}

#[unsafe(export_name = "fe2o3_kernel_undeclared_asm")]
pub unsafe fn kernel() {
    unsafe { helper() }
}

#[used]
static __fe2o3_kernel_registration_undeclared_asm: (
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
    "undeclared_asm",
    "undeclared_asm",
    kernel,
);
