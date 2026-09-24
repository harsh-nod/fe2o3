//! Explicit bounded gfx942 physical-lds-exchange authoring. These markers are not
//! host assembly or execution authority: the registered compiler consumes the
//! actual source calls, checks all physical state and retains source custody.
//! Initial profile: one root, two Wave64 waves, required/max workgroup128 and max_grid1.
//! A typed static 512-byte LDS frame is owned by Begin, never a source LLVM attribute.
//! Full-EXEC input/load, local LDS write/wait, publication barrier, peer LDS read/wait,
//! explicit output mask/store/wait, restore and end. No implicit tail or M0 input.
#[doc(hidden)]
#[rustc_diagnostic_item = "fe2o3_device_amdgpu_physical_lds_exchange_begin_gfx942_v1"]
#[inline(never)]
pub fn __amdgpu_physical_lds_exchange_begin_gfx942_v1<
    const BASE: u32,
    const BYTES: u32,
    const ALIGN: u32,
    const EPOCH: u8,
>(
    input: &[u32],
    output: crate::DisjointSlice<u32>,
) {
    let _ = (input, output);
    unreachable!("physical-lds-exchange begin requires registered source lowering");
}
#[doc(hidden)]
#[rustc_diagnostic_item = "fe2o3_device_amdgpu_physical_lds_exchange_label_gfx942_v1"]
#[inline(never)]
pub fn __amdgpu_physical_lds_exchange_label_gfx942_v1<const LABEL: u8>() {
    unreachable!("physical-lds-exchange label requires registered source lowering");
}
#[doc(hidden)]
#[rustc_diagnostic_item = "fe2o3_device_amdgpu_physical_lds_exchange_step_gfx942_v1"]
#[inline(never)]
pub fn __amdgpu_physical_lds_exchange_step_gfx942_v1<
    const OP: u8,
    const D: u8,
    const S0: u8,
    const S1: u8,
    const IMM: u32,
>() {
    unreachable!("physical-lds-exchange step requires registered source lowering");
}

/// Authors every physical entry instruction through typed marker calls.
/// `s(n)`/`v(n)` name units; `s_pair(n)`/`v_pair(n)` name the even low unit.
/// Labels and registers are literal source constants, never assembler strings.
/// The compiler rejects unsupported state/ABI/launch or malformed registers.
#[macro_export]
macro_rules! amdgpu_physical_lds_exchange {
    (
        gfx942_xnack_off_wave64;
        input = $input:expr; output = $output:expr;
        lds = static_u32_frame($base:literal, $bytes:literal, $align:literal, $epoch:literal);
        $($body:tt)*
    ) => {{
        $crate::physical_lds_exchange_v1::__amdgpu_physical_lds_exchange_begin_gfx942_v1::<$base, $bytes, $align, $epoch>(
            $input, $output,
        );
        $crate::amdgpu_physical_lds_exchange!(@body $($body)*);
    }};
    (@body) => {};
    (@body label($label:literal); $($tail:tt)*) => {
        $crate::physical_lds_exchange_v1::__amdgpu_physical_lds_exchange_label_gfx942_v1::<$label>();
        $crate::amdgpu_physical_lds_exchange!(@body $($tail)*);
    };
    (@body s_load_dwordx2(s_pair($d:literal), kernarg, $imm:literal); $($tail:tt)*) => {
        $crate::physical_lds_exchange_v1::__amdgpu_physical_lds_exchange_step_gfx942_v1::<
            0, $d, 0, 0, $imm,
        >();
        $crate::amdgpu_physical_lds_exchange!(@body $($tail)*);
    };
    (@body s_waitcnt_lgkmcnt0(); $($tail:tt)*) => {
        $crate::physical_lds_exchange_v1::__amdgpu_physical_lds_exchange_step_gfx942_v1::<
            1, 0, 0, 0, 0,
        >();
        $crate::amdgpu_physical_lds_exchange!(@body $($tail)*);
    };
    (@body s_lshl_b32(s($d:literal), s($s0:literal), $imm:literal); $($tail:tt)*) => {
        $crate::physical_lds_exchange_v1::__amdgpu_physical_lds_exchange_step_gfx942_v1::<
            2, $d, $s0, 0, $imm,
        >();
        $crate::amdgpu_physical_lds_exchange!(@body $($tail)*);
    };
    (@body v_add_u32_e32(v($d:literal), s($s0:literal), v($s1:literal)); $($tail:tt)*) => {
        $crate::physical_lds_exchange_v1::__amdgpu_physical_lds_exchange_step_gfx942_v1::<
            3, $d, $s0, $s1, 0,
        >();
        $crate::amdgpu_physical_lds_exchange!(@body $($tail)*);
    };
    (@body v_mov_b32_e32(v($d:literal), s($s0:literal)); $($tail:tt)*) => {
        $crate::physical_lds_exchange_v1::__amdgpu_physical_lds_exchange_step_gfx942_v1::<
            4, $d, $s0, 0, 0,
        >();
        $crate::amdgpu_physical_lds_exchange!(@body $($tail)*);
    };
    (@body v_mov_b32_e32(v($d:literal), zero); $($tail:tt)*) => {
        $crate::physical_lds_exchange_v1::__amdgpu_physical_lds_exchange_step_gfx942_v1::<
            4, $d, 255, 0, 0,
        >();
        $crate::amdgpu_physical_lds_exchange!(@body $($tail)*);
    };
    (@body v_lshlrev_b64(v_pair($d:literal), v_pair($s0:literal), $imm:literal); $($tail:tt)*) => {
        $crate::physical_lds_exchange_v1::__amdgpu_physical_lds_exchange_step_gfx942_v1::<
            5, $d, $s0, 0, $imm,
        >();
        $crate::amdgpu_physical_lds_exchange!(@body $($tail)*);
    };
    (@body v_add_co_u32_e32(v($d:literal), s($s0:literal), v($s1:literal)); $($tail:tt)*) => {
        $crate::physical_lds_exchange_v1::__amdgpu_physical_lds_exchange_step_gfx942_v1::<
            6, $d, $s0, $s1, 0,
        >();
        $crate::amdgpu_physical_lds_exchange!(@body $($tail)*);
    };
    (@body v_addc_co_u32_e32(v($d:literal), v($s0:literal), v($s1:literal)); $($tail:tt)*) => {
        $crate::physical_lds_exchange_v1::__amdgpu_physical_lds_exchange_step_gfx942_v1::<
            7, $d, $s0, $s1, 0,
        >();
        $crate::amdgpu_physical_lds_exchange!(@body $($tail)*);
    };
    (@body v_cmp_gt_u64_e32(s_pair($s0:literal), v_pair($s1:literal)); $($tail:tt)*) => {
        $crate::physical_lds_exchange_v1::__amdgpu_physical_lds_exchange_step_gfx942_v1::<
            10, 0, $s0, $s1, 0,
        >();
        $crate::amdgpu_physical_lds_exchange!(@body $($tail)*);
    };
    (@body s_and_saveexec_b64(s_pair($d:literal)); $($tail:tt)*) => {
        $crate::physical_lds_exchange_v1::__amdgpu_physical_lds_exchange_step_gfx942_v1::<
            11, $d, 0, 0, 0,
        >();
        $crate::amdgpu_physical_lds_exchange!(@body $($tail)*);
    };
    (@body global_store_dword(v_pair($s0:literal), v($s1:literal)); $($tail:tt)*) => {
        $crate::physical_lds_exchange_v1::__amdgpu_physical_lds_exchange_step_gfx942_v1::<
            12, 0, $s0, $s1, 0,
        >();
        $crate::amdgpu_physical_lds_exchange!(@body $($tail)*);
    };
    (@body s_waitcnt_vmcnt0(); $($tail:tt)*) => {
        $crate::physical_lds_exchange_v1::__amdgpu_physical_lds_exchange_step_gfx942_v1::<
            9, 0, 0, 0, 0,
        >();
        $crate::amdgpu_physical_lds_exchange!(@body $($tail)*);
    };
    (@body s_mov_b64_exec(s_pair($s0:literal)); $($tail:tt)*) => {
        $crate::physical_lds_exchange_v1::__amdgpu_physical_lds_exchange_step_gfx942_v1::<
            13, 0, $s0, 0, 0,
        >();
        $crate::amdgpu_physical_lds_exchange!(@body $($tail)*);
    };
    (@body s_endpgm0(); $($tail:tt)*) => {
        $crate::physical_lds_exchange_v1::__amdgpu_physical_lds_exchange_step_gfx942_v1::<
            14, 0, 0, 0, 0,
        >();
        $crate::amdgpu_physical_lds_exchange!(@body $($tail)*);
    };
    (@body global_load_dword(v($d:literal), v_pair($s0:literal)); $($tail:tt)*) => {
        $crate::physical_lds_exchange_v1::__amdgpu_physical_lds_exchange_step_gfx942_v1::<8, $d, $s0, 0, 0>();
        $crate::amdgpu_physical_lds_exchange!(@body $($tail)*);
    };
    (@body v_lshlrev_b32(v($d:literal), v($s0:literal), $imm:literal); $($tail:tt)*) => {
        $crate::physical_lds_exchange_v1::__amdgpu_physical_lds_exchange_step_gfx942_v1::<15, $d, $s0, 0, $imm>();
        $crate::amdgpu_physical_lds_exchange!(@body $($tail)*);
    };
    (@body v_xor_b32_e32(v($d:literal), v($s0:literal), $imm:literal); $($tail:tt)*) => {
        $crate::physical_lds_exchange_v1::__amdgpu_physical_lds_exchange_step_gfx942_v1::<16, $d, $s0, 0, $imm>();
        $crate::amdgpu_physical_lds_exchange!(@body $($tail)*);
    };
    (@body ds_write_b32(v($s0:literal), v($s1:literal)); $($tail:tt)*) => {
        $crate::physical_lds_exchange_v1::__amdgpu_physical_lds_exchange_step_gfx942_v1::<17, 0, $s0, $s1, 0>();
        $crate::amdgpu_physical_lds_exchange!(@body $($tail)*);
    };
    (@body ds_read_b32(v($d:literal), v($s0:literal)); $($tail:tt)*) => {
        $crate::physical_lds_exchange_v1::__amdgpu_physical_lds_exchange_step_gfx942_v1::<18, $d, $s0, 0, 0>();
        $crate::amdgpu_physical_lds_exchange!(@body $($tail)*);
    };
    (@body s_barrier(); $($tail:tt)*) => {
        $crate::physical_lds_exchange_v1::__amdgpu_physical_lds_exchange_step_gfx942_v1::<19, 0, 0, 0, 0>();
        $crate::amdgpu_physical_lds_exchange!(@body $($tail)*);
    };
    ($($unsupported:tt)*) => {
        compile_error!("unsupported amdgpu_physical_lds_exchange! syntax; use the closed typed gfx942 physical-lds-exchange primitives");
    };
}
