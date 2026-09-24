//! Actual Rust source controls; no fixture is source-custody evidence until compiled.
/// Required/max workgroup128, exactly one group, both Wave64 waves participate.
/// Input128 remains required even for zero/short output. LDS publication is
/// explicit write/wait/barrier/read/wait, never an inferred compiler barrier.
use fe2o3_device::{DisjointSlice, amdgpu_physical_lds_exchange, kernel};

#[cfg(feature = "physical-lds-exchange-foreign-marker-v22")]
#[inline(never)]
fn foreign_marker() {}

#[cfg(feature = "physical-lds-exchange-one-v22")]
#[kernel(typed, launch(required=[128,1,1], max=[128,1,1], max_grid=[1,1,1]))]
pub fn physical_lds_exchange_one(input: &[u32], output: DisjointSlice<u32>) {
    amdgpu_physical_lds_exchange! {
        gfx942_xnack_off_wave64;
        input=input; output=output;
        lds=static_u32_frame(0, 512, 4, 1);
        label(0);
        s_load_dwordx2(s_pair(8), kernarg, 0);
        s_load_dwordx2(s_pair(10), kernarg, 8);
        s_load_dwordx2(s_pair(12), kernarg, 16);
        s_load_dwordx2(s_pair(14), kernarg, 24);
        s_waitcnt_lgkmcnt0();
        s_lshl_b32(s(16), s(2), 7);
        v_add_u32_e32(v(2), s(16), v(0));
        v_mov_b32_e32(v(3), zero);
        v_mov_b32_e32(v(4), s(9));
        v_lshlrev_b64(v_pair(6), v_pair(2), 2);
        v_add_co_u32_e32(v(6), s(8), v(6));
        v_addc_co_u32_e32(v(7), v(4), v(7));
        global_load_dword(v(8), v_pair(6));
        s_waitcnt_vmcnt0();
        v_lshlrev_b32(v(16), v(0), 2);
        ds_write_b32(v(16), v(8));
        s_waitcnt_lgkmcnt0();
        s_barrier();
        v_xor_b32_e32(v(17), v(0), 64);
        v_lshlrev_b32(v(17), v(17), 2);
        ds_read_b32(v(18), v(17));
        s_waitcnt_lgkmcnt0();
        v_mov_b32_e32(v(5), s(13));
        v_lshlrev_b64(v_pair(10), v_pair(2), 2);
        v_add_co_u32_e32(v(10), s(12), v(10));
        v_addc_co_u32_e32(v(11), v(5), v(11));
        v_cmp_gt_u64_e32(s_pair(14), v_pair(2));
        s_and_saveexec_b64(s_pair(18));
        global_store_dword(v_pair(10), v(18));
        s_waitcnt_vmcnt0();
        s_mov_b64_exec(s_pair(18));
        s_endpgm0();
    }
}

#[cfg(feature = "physical-lds-exchange-registers-v22")]
#[kernel(typed, launch(required=[128,1,1], max=[128,1,1], max_grid=[1,1,1]))]
pub fn physical_lds_exchange_registers(input: &[u32], output: DisjointSlice<u32>) {
    amdgpu_physical_lds_exchange! {
        gfx942_xnack_off_wave64;
        input=input; output=output;
        lds=static_u32_frame(0, 512, 4, 1);
        label(0);
        s_load_dwordx2(s_pair(8), kernarg, 0);
        s_load_dwordx2(s_pair(10), kernarg, 8);
        s_load_dwordx2(s_pair(12), kernarg, 16);
        s_load_dwordx2(s_pair(14), kernarg, 24);
        s_waitcnt_lgkmcnt0();
        s_lshl_b32(s(16), s(2), 7);
        v_add_u32_e32(v(2), s(16), v(0));
        v_mov_b32_e32(v(3), zero);
        v_mov_b32_e32(v(4), s(9));
        v_lshlrev_b64(v_pair(6), v_pair(2), 2);
        v_add_co_u32_e32(v(6), s(8), v(6));
        v_addc_co_u32_e32(v(7), v(4), v(7));
        global_load_dword(v(22), v_pair(6));
        s_waitcnt_vmcnt0();
        v_lshlrev_b32(v(24), v(0), 2);
        ds_write_b32(v(24), v(22));
        s_waitcnt_lgkmcnt0();
        s_barrier();
        v_xor_b32_e32(v(25), v(0), 64);
        v_lshlrev_b32(v(25), v(25), 2);
        ds_read_b32(v(26), v(25));
        s_waitcnt_lgkmcnt0();
        v_mov_b32_e32(v(5), s(13));
        v_lshlrev_b64(v_pair(10), v_pair(2), 2);
        v_add_co_u32_e32(v(10), s(12), v(10));
        v_addc_co_u32_e32(v(11), v(5), v(11));
        v_cmp_gt_u64_e32(s_pair(14), v_pair(2));
        s_and_saveexec_b64(s_pair(18));
        global_store_dword(v_pair(10), v(26));
        s_waitcnt_vmcnt0();
        s_mov_b64_exec(s_pair(18));
        s_endpgm0();
    }
}

#[cfg(feature = "physical-lds-exchange-wrong-launch-v22")]
#[kernel(typed, launch(required=[64,1,1], max=[64,1,1], max_grid=[1,1,1]))]
pub fn physical_lds_exchange_wrong_launch(input: &[u32], output: DisjointSlice<u32>) {
    amdgpu_physical_lds_exchange! {
        gfx942_xnack_off_wave64;
        input=input; output=output;
        lds=static_u32_frame(0, 512, 4, 1);
        label(0);
        s_load_dwordx2(s_pair(8), kernarg, 0);
        s_load_dwordx2(s_pair(10), kernarg, 8);
        s_load_dwordx2(s_pair(12), kernarg, 16);
        s_load_dwordx2(s_pair(14), kernarg, 24);
        s_waitcnt_lgkmcnt0();
        s_lshl_b32(s(16), s(2), 7);
        v_add_u32_e32(v(2), s(16), v(0));
        v_mov_b32_e32(v(3), zero);
        v_mov_b32_e32(v(4), s(9));
        v_lshlrev_b64(v_pair(6), v_pair(2), 2);
        v_add_co_u32_e32(v(6), s(8), v(6));
        v_addc_co_u32_e32(v(7), v(4), v(7));
        global_load_dword(v(8), v_pair(6));
        s_waitcnt_vmcnt0();
        v_lshlrev_b32(v(16), v(0), 2);
        ds_write_b32(v(16), v(8));
        s_waitcnt_lgkmcnt0();
        s_barrier();
        v_xor_b32_e32(v(17), v(0), 64);
        v_lshlrev_b32(v(17), v(17), 2);
        ds_read_b32(v(18), v(17));
        s_waitcnt_lgkmcnt0();
        v_mov_b32_e32(v(5), s(13));
        v_lshlrev_b64(v_pair(10), v_pair(2), 2);
        v_add_co_u32_e32(v(10), s(12), v(10));
        v_addc_co_u32_e32(v(11), v(5), v(11));
        v_cmp_gt_u64_e32(s_pair(14), v_pair(2));
        s_and_saveexec_b64(s_pair(18));
        global_store_dword(v_pair(10), v(18));
        s_waitcnt_vmcnt0();
        s_mov_b64_exec(s_pair(18));
        s_endpgm0();
    }
}

#[cfg(feature = "physical-lds-exchange-dynamic-grid-v22")]
#[kernel(typed, launch(required=[128,1,1], max=[128,1,1]))]
pub fn physical_lds_exchange_dynamic_grid(input: &[u32], output: DisjointSlice<u32>) {
    amdgpu_physical_lds_exchange! {
        gfx942_xnack_off_wave64;
        input=input; output=output;
        lds=static_u32_frame(0, 512, 4, 1);
        label(0);
        s_load_dwordx2(s_pair(8), kernarg, 0);
        s_load_dwordx2(s_pair(10), kernarg, 8);
        s_load_dwordx2(s_pair(12), kernarg, 16);
        s_load_dwordx2(s_pair(14), kernarg, 24);
        s_waitcnt_lgkmcnt0();
        s_lshl_b32(s(16), s(2), 7);
        v_add_u32_e32(v(2), s(16), v(0));
        v_mov_b32_e32(v(3), zero);
        v_mov_b32_e32(v(4), s(9));
        v_lshlrev_b64(v_pair(6), v_pair(2), 2);
        v_add_co_u32_e32(v(6), s(8), v(6));
        v_addc_co_u32_e32(v(7), v(4), v(7));
        global_load_dword(v(8), v_pair(6));
        s_waitcnt_vmcnt0();
        v_lshlrev_b32(v(16), v(0), 2);
        ds_write_b32(v(16), v(8));
        s_waitcnt_lgkmcnt0();
        s_barrier();
        v_xor_b32_e32(v(17), v(0), 64);
        v_lshlrev_b32(v(17), v(17), 2);
        ds_read_b32(v(18), v(17));
        s_waitcnt_lgkmcnt0();
        v_mov_b32_e32(v(5), s(13));
        v_lshlrev_b64(v_pair(10), v_pair(2), 2);
        v_add_co_u32_e32(v(10), s(12), v(10));
        v_addc_co_u32_e32(v(11), v(5), v(11));
        v_cmp_gt_u64_e32(s_pair(14), v_pair(2));
        s_and_saveexec_b64(s_pair(18));
        global_store_dword(v_pair(10), v(18));
        s_waitcnt_vmcnt0();
        s_mov_b64_exec(s_pair(18));
        s_endpgm0();
    }
}

#[cfg(feature = "physical-lds-exchange-frame-base-v22")]
#[kernel(typed, launch(required=[128,1,1], max=[128,1,1], max_grid=[1,1,1]))]
pub fn physical_lds_exchange_frame_base(input: &[u32], output: DisjointSlice<u32>) {
    amdgpu_physical_lds_exchange! {
        gfx942_xnack_off_wave64;
        input=input; output=output;
        lds=static_u32_frame(4, 512, 4, 1);
        label(0);
        s_load_dwordx2(s_pair(8), kernarg, 0);
        s_load_dwordx2(s_pair(10), kernarg, 8);
        s_load_dwordx2(s_pair(12), kernarg, 16);
        s_load_dwordx2(s_pair(14), kernarg, 24);
        s_waitcnt_lgkmcnt0();
        s_lshl_b32(s(16), s(2), 7);
        v_add_u32_e32(v(2), s(16), v(0));
        v_mov_b32_e32(v(3), zero);
        v_mov_b32_e32(v(4), s(9));
        v_lshlrev_b64(v_pair(6), v_pair(2), 2);
        v_add_co_u32_e32(v(6), s(8), v(6));
        v_addc_co_u32_e32(v(7), v(4), v(7));
        global_load_dword(v(8), v_pair(6));
        s_waitcnt_vmcnt0();
        v_lshlrev_b32(v(16), v(0), 2);
        ds_write_b32(v(16), v(8));
        s_waitcnt_lgkmcnt0();
        s_barrier();
        v_xor_b32_e32(v(17), v(0), 64);
        v_lshlrev_b32(v(17), v(17), 2);
        ds_read_b32(v(18), v(17));
        s_waitcnt_lgkmcnt0();
        v_mov_b32_e32(v(5), s(13));
        v_lshlrev_b64(v_pair(10), v_pair(2), 2);
        v_add_co_u32_e32(v(10), s(12), v(10));
        v_addc_co_u32_e32(v(11), v(5), v(11));
        v_cmp_gt_u64_e32(s_pair(14), v_pair(2));
        s_and_saveexec_b64(s_pair(18));
        global_store_dword(v_pair(10), v(18));
        s_waitcnt_vmcnt0();
        s_mov_b64_exec(s_pair(18));
        s_endpgm0();
    }
}

#[cfg(feature = "physical-lds-exchange-frame-size-v22")]
#[kernel(typed, launch(required=[128,1,1], max=[128,1,1], max_grid=[1,1,1]))]
pub fn physical_lds_exchange_frame_size(input: &[u32], output: DisjointSlice<u32>) {
    amdgpu_physical_lds_exchange! {
        gfx942_xnack_off_wave64;
        input=input; output=output;
        lds=static_u32_frame(0, 508, 4, 1);
        label(0);
        s_load_dwordx2(s_pair(8), kernarg, 0);
        s_load_dwordx2(s_pair(10), kernarg, 8);
        s_load_dwordx2(s_pair(12), kernarg, 16);
        s_load_dwordx2(s_pair(14), kernarg, 24);
        s_waitcnt_lgkmcnt0();
        s_lshl_b32(s(16), s(2), 7);
        v_add_u32_e32(v(2), s(16), v(0));
        v_mov_b32_e32(v(3), zero);
        v_mov_b32_e32(v(4), s(9));
        v_lshlrev_b64(v_pair(6), v_pair(2), 2);
        v_add_co_u32_e32(v(6), s(8), v(6));
        v_addc_co_u32_e32(v(7), v(4), v(7));
        global_load_dword(v(8), v_pair(6));
        s_waitcnt_vmcnt0();
        v_lshlrev_b32(v(16), v(0), 2);
        ds_write_b32(v(16), v(8));
        s_waitcnt_lgkmcnt0();
        s_barrier();
        v_xor_b32_e32(v(17), v(0), 64);
        v_lshlrev_b32(v(17), v(17), 2);
        ds_read_b32(v(18), v(17));
        s_waitcnt_lgkmcnt0();
        v_mov_b32_e32(v(5), s(13));
        v_lshlrev_b64(v_pair(10), v_pair(2), 2);
        v_add_co_u32_e32(v(10), s(12), v(10));
        v_addc_co_u32_e32(v(11), v(5), v(11));
        v_cmp_gt_u64_e32(s_pair(14), v_pair(2));
        s_and_saveexec_b64(s_pair(18));
        global_store_dword(v_pair(10), v(18));
        s_waitcnt_vmcnt0();
        s_mov_b64_exec(s_pair(18));
        s_endpgm0();
    }
}

#[cfg(feature = "physical-lds-exchange-frame-alignment-v22")]
#[kernel(typed, launch(required=[128,1,1], max=[128,1,1], max_grid=[1,1,1]))]
pub fn physical_lds_exchange_frame_alignment(input: &[u32], output: DisjointSlice<u32>) {
    amdgpu_physical_lds_exchange! {
        gfx942_xnack_off_wave64;
        input=input; output=output;
        lds=static_u32_frame(0, 512, 8, 1);
        label(0);
        s_load_dwordx2(s_pair(8), kernarg, 0);
        s_load_dwordx2(s_pair(10), kernarg, 8);
        s_load_dwordx2(s_pair(12), kernarg, 16);
        s_load_dwordx2(s_pair(14), kernarg, 24);
        s_waitcnt_lgkmcnt0();
        s_lshl_b32(s(16), s(2), 7);
        v_add_u32_e32(v(2), s(16), v(0));
        v_mov_b32_e32(v(3), zero);
        v_mov_b32_e32(v(4), s(9));
        v_lshlrev_b64(v_pair(6), v_pair(2), 2);
        v_add_co_u32_e32(v(6), s(8), v(6));
        v_addc_co_u32_e32(v(7), v(4), v(7));
        global_load_dword(v(8), v_pair(6));
        s_waitcnt_vmcnt0();
        v_lshlrev_b32(v(16), v(0), 2);
        ds_write_b32(v(16), v(8));
        s_waitcnt_lgkmcnt0();
        s_barrier();
        v_xor_b32_e32(v(17), v(0), 64);
        v_lshlrev_b32(v(17), v(17), 2);
        ds_read_b32(v(18), v(17));
        s_waitcnt_lgkmcnt0();
        v_mov_b32_e32(v(5), s(13));
        v_lshlrev_b64(v_pair(10), v_pair(2), 2);
        v_add_co_u32_e32(v(10), s(12), v(10));
        v_addc_co_u32_e32(v(11), v(5), v(11));
        v_cmp_gt_u64_e32(s_pair(14), v_pair(2));
        s_and_saveexec_b64(s_pair(18));
        global_store_dword(v_pair(10), v(18));
        s_waitcnt_vmcnt0();
        s_mov_b64_exec(s_pair(18));
        s_endpgm0();
    }
}

#[cfg(feature = "physical-lds-exchange-frame-epoch-v22")]
#[kernel(typed, launch(required=[128,1,1], max=[128,1,1], max_grid=[1,1,1]))]
pub fn physical_lds_exchange_frame_epoch(input: &[u32], output: DisjointSlice<u32>) {
    amdgpu_physical_lds_exchange! {
        gfx942_xnack_off_wave64;
        input=input; output=output;
        lds=static_u32_frame(0, 512, 4, 2);
        label(0);
        s_load_dwordx2(s_pair(8), kernarg, 0);
        s_load_dwordx2(s_pair(10), kernarg, 8);
        s_load_dwordx2(s_pair(12), kernarg, 16);
        s_load_dwordx2(s_pair(14), kernarg, 24);
        s_waitcnt_lgkmcnt0();
        s_lshl_b32(s(16), s(2), 7);
        v_add_u32_e32(v(2), s(16), v(0));
        v_mov_b32_e32(v(3), zero);
        v_mov_b32_e32(v(4), s(9));
        v_lshlrev_b64(v_pair(6), v_pair(2), 2);
        v_add_co_u32_e32(v(6), s(8), v(6));
        v_addc_co_u32_e32(v(7), v(4), v(7));
        global_load_dword(v(8), v_pair(6));
        s_waitcnt_vmcnt0();
        v_lshlrev_b32(v(16), v(0), 2);
        ds_write_b32(v(16), v(8));
        s_waitcnt_lgkmcnt0();
        s_barrier();
        v_xor_b32_e32(v(17), v(0), 64);
        v_lshlrev_b32(v(17), v(17), 2);
        ds_read_b32(v(18), v(17));
        s_waitcnt_lgkmcnt0();
        v_mov_b32_e32(v(5), s(13));
        v_lshlrev_b64(v_pair(10), v_pair(2), 2);
        v_add_co_u32_e32(v(10), s(12), v(10));
        v_addc_co_u32_e32(v(11), v(5), v(11));
        v_cmp_gt_u64_e32(s_pair(14), v_pair(2));
        s_and_saveexec_b64(s_pair(18));
        global_store_dword(v_pair(10), v(18));
        s_waitcnt_vmcnt0();
        s_mov_b64_exec(s_pair(18));
        s_endpgm0();
    }
}

#[cfg(feature = "physical-lds-exchange-missing-write-wait-v22")]
#[kernel(typed, launch(required=[128,1,1], max=[128,1,1], max_grid=[1,1,1]))]
pub fn physical_lds_exchange_missing_write_wait(input: &[u32], output: DisjointSlice<u32>) {
    amdgpu_physical_lds_exchange! {
        gfx942_xnack_off_wave64;
        input=input; output=output;
        lds=static_u32_frame(0, 512, 4, 1);
        label(0);
        s_load_dwordx2(s_pair(8), kernarg, 0);
        s_load_dwordx2(s_pair(10), kernarg, 8);
        s_load_dwordx2(s_pair(12), kernarg, 16);
        s_load_dwordx2(s_pair(14), kernarg, 24);
        s_waitcnt_lgkmcnt0();
        s_lshl_b32(s(16), s(2), 7);
        v_add_u32_e32(v(2), s(16), v(0));
        v_mov_b32_e32(v(3), zero);
        v_mov_b32_e32(v(4), s(9));
        v_lshlrev_b64(v_pair(6), v_pair(2), 2);
        v_add_co_u32_e32(v(6), s(8), v(6));
        v_addc_co_u32_e32(v(7), v(4), v(7));
        global_load_dword(v(8), v_pair(6));
        s_waitcnt_vmcnt0();
        v_lshlrev_b32(v(16), v(0), 2);
        ds_write_b32(v(16), v(8));
        s_barrier();
        v_xor_b32_e32(v(17), v(0), 64);
        v_lshlrev_b32(v(17), v(17), 2);
        ds_read_b32(v(18), v(17));
        s_waitcnt_lgkmcnt0();
        v_mov_b32_e32(v(5), s(13));
        v_lshlrev_b64(v_pair(10), v_pair(2), 2);
        v_add_co_u32_e32(v(10), s(12), v(10));
        v_addc_co_u32_e32(v(11), v(5), v(11));
        v_cmp_gt_u64_e32(s_pair(14), v_pair(2));
        s_and_saveexec_b64(s_pair(18));
        global_store_dword(v_pair(10), v(18));
        s_waitcnt_vmcnt0();
        s_mov_b64_exec(s_pair(18));
        s_endpgm0();
    }
}

#[cfg(feature = "physical-lds-exchange-missing-barrier-v22")]
#[kernel(typed, launch(required=[128,1,1], max=[128,1,1], max_grid=[1,1,1]))]
pub fn physical_lds_exchange_missing_barrier(input: &[u32], output: DisjointSlice<u32>) {
    amdgpu_physical_lds_exchange! {
        gfx942_xnack_off_wave64;
        input=input; output=output;
        lds=static_u32_frame(0, 512, 4, 1);
        label(0);
        s_load_dwordx2(s_pair(8), kernarg, 0);
        s_load_dwordx2(s_pair(10), kernarg, 8);
        s_load_dwordx2(s_pair(12), kernarg, 16);
        s_load_dwordx2(s_pair(14), kernarg, 24);
        s_waitcnt_lgkmcnt0();
        s_lshl_b32(s(16), s(2), 7);
        v_add_u32_e32(v(2), s(16), v(0));
        v_mov_b32_e32(v(3), zero);
        v_mov_b32_e32(v(4), s(9));
        v_lshlrev_b64(v_pair(6), v_pair(2), 2);
        v_add_co_u32_e32(v(6), s(8), v(6));
        v_addc_co_u32_e32(v(7), v(4), v(7));
        global_load_dword(v(8), v_pair(6));
        s_waitcnt_vmcnt0();
        v_lshlrev_b32(v(16), v(0), 2);
        ds_write_b32(v(16), v(8));
        s_waitcnt_lgkmcnt0();
        v_xor_b32_e32(v(17), v(0), 64);
        v_lshlrev_b32(v(17), v(17), 2);
        ds_read_b32(v(18), v(17));
        s_waitcnt_lgkmcnt0();
        v_mov_b32_e32(v(5), s(13));
        v_lshlrev_b64(v_pair(10), v_pair(2), 2);
        v_add_co_u32_e32(v(10), s(12), v(10));
        v_addc_co_u32_e32(v(11), v(5), v(11));
        v_cmp_gt_u64_e32(s_pair(14), v_pair(2));
        s_and_saveexec_b64(s_pair(18));
        global_store_dword(v_pair(10), v(18));
        s_waitcnt_vmcnt0();
        s_mov_b64_exec(s_pair(18));
        s_endpgm0();
    }
}

#[cfg(feature = "physical-lds-exchange-missing-read-wait-v22")]
#[kernel(typed, launch(required=[128,1,1], max=[128,1,1], max_grid=[1,1,1]))]
pub fn physical_lds_exchange_missing_read_wait(input: &[u32], output: DisjointSlice<u32>) {
    amdgpu_physical_lds_exchange! {
        gfx942_xnack_off_wave64;
        input=input; output=output;
        lds=static_u32_frame(0, 512, 4, 1);
        label(0);
        s_load_dwordx2(s_pair(8), kernarg, 0);
        s_load_dwordx2(s_pair(10), kernarg, 8);
        s_load_dwordx2(s_pair(12), kernarg, 16);
        s_load_dwordx2(s_pair(14), kernarg, 24);
        s_waitcnt_lgkmcnt0();
        s_lshl_b32(s(16), s(2), 7);
        v_add_u32_e32(v(2), s(16), v(0));
        v_mov_b32_e32(v(3), zero);
        v_mov_b32_e32(v(4), s(9));
        v_lshlrev_b64(v_pair(6), v_pair(2), 2);
        v_add_co_u32_e32(v(6), s(8), v(6));
        v_addc_co_u32_e32(v(7), v(4), v(7));
        global_load_dword(v(8), v_pair(6));
        s_waitcnt_vmcnt0();
        v_lshlrev_b32(v(16), v(0), 2);
        ds_write_b32(v(16), v(8));
        s_waitcnt_lgkmcnt0();
        s_barrier();
        v_xor_b32_e32(v(17), v(0), 64);
        v_lshlrev_b32(v(17), v(17), 2);
        ds_read_b32(v(18), v(17));
        v_mov_b32_e32(v(5), s(13));
        v_lshlrev_b64(v_pair(10), v_pair(2), 2);
        v_add_co_u32_e32(v(10), s(12), v(10));
        v_addc_co_u32_e32(v(11), v(5), v(11));
        v_cmp_gt_u64_e32(s_pair(14), v_pair(2));
        s_and_saveexec_b64(s_pair(18));
        global_store_dword(v_pair(10), v(18));
        s_waitcnt_vmcnt0();
        s_mov_b64_exec(s_pair(18));
        s_endpgm0();
    }
}

#[cfg(feature = "physical-lds-exchange-wrong-peer-v22")]
#[kernel(typed, launch(required=[128,1,1], max=[128,1,1], max_grid=[1,1,1]))]
pub fn physical_lds_exchange_wrong_peer(input: &[u32], output: DisjointSlice<u32>) {
    amdgpu_physical_lds_exchange! {
        gfx942_xnack_off_wave64;
        input=input; output=output;
        lds=static_u32_frame(0, 512, 4, 1);
        label(0);
        s_load_dwordx2(s_pair(8), kernarg, 0);
        s_load_dwordx2(s_pair(10), kernarg, 8);
        s_load_dwordx2(s_pair(12), kernarg, 16);
        s_load_dwordx2(s_pair(14), kernarg, 24);
        s_waitcnt_lgkmcnt0();
        s_lshl_b32(s(16), s(2), 7);
        v_add_u32_e32(v(2), s(16), v(0));
        v_mov_b32_e32(v(3), zero);
        v_mov_b32_e32(v(4), s(9));
        v_lshlrev_b64(v_pair(6), v_pair(2), 2);
        v_add_co_u32_e32(v(6), s(8), v(6));
        v_addc_co_u32_e32(v(7), v(4), v(7));
        global_load_dword(v(8), v_pair(6));
        s_waitcnt_vmcnt0();
        v_lshlrev_b32(v(16), v(0), 2);
        ds_write_b32(v(16), v(8));
        s_waitcnt_lgkmcnt0();
        s_barrier();
        v_xor_b32_e32(v(17), v(2), 64);
        v_lshlrev_b32(v(17), v(17), 2);
        ds_read_b32(v(18), v(17));
        s_waitcnt_lgkmcnt0();
        v_mov_b32_e32(v(5), s(13));
        v_lshlrev_b64(v_pair(10), v_pair(2), 2);
        v_add_co_u32_e32(v(10), s(12), v(10));
        v_addc_co_u32_e32(v(11), v(5), v(11));
        v_cmp_gt_u64_e32(s_pair(14), v_pair(2));
        s_and_saveexec_b64(s_pair(18));
        global_store_dword(v_pair(10), v(18));
        s_waitcnt_vmcnt0();
        s_mov_b64_exec(s_pair(18));
        s_endpgm0();
    }
}

#[cfg(feature = "physical-lds-exchange-wrong-lds-address-v22")]
#[kernel(typed, launch(required=[128,1,1], max=[128,1,1], max_grid=[1,1,1]))]
pub fn physical_lds_exchange_wrong_lds_address(input: &[u32], output: DisjointSlice<u32>) {
    amdgpu_physical_lds_exchange! {
        gfx942_xnack_off_wave64;
        input=input; output=output;
        lds=static_u32_frame(0, 512, 4, 1);
        label(0);
        s_load_dwordx2(s_pair(8), kernarg, 0);
        s_load_dwordx2(s_pair(10), kernarg, 8);
        s_load_dwordx2(s_pair(12), kernarg, 16);
        s_load_dwordx2(s_pair(14), kernarg, 24);
        s_waitcnt_lgkmcnt0();
        s_lshl_b32(s(16), s(2), 7);
        v_add_u32_e32(v(2), s(16), v(0));
        v_mov_b32_e32(v(3), zero);
        v_mov_b32_e32(v(4), s(9));
        v_lshlrev_b64(v_pair(6), v_pair(2), 2);
        v_add_co_u32_e32(v(6), s(8), v(6));
        v_addc_co_u32_e32(v(7), v(4), v(7));
        global_load_dword(v(8), v_pair(6));
        s_waitcnt_vmcnt0();
        v_lshlrev_b32(v(16), v(0), 2);
        ds_write_b32(v(2), v(8));
        s_waitcnt_lgkmcnt0();
        s_barrier();
        v_xor_b32_e32(v(17), v(0), 64);
        v_lshlrev_b32(v(17), v(17), 2);
        ds_read_b32(v(18), v(17));
        s_waitcnt_lgkmcnt0();
        v_mov_b32_e32(v(5), s(13));
        v_lshlrev_b64(v_pair(10), v_pair(2), 2);
        v_add_co_u32_e32(v(10), s(12), v(10));
        v_addc_co_u32_e32(v(11), v(5), v(11));
        v_cmp_gt_u64_e32(s_pair(14), v_pair(2));
        s_and_saveexec_b64(s_pair(18));
        global_store_dword(v_pair(10), v(18));
        s_waitcnt_vmcnt0();
        s_mov_b64_exec(s_pair(18));
        s_endpgm0();
    }
}

#[cfg(feature = "physical-lds-exchange-wrong-store-v22")]
#[kernel(typed, launch(required=[128,1,1], max=[128,1,1], max_grid=[1,1,1]))]
pub fn physical_lds_exchange_wrong_store(input: &[u32], output: DisjointSlice<u32>) {
    amdgpu_physical_lds_exchange! {
        gfx942_xnack_off_wave64;
        input=input; output=output;
        lds=static_u32_frame(0, 512, 4, 1);
        label(0);
        s_load_dwordx2(s_pair(8), kernarg, 0);
        s_load_dwordx2(s_pair(10), kernarg, 8);
        s_load_dwordx2(s_pair(12), kernarg, 16);
        s_load_dwordx2(s_pair(14), kernarg, 24);
        s_waitcnt_lgkmcnt0();
        s_lshl_b32(s(16), s(2), 7);
        v_add_u32_e32(v(2), s(16), v(0));
        v_mov_b32_e32(v(3), zero);
        v_mov_b32_e32(v(4), s(9));
        v_lshlrev_b64(v_pair(6), v_pair(2), 2);
        v_add_co_u32_e32(v(6), s(8), v(6));
        v_addc_co_u32_e32(v(7), v(4), v(7));
        global_load_dword(v(8), v_pair(6));
        s_waitcnt_vmcnt0();
        v_lshlrev_b32(v(16), v(0), 2);
        ds_write_b32(v(16), v(8));
        s_waitcnt_lgkmcnt0();
        s_barrier();
        v_xor_b32_e32(v(17), v(0), 64);
        v_lshlrev_b32(v(17), v(17), 2);
        ds_read_b32(v(18), v(17));
        s_waitcnt_lgkmcnt0();
        v_mov_b32_e32(v(5), s(13));
        v_lshlrev_b64(v_pair(10), v_pair(2), 2);
        v_add_co_u32_e32(v(10), s(12), v(10));
        v_addc_co_u32_e32(v(11), v(5), v(11));
        v_cmp_gt_u64_e32(s_pair(14), v_pair(2));
        s_and_saveexec_b64(s_pair(18));
        global_store_dword(v_pair(10), v(8));
        s_waitcnt_vmcnt0();
        s_mov_b64_exec(s_pair(18));
        s_endpgm0();
    }
}

#[cfg(feature = "physical-lds-exchange-missing-vm-v22")]
#[kernel(typed, launch(required=[128,1,1], max=[128,1,1], max_grid=[1,1,1]))]
pub fn physical_lds_exchange_missing_vm(input: &[u32], output: DisjointSlice<u32>) {
    amdgpu_physical_lds_exchange! {
        gfx942_xnack_off_wave64;
        input=input; output=output;
        lds=static_u32_frame(0, 512, 4, 1);
        label(0);
        s_load_dwordx2(s_pair(8), kernarg, 0);
        s_load_dwordx2(s_pair(10), kernarg, 8);
        s_load_dwordx2(s_pair(12), kernarg, 16);
        s_load_dwordx2(s_pair(14), kernarg, 24);
        s_waitcnt_lgkmcnt0();
        s_lshl_b32(s(16), s(2), 7);
        v_add_u32_e32(v(2), s(16), v(0));
        v_mov_b32_e32(v(3), zero);
        v_mov_b32_e32(v(4), s(9));
        v_lshlrev_b64(v_pair(6), v_pair(2), 2);
        v_add_co_u32_e32(v(6), s(8), v(6));
        v_addc_co_u32_e32(v(7), v(4), v(7));
        global_load_dword(v(8), v_pair(6));
        v_lshlrev_b32(v(16), v(0), 2);
        ds_write_b32(v(16), v(8));
        s_waitcnt_lgkmcnt0();
        s_barrier();
        v_xor_b32_e32(v(17), v(0), 64);
        v_lshlrev_b32(v(17), v(17), 2);
        ds_read_b32(v(18), v(17));
        s_waitcnt_lgkmcnt0();
        v_mov_b32_e32(v(5), s(13));
        v_lshlrev_b64(v_pair(10), v_pair(2), 2);
        v_add_co_u32_e32(v(10), s(12), v(10));
        v_addc_co_u32_e32(v(11), v(5), v(11));
        v_cmp_gt_u64_e32(s_pair(14), v_pair(2));
        s_and_saveexec_b64(s_pair(18));
        global_store_dword(v_pair(10), v(18));
        s_waitcnt_vmcnt0();
        s_mov_b64_exec(s_pair(18));
        s_endpgm0();
    }
}

#[cfg(feature = "physical-lds-exchange-wrong-carry-v22")]
#[kernel(typed, launch(required=[128,1,1], max=[128,1,1], max_grid=[1,1,1]))]
pub fn physical_lds_exchange_wrong_carry(input: &[u32], output: DisjointSlice<u32>) {
    amdgpu_physical_lds_exchange! {
        gfx942_xnack_off_wave64;
        input=input; output=output;
        lds=static_u32_frame(0, 512, 4, 1);
        label(0);
        s_load_dwordx2(s_pair(8), kernarg, 0);
        s_load_dwordx2(s_pair(10), kernarg, 8);
        s_load_dwordx2(s_pair(12), kernarg, 16);
        s_load_dwordx2(s_pair(14), kernarg, 24);
        s_waitcnt_lgkmcnt0();
        s_lshl_b32(s(16), s(2), 7);
        v_add_u32_e32(v(2), s(16), v(0));
        v_mov_b32_e32(v(3), zero);
        v_mov_b32_e32(v(4), s(9));
        v_lshlrev_b64(v_pair(6), v_pair(2), 2);
        v_add_co_u32_e32(v(6), s(8), v(6));
        v_addc_co_u32_e32(v(7), v(3), v(7));
        global_load_dword(v(8), v_pair(6));
        s_waitcnt_vmcnt0();
        v_lshlrev_b32(v(16), v(0), 2);
        ds_write_b32(v(16), v(8));
        s_waitcnt_lgkmcnt0();
        s_barrier();
        v_xor_b32_e32(v(17), v(0), 64);
        v_lshlrev_b32(v(17), v(17), 2);
        ds_read_b32(v(18), v(17));
        s_waitcnt_lgkmcnt0();
        v_mov_b32_e32(v(5), s(13));
        v_lshlrev_b64(v_pair(10), v_pair(2), 2);
        v_add_co_u32_e32(v(10), s(12), v(10));
        v_addc_co_u32_e32(v(11), v(5), v(11));
        v_cmp_gt_u64_e32(s_pair(14), v_pair(2));
        s_and_saveexec_b64(s_pair(18));
        global_store_dword(v_pair(10), v(18));
        s_waitcnt_vmcnt0();
        s_mov_b64_exec(s_pair(18));
        s_endpgm0();
    }
}

#[cfg(feature = "physical-lds-exchange-foreign-input-v22")]
#[kernel(typed, launch(required=[128,1,1], max=[128,1,1], max_grid=[1,1,1]))]
pub fn physical_lds_exchange_foreign_input(input: &[u32], output: DisjointSlice<u32>) {
    // A compile-time foreign slice reaches ownership checking without a bounds-panic helper.
    const FOREIGN_INPUT: &[u32] = &[0u32; 128];
    let input = FOREIGN_INPUT;
    amdgpu_physical_lds_exchange! {
        gfx942_xnack_off_wave64;
        input=input; output=output;
        lds=static_u32_frame(0, 512, 4, 1);
        label(0);
        s_load_dwordx2(s_pair(8), kernarg, 0);
        s_load_dwordx2(s_pair(10), kernarg, 8);
        s_load_dwordx2(s_pair(12), kernarg, 16);
        s_load_dwordx2(s_pair(14), kernarg, 24);
        s_waitcnt_lgkmcnt0();
        s_lshl_b32(s(16), s(2), 7);
        v_add_u32_e32(v(2), s(16), v(0));
        v_mov_b32_e32(v(3), zero);
        v_mov_b32_e32(v(4), s(9));
        v_lshlrev_b64(v_pair(6), v_pair(2), 2);
        v_add_co_u32_e32(v(6), s(8), v(6));
        v_addc_co_u32_e32(v(7), v(4), v(7));
        global_load_dword(v(8), v_pair(6));
        s_waitcnt_vmcnt0();
        v_lshlrev_b32(v(16), v(0), 2);
        ds_write_b32(v(16), v(8));
        s_waitcnt_lgkmcnt0();
        s_barrier();
        v_xor_b32_e32(v(17), v(0), 64);
        v_lshlrev_b32(v(17), v(17), 2);
        ds_read_b32(v(18), v(17));
        s_waitcnt_lgkmcnt0();
        v_mov_b32_e32(v(5), s(13));
        v_lshlrev_b64(v_pair(10), v_pair(2), 2);
        v_add_co_u32_e32(v(10), s(12), v(10));
        v_addc_co_u32_e32(v(11), v(5), v(11));
        v_cmp_gt_u64_e32(s_pair(14), v_pair(2));
        s_and_saveexec_b64(s_pair(18));
        global_store_dword(v_pair(10), v(18));
        s_waitcnt_vmcnt0();
        s_mov_b64_exec(s_pair(18));
        s_endpgm0();
    }
}

#[cfg(feature = "physical-lds-exchange-foreign-marker-v22")]
#[kernel(typed, launch(required=[128,1,1], max=[128,1,1], max_grid=[1,1,1]))]
pub fn physical_lds_exchange_foreign_marker(input: &[u32], output: DisjointSlice<u32>) {
    foreign_marker();
    amdgpu_physical_lds_exchange! {
        gfx942_xnack_off_wave64;
        input=input; output=output;
        lds=static_u32_frame(0, 512, 4, 1);
        label(0);
        s_load_dwordx2(s_pair(8), kernarg, 0);
        s_load_dwordx2(s_pair(10), kernarg, 8);
        s_load_dwordx2(s_pair(12), kernarg, 16);
        s_load_dwordx2(s_pair(14), kernarg, 24);
        s_waitcnt_lgkmcnt0();
        s_lshl_b32(s(16), s(2), 7);
        v_add_u32_e32(v(2), s(16), v(0));
        v_mov_b32_e32(v(3), zero);
        v_mov_b32_e32(v(4), s(9));
        v_lshlrev_b64(v_pair(6), v_pair(2), 2);
        v_add_co_u32_e32(v(6), s(8), v(6));
        v_addc_co_u32_e32(v(7), v(4), v(7));
        global_load_dword(v(8), v_pair(6));
        s_waitcnt_vmcnt0();
        v_lshlrev_b32(v(16), v(0), 2);
        ds_write_b32(v(16), v(8));
        s_waitcnt_lgkmcnt0();
        s_barrier();
        v_xor_b32_e32(v(17), v(0), 64);
        v_lshlrev_b32(v(17), v(17), 2);
        ds_read_b32(v(18), v(17));
        s_waitcnt_lgkmcnt0();
        v_mov_b32_e32(v(5), s(13));
        v_lshlrev_b64(v_pair(10), v_pair(2), 2);
        v_add_co_u32_e32(v(10), s(12), v(10));
        v_addc_co_u32_e32(v(11), v(5), v(11));
        v_cmp_gt_u64_e32(s_pair(14), v_pair(2));
        s_and_saveexec_b64(s_pair(18));
        global_store_dword(v_pair(10), v(18));
        s_waitcnt_vmcnt0();
        s_mov_b64_exec(s_pair(18));
        s_endpgm0();
    }
}

#[cfg(feature = "physical-lds-exchange-mixed-marker-v22")]
#[kernel(typed, launch(required=[128,1,1], max=[128,1,1], max_grid=[1,1,1]))]
pub fn physical_lds_exchange_mixed_marker(input: &[u32], output: DisjointSlice<u32>) {
    fe2o3_device::physical_global_copy_v1::__amdgpu_physical_global_copy_label_gfx942_v1::<0>();
    amdgpu_physical_lds_exchange! {
        gfx942_xnack_off_wave64;
        input=input; output=output;
        lds=static_u32_frame(0, 512, 4, 1);
        label(0);
        s_load_dwordx2(s_pair(8), kernarg, 0);
        s_load_dwordx2(s_pair(10), kernarg, 8);
        s_load_dwordx2(s_pair(12), kernarg, 16);
        s_load_dwordx2(s_pair(14), kernarg, 24);
        s_waitcnt_lgkmcnt0();
        s_lshl_b32(s(16), s(2), 7);
        v_add_u32_e32(v(2), s(16), v(0));
        v_mov_b32_e32(v(3), zero);
        v_mov_b32_e32(v(4), s(9));
        v_lshlrev_b64(v_pair(6), v_pair(2), 2);
        v_add_co_u32_e32(v(6), s(8), v(6));
        v_addc_co_u32_e32(v(7), v(4), v(7));
        global_load_dword(v(8), v_pair(6));
        s_waitcnt_vmcnt0();
        v_lshlrev_b32(v(16), v(0), 2);
        ds_write_b32(v(16), v(8));
        s_waitcnt_lgkmcnt0();
        s_barrier();
        v_xor_b32_e32(v(17), v(0), 64);
        v_lshlrev_b32(v(17), v(17), 2);
        ds_read_b32(v(18), v(17));
        s_waitcnt_lgkmcnt0();
        v_mov_b32_e32(v(5), s(13));
        v_lshlrev_b64(v_pair(10), v_pair(2), 2);
        v_add_co_u32_e32(v(10), s(12), v(10));
        v_addc_co_u32_e32(v(11), v(5), v(11));
        v_cmp_gt_u64_e32(s_pair(14), v_pair(2));
        s_and_saveexec_b64(s_pair(18));
        global_store_dword(v_pair(10), v(18));
        s_waitcnt_vmcnt0();
        s_mov_b64_exec(s_pair(18));
        s_endpgm0();
    }
}
