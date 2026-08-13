use fe2o3_device::{Gfx942Collectives, kernel};

#[kernel(
    typed,
    namespace = "0c181b24f360a4b30f4f79e64cf579273d2239bbcdfdfea06003f40e82de7d53"
)]
pub fn gfx942_wave_lds_v1(active_flag: u32, value: u32) {
    // SAFETY: authenticated lowering replaces this constructor only for the
    // exact gfx942, wave64, 256x1x1 profile declared by the typed kernel.
    let context = unsafe { Gfx942Collectives::from_compiler() };
    // SAFETY: the typed launch profile makes all 256 work-items execute this
    // straight-line call; `active_flag` is lane-local logical participation.
    let wave_sum = unsafe { context.wave64_reduce_sum_active_u32(active_flag, value) };
    // SAFETY: authenticated lowering creates one exact 256-slot addrspace(3)
    // allocation for this non-forgeable capability.
    let mut scratch = unsafe { context.static_lds_u32x256() };
    // SAFETY: this call is straight-line and therefore has uniform physical
    // participation in every barrier for the exact 256-thread launch profile.
    let workgroup_sum =
        unsafe { context.workgroup256_reduce_sum_active_u32(&mut scratch, active_flag, value) };
    let _ = wave_sum;
    let _ = workgroup_sum;
}

fn main() {}
