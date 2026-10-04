//! Explicit bounded gfx942 physical-entry authoring. These markers are not
//! host assembly or execution authority: the registered compiler consumes the
//! actual source calls, checks all physical state and retains source custody.
//! Initial profile: one root, Wave64, required/max workgroup64 and max_grid2,
//! explicit kernarg loads, waits, bounds mask/store, restore and end.
#[doc(hidden)]
#[rustc_diagnostic_item = "fe2o3_device_amdgpu_physical_entry_begin_gfx942_v1"]
#[inline(never)]
pub fn __amdgpu_physical_entry_begin_gfx942_v1(
    output: crate::DisjointSlice<u32>,
    a: u32,
    b: u32,
    c: u32,
    selector: u32,
) {
    let _ = (output, a, b, c, selector);
    unreachable!("physical-entry begin requires registered source lowering");
}
#[doc(hidden)]
#[rustc_diagnostic_item = "fe2o3_device_amdgpu_physical_entry_label_gfx942_v1"]
#[inline(never)]
pub fn __amdgpu_physical_entry_label_gfx942_v1<const LABEL: u8>() {
    unreachable!("physical-entry label requires registered source lowering");
}
#[doc(hidden)]
#[rustc_diagnostic_item = "fe2o3_device_amdgpu_physical_entry_step_gfx942_v1"]
#[inline(never)]
pub fn __amdgpu_physical_entry_step_gfx942_v1<
    const OP: u8,
    const D: u8,
    const S0: u8,
    const S1: u8,
    const IMM: u32,
>() {
    unreachable!("physical-entry step requires registered source lowering");
}

/// Authors every physical entry instruction through typed marker calls.
/// `s(n)`/`v(n)` name units; `s_pair(n)`/`v_pair(n)` name the even low unit.
/// Labels and registers are literal source constants, never assembler strings.
/// The compiler rejects unsupported state/ABI/launch or malformed registers.
#[macro_export]
macro_rules! amdgpu_physical_entry {
    (
        gfx942_xnack_off_wave64;
        output = $output:expr; a = $a:expr; b = $b:expr;
        c = $c:expr; selector = $selector:expr;
        $($body:tt)*
    ) => {{
        $crate::physical_entry_v1::__amdgpu_physical_entry_begin_gfx942_v1(
            $output, $a, $b, $c, $selector,
        );
        $crate::amdgpu_physical_entry!(@body $($body)*);
    }};
    (@body) => {};
    (@body label($label:literal); $($tail:tt)*) => {
        $crate::physical_entry_v1::__amdgpu_physical_entry_label_gfx942_v1::<$label>();
        $crate::amdgpu_physical_entry!(@body $($tail)*);
    };
    (@body s_load_dwordx2(s_pair($d:literal), kernarg, $imm:literal); $($tail:tt)*) => {
        $crate::physical_entry_v1::__amdgpu_physical_entry_step_gfx942_v1::<
            0, $d, 0, 0, $imm,
        >();
        $crate::amdgpu_physical_entry!(@body $($tail)*);
    };
    (@body s_load_dword(s($d:literal), kernarg, $imm:literal); $($tail:tt)*) => {
        $crate::physical_entry_v1::__amdgpu_physical_entry_step_gfx942_v1::<
            1, $d, 0, 0, $imm,
        >();
        $crate::amdgpu_physical_entry!(@body $($tail)*);
    };
    (@body s_waitcnt_lgkmcnt0(); $($tail:tt)*) => {
        $crate::physical_entry_v1::__amdgpu_physical_entry_step_gfx942_v1::<
            2, 0, 0, 0, 0,
        >();
        $crate::amdgpu_physical_entry!(@body $($tail)*);
    };
    (@body s_lshl_b32(s($d:literal), s($s0:literal), $imm:literal); $($tail:tt)*) => {
        $crate::physical_entry_v1::__amdgpu_physical_entry_step_gfx942_v1::<
            3, $d, $s0, 0, $imm,
        >();
        $crate::amdgpu_physical_entry!(@body $($tail)*);
    };
    (@body v_add_u32_e32(v($d:literal), s($s0:literal), v($s1:literal)); $($tail:tt)*) => {
        $crate::physical_entry_v1::__amdgpu_physical_entry_step_gfx942_v1::<
            4, $d, $s0, $s1, 0,
        >();
        $crate::amdgpu_physical_entry!(@body $($tail)*);
    };
    (@body v_mov_b32_e32(v($d:literal), s($s0:literal)); $($tail:tt)*) => {
        $crate::physical_entry_v1::__amdgpu_physical_entry_step_gfx942_v1::<
            5, $d, $s0, 0, 0,
        >();
        $crate::amdgpu_physical_entry!(@body $($tail)*);
    };
    (@body v_mov_b32_e32(v($d:literal), zero); $($tail:tt)*) => {
        $crate::physical_entry_v1::__amdgpu_physical_entry_step_gfx942_v1::<
            5, $d, 255, 0, 0,
        >();
        $crate::amdgpu_physical_entry!(@body $($tail)*);
    };
    (@body s_cmp_eq_u32(s($s0:literal), zero); $($tail:tt)*) => {
        $crate::physical_entry_v1::__amdgpu_physical_entry_step_gfx942_v1::<
            6, 0, $s0, 0, 0,
        >();
        $crate::amdgpu_physical_entry!(@body $($tail)*);
    };
    (@body s_cbranch_scc1(label($label:literal)); $($tail:tt)*) => {
        $crate::physical_entry_v1::__amdgpu_physical_entry_step_gfx942_v1::<
            7, 0, 0, 0, $label,
        >();
        $crate::amdgpu_physical_entry!(@body $($tail)*);
    };
    (@body s_branch(label($label:literal)); $($tail:tt)*) => {
        $crate::physical_entry_v1::__amdgpu_physical_entry_step_gfx942_v1::<
            8, 0, 0, 0, $label,
        >();
        $crate::amdgpu_physical_entry!(@body $($tail)*);
    };
    (@body v_lshlrev_b64(v_pair($d:literal), v_pair($s0:literal), $imm:literal); $($tail:tt)*) => {
        $crate::physical_entry_v1::__amdgpu_physical_entry_step_gfx942_v1::<
            9, $d, $s0, 0, $imm,
        >();
        $crate::amdgpu_physical_entry!(@body $($tail)*);
    };
    (@body v_add_co_u32_e32(v($d:literal), s($s0:literal), v($s1:literal)); $($tail:tt)*) => {
        $crate::physical_entry_v1::__amdgpu_physical_entry_step_gfx942_v1::<
            10, $d, $s0, $s1, 0,
        >();
        $crate::amdgpu_physical_entry!(@body $($tail)*);
    };
    (@body v_addc_co_u32_e32(v($d:literal), v($s0:literal), v($s1:literal)); $($tail:tt)*) => {
        $crate::physical_entry_v1::__amdgpu_physical_entry_step_gfx942_v1::<
            11, $d, $s0, $s1, 0,
        >();
        $crate::amdgpu_physical_entry!(@body $($tail)*);
    };
    (@body v_cmp_gt_u64_e32(s_pair($s0:literal), v_pair($s1:literal)); $($tail:tt)*) => {
        $crate::physical_entry_v1::__amdgpu_physical_entry_step_gfx942_v1::<
            12, 0, $s0, $s1, 0,
        >();
        $crate::amdgpu_physical_entry!(@body $($tail)*);
    };
    (@body s_and_saveexec_b64(s_pair($d:literal)); $($tail:tt)*) => {
        $crate::physical_entry_v1::__amdgpu_physical_entry_step_gfx942_v1::<
            13, $d, 0, 0, 0,
        >();
        $crate::amdgpu_physical_entry!(@body $($tail)*);
    };
    (@body global_store_dword(v_pair($s0:literal), v($s1:literal)); $($tail:tt)*) => {
        $crate::physical_entry_v1::__amdgpu_physical_entry_step_gfx942_v1::<
            14, 0, $s0, $s1, 0,
        >();
        $crate::amdgpu_physical_entry!(@body $($tail)*);
    };
    (@body s_waitcnt_vmcnt0(); $($tail:tt)*) => {
        $crate::physical_entry_v1::__amdgpu_physical_entry_step_gfx942_v1::<
            15, 0, 0, 0, 0,
        >();
        $crate::amdgpu_physical_entry!(@body $($tail)*);
    };
    (@body s_mov_b64_exec(s_pair($s0:literal)); $($tail:tt)*) => {
        $crate::physical_entry_v1::__amdgpu_physical_entry_step_gfx942_v1::<
            16, 0, $s0, 0, 0,
        >();
        $crate::amdgpu_physical_entry!(@body $($tail)*);
    };
    (@body s_endpgm0(); $($tail:tt)*) => {
        $crate::physical_entry_v1::__amdgpu_physical_entry_step_gfx942_v1::<
            17, 0, 0, 0, 0,
        >();
        $crate::amdgpu_physical_entry!(@body $($tail)*);
    };
    (@body fallthrough(label($label:literal)); $($tail:tt)*) => {
        $crate::physical_entry_v1::__amdgpu_physical_entry_step_gfx942_v1::<
            18, 0, 0, 0, $label,
        >();
        $crate::amdgpu_physical_entry!(@body $($tail)*);
    };
    ($($unsupported:tt)*) => {
        compile_error!("unsupported amdgpu_physical_entry! syntax; use the closed typed gfx942 physical-entry primitives");
    };
}
