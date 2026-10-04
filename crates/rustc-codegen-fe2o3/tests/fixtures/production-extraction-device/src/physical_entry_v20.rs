//! Actual authored entry bodies, not AMD encoding words or a compiler tail.
//! Each selected fixture contributes exactly one kernel. Marker expansion stays
//! straight-line Rust; native branches and every memory/wait/EXEC/end instruction
//! are authored here. No protected/native/runtime qualification is implied.
use fe2o3_device::{DisjointSlice, amdgpu_physical_entry, kernel};

#[cfg(feature = "physical-entry-one-v20")]
#[kernel(typed, launch(required=[64,1,1], max=[64,1,1], max_grid=[2,1,1]))]
pub fn physical_one(output: DisjointSlice<u32>, a: u32, b: u32, c: u32, selector: u32) {
    amdgpu_physical_entry! {
        gfx942_xnack_off_wave64;
        output=output; a=a; b=b; c=c; selector=selector;
        label(250);
        s_load_dwordx2(s_pair(8), kernarg, 0);
        s_load_dwordx2(s_pair(10), kernarg, 8);
        s_load_dword(s(12), kernarg, 16);
        s_load_dword(s(13), kernarg, 20);
        s_load_dword(s(14), kernarg, 24);
        s_load_dword(s(15), kernarg, 28);
        s_waitcnt_lgkmcnt0();
        s_lshl_b32(s(16), s(2), 6);
        v_add_u32_e32(v(2), s(16), v(0));
        v_mov_b32_e32(v(3), zero);
        v_mov_b32_e32(v(4), s(9));
        v_mov_b32_e32(v(8), s(12));
        v_lshlrev_b64(v_pair(6), v_pair(2), 2);
        v_add_co_u32_e32(v(6), s(8), v(6));
        v_addc_co_u32_e32(v(7), v(4), v(7));
        v_cmp_gt_u64_e32(s_pair(10), v_pair(2));
        s_and_saveexec_b64(s_pair(18));
        global_store_dword(v_pair(6), v(8));
        s_waitcnt_vmcnt0();
        s_mov_b64_exec(s_pair(18));
        s_endpgm0();
    }
}

#[cfg(feature = "physical-entry-diamond-v20")]
#[kernel(typed, launch(required=[64,1,1], max=[64,1,1], max_grid=[2,1,1]))]
pub fn physical_diamond(output: DisjointSlice<u32>, a: u32, b: u32, c: u32, selector: u32) {
    amdgpu_physical_entry! {
        gfx942_xnack_off_wave64;
        output=output; a=a; b=b; c=c; selector=selector;
        label(250);
        s_load_dwordx2(s_pair(8), kernarg, 0);
        s_load_dwordx2(s_pair(10), kernarg, 8);
        s_load_dword(s(12), kernarg, 16);
        s_load_dword(s(13), kernarg, 20);
        s_load_dword(s(14), kernarg, 24);
        s_load_dword(s(15), kernarg, 28);
        s_waitcnt_lgkmcnt0();
        s_lshl_b32(s(16), s(2), 6);
        v_add_u32_e32(v(2), s(16), v(0));
        v_mov_b32_e32(v(3), zero);
        v_mov_b32_e32(v(4), s(9));
        s_cmp_eq_u32(s(15), zero);
        s_cbranch_scc1(label(0));
        label(4);
        v_mov_b32_e32(v(8), s(13));
        s_branch(label(7));
        label(0);
        v_mov_b32_e32(v(8), s(12));
        fallthrough(label(7));
        label(7);
        v_lshlrev_b64(v_pair(6), v_pair(2), 2);
        v_add_co_u32_e32(v(6), s(8), v(6));
        v_addc_co_u32_e32(v(7), v(4), v(7));
        v_cmp_gt_u64_e32(s_pair(10), v_pair(2));
        s_and_saveexec_b64(s_pair(18));
        global_store_dword(v_pair(6), v(8));
        s_waitcnt_vmcnt0();
        s_mov_b64_exec(s_pair(18));
        s_endpgm0();
    }
}

#[cfg(feature = "physical-entry-registers-v20")]
#[kernel(typed, launch(required=[64,1,1], max=[64,1,1], max_grid=[2,1,1]))]
pub fn physical_registers(output: DisjointSlice<u32>, a: u32, b: u32, c: u32, selector: u32) {
    amdgpu_physical_entry! {
        gfx942_xnack_off_wave64;
        output=output; a=a; b=b; c=c; selector=selector;
        label(250);
        s_load_dwordx2(s_pair(8), kernarg, 0);
        s_load_dwordx2(s_pair(10), kernarg, 8);
        s_load_dword(s(12), kernarg, 16);
        s_load_dword(s(13), kernarg, 20);
        s_load_dword(s(14), kernarg, 24);
        s_load_dword(s(15), kernarg, 28);
        s_waitcnt_lgkmcnt0();
        s_lshl_b32(s(16), s(2), 6);
        v_add_u32_e32(v(2), s(16), v(0));
        v_mov_b32_e32(v(3), zero);
        v_mov_b32_e32(v(4), s(9));
        s_cmp_eq_u32(s(15), zero);
        s_cbranch_scc1(label(0));
        label(4);
        v_mov_b32_e32(v(22), s(13));
        s_branch(label(7));
        label(0);
        v_mov_b32_e32(v(22), s(12));
        fallthrough(label(7));
        label(7);
        v_lshlrev_b64(v_pair(6), v_pair(2), 2);
        v_add_co_u32_e32(v(6), s(8), v(6));
        v_addc_co_u32_e32(v(7), v(4), v(7));
        v_cmp_gt_u64_e32(s_pair(10), v_pair(2));
        s_and_saveexec_b64(s_pair(18));
        global_store_dword(v_pair(6), v(22));
        s_waitcnt_vmcnt0();
        s_mov_b64_exec(s_pair(18));
        s_endpgm0();
    }
}

#[cfg(feature = "physical-entry-wrong-launch-v20")]
#[kernel(typed, launch(required=[128,1,1], max=[128,1,1], max_grid=[2,1,1]))]
pub fn physical_wrong_launch(output: DisjointSlice<u32>, a: u32, b: u32, c: u32, selector: u32) {
    amdgpu_physical_entry! {
        gfx942_xnack_off_wave64;
        output=output; a=a; b=b; c=c; selector=selector;
        label(250);
        s_load_dwordx2(s_pair(8), kernarg, 0);
        s_load_dwordx2(s_pair(10), kernarg, 8);
        s_load_dword(s(12), kernarg, 16);
        s_load_dword(s(13), kernarg, 20);
        s_load_dword(s(14), kernarg, 24);
        s_load_dword(s(15), kernarg, 28);
        s_waitcnt_lgkmcnt0();
        s_lshl_b32(s(16), s(2), 6);
        v_add_u32_e32(v(2), s(16), v(0));
        v_mov_b32_e32(v(3), zero);
        v_mov_b32_e32(v(4), s(9));
        v_mov_b32_e32(v(8), s(12));
        v_lshlrev_b64(v_pair(6), v_pair(2), 2);
        v_add_co_u32_e32(v(6), s(8), v(6));
        v_addc_co_u32_e32(v(7), v(4), v(7));
        v_cmp_gt_u64_e32(s_pair(10), v_pair(2));
        s_and_saveexec_b64(s_pair(18));
        global_store_dword(v_pair(6), v(8));
        s_waitcnt_vmcnt0();
        s_mov_b64_exec(s_pair(18));
        s_endpgm0();
    }
}

#[cfg(feature = "physical-entry-foreign-input-v20")]
#[kernel(typed, launch(required=[64,1,1], max=[64,1,1], max_grid=[2,1,1]))]
pub fn physical_foreign_input(output: DisjointSlice<u32>, a: u32, b: u32, c: u32, selector: u32) {
    let _ = a;
    amdgpu_physical_entry! {
        gfx942_xnack_off_wave64;
        output=output; a=b; b=b; c=c; selector=selector;
        label(250);
        s_load_dwordx2(s_pair(8), kernarg, 0);
        s_load_dwordx2(s_pair(10), kernarg, 8);
        s_load_dword(s(12), kernarg, 16);
        s_load_dword(s(13), kernarg, 20);
        s_load_dword(s(14), kernarg, 24);
        s_load_dword(s(15), kernarg, 28);
        s_waitcnt_lgkmcnt0();
        s_lshl_b32(s(16), s(2), 6);
        v_add_u32_e32(v(2), s(16), v(0));
        v_mov_b32_e32(v(3), zero);
        v_mov_b32_e32(v(4), s(9));
        v_mov_b32_e32(v(8), s(12));
        v_lshlrev_b64(v_pair(6), v_pair(2), 2);
        v_add_co_u32_e32(v(6), s(8), v(6));
        v_addc_co_u32_e32(v(7), v(4), v(7));
        v_cmp_gt_u64_e32(s_pair(10), v_pair(2));
        s_and_saveexec_b64(s_pair(18));
        global_store_dword(v_pair(6), v(8));
        s_waitcnt_vmcnt0();
        s_mov_b64_exec(s_pair(18));
        s_endpgm0();
    }
}

#[cfg(feature = "physical-entry-undefined-merge-v20")]
#[kernel(typed, launch(required=[64,1,1], max=[64,1,1], max_grid=[2,1,1]))]
pub fn physical_undefined_merge(output: DisjointSlice<u32>, a: u32, b: u32, c: u32, selector: u32) {
    amdgpu_physical_entry! {
        gfx942_xnack_off_wave64;
        output=output; a=a; b=b; c=c; selector=selector;
        label(250);
        s_load_dwordx2(s_pair(8), kernarg, 0);
        s_load_dwordx2(s_pair(10), kernarg, 8);
        s_load_dword(s(12), kernarg, 16);
        s_load_dword(s(13), kernarg, 20);
        s_load_dword(s(14), kernarg, 24);
        s_load_dword(s(15), kernarg, 28);
        s_waitcnt_lgkmcnt0();
        s_lshl_b32(s(16), s(2), 6);
        v_add_u32_e32(v(2), s(16), v(0));
        v_mov_b32_e32(v(3), zero);
        v_mov_b32_e32(v(4), s(9));
        s_cmp_eq_u32(s(15), zero);
        s_cbranch_scc1(label(0));
        label(4);
        v_mov_b32_e32(v(22), s(13));
        s_branch(label(7));
        label(0);
        v_mov_b32_e32(v(8), s(12));
        fallthrough(label(7));
        label(7);
        v_lshlrev_b64(v_pair(6), v_pair(2), 2);
        v_add_co_u32_e32(v(6), s(8), v(6));
        v_addc_co_u32_e32(v(7), v(4), v(7));
        v_cmp_gt_u64_e32(s_pair(10), v_pair(2));
        s_and_saveexec_b64(s_pair(18));
        global_store_dword(v_pair(6), v(8));
        s_waitcnt_vmcnt0();
        s_mov_b64_exec(s_pair(18));
        s_endpgm0();
    }
}

#[cfg(feature = "physical-entry-missing-wait-v20")]
#[kernel(typed, launch(required=[64,1,1], max=[64,1,1], max_grid=[2,1,1]))]
pub fn physical_missing_wait(output: DisjointSlice<u32>, a: u32, b: u32, c: u32, selector: u32) {
    amdgpu_physical_entry! {
        gfx942_xnack_off_wave64;
        output=output; a=a; b=b; c=c; selector=selector;
        label(250);
        s_load_dwordx2(s_pair(8), kernarg, 0);
        s_load_dwordx2(s_pair(10), kernarg, 8);
        s_load_dword(s(12), kernarg, 16);
        s_load_dword(s(13), kernarg, 20);
        s_load_dword(s(14), kernarg, 24);
        s_load_dword(s(15), kernarg, 28);
        s_lshl_b32(s(16), s(2), 6);
        v_add_u32_e32(v(2), s(16), v(0));
        v_mov_b32_e32(v(3), zero);
        v_mov_b32_e32(v(4), s(9));
        v_mov_b32_e32(v(8), s(12));
        v_lshlrev_b64(v_pair(6), v_pair(2), 2);
        v_add_co_u32_e32(v(6), s(8), v(6));
        v_addc_co_u32_e32(v(7), v(4), v(7));
        v_cmp_gt_u64_e32(s_pair(10), v_pair(2));
        s_and_saveexec_b64(s_pair(18));
        global_store_dword(v_pair(6), v(8));
        s_waitcnt_vmcnt0();
        s_mov_b64_exec(s_pair(18));
        s_endpgm0();
    }
}

#[cfg(feature = "physical-entry-wrong-carry-v20")]
#[kernel(typed, launch(required=[64,1,1], max=[64,1,1], max_grid=[2,1,1]))]
pub fn physical_wrong_carry(output: DisjointSlice<u32>, a: u32, b: u32, c: u32, selector: u32) {
    amdgpu_physical_entry! {
        gfx942_xnack_off_wave64;
        output=output; a=a; b=b; c=c; selector=selector;
        label(250);
        s_load_dwordx2(s_pair(8), kernarg, 0);
        s_load_dwordx2(s_pair(10), kernarg, 8);
        s_load_dword(s(12), kernarg, 16);
        s_load_dword(s(13), kernarg, 20);
        s_load_dword(s(14), kernarg, 24);
        s_load_dword(s(15), kernarg, 28);
        s_waitcnt_lgkmcnt0();
        s_lshl_b32(s(16), s(2), 6);
        v_add_u32_e32(v(2), s(16), v(0));
        v_mov_b32_e32(v(3), zero);
        v_mov_b32_e32(v(4), s(9));
        v_mov_b32_e32(v(8), s(12));
        v_lshlrev_b64(v_pair(6), v_pair(2), 2);
        v_add_co_u32_e32(v(6), s(8), v(6));
        v_addc_co_u32_e32(v(7), v(3), v(7));
        v_cmp_gt_u64_e32(s_pair(10), v_pair(2));
        s_and_saveexec_b64(s_pair(18));
        global_store_dword(v_pair(6), v(8));
        s_waitcnt_vmcnt0();
        s_mov_b64_exec(s_pair(18));
        s_endpgm0();
    }
}
