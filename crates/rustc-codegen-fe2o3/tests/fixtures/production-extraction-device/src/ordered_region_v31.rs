//! Actual source fixture; negatives are admitted by Rust but refused by the
//! closed ordered-region source profile, not by a synthetic KIR constructor.
use fe2o3_device::{DisjointSlice, amdgpu_asm, amdgpu_ordered_region, kernel, thread};

#[cfg_attr(feature = "ordered-region-wrong-launch-v31",
    kernel(typed, launch(required = [32, 1, 1], max = [32, 1, 1])))]
#[cfg_attr(not(feature = "ordered-region-wrong-launch-v31"),
    kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1])))]
pub fn ordered_xor_add(mut output: DisjointSlice<u32>, a: u32, b: u32, c: u32) {
    // The retained V31 body composes the old SSA marker and the new region.
    let a = amdgpu_asm!(v_mov_b32(a));

    #[cfg(feature = "ordered-region-dynamic-v31")]
    let region_value = fe2o3_device::diagnostics::__amdgpu_ordered_xor_add_e32_v1(
        a, b, c, a as u8, 33, 34, 35, 36,
    );

    #[cfg(feature = "ordered-region-divergent-v31")]
    let region_value = if a == 0 {
        amdgpu_ordered_region! {
            gfx942_xnack_off_wave64; scratch(32); out(33);
            in(34) = a; in(35) = b; in(36) = c; xor_add_u32_e32;
        }
    } else {
        a
    };

    #[cfg(feature = "ordered-region-alias-v31")]
    let region_value = amdgpu_ordered_region! {
        gfx942_xnack_off_wave64; scratch(32); out(32);
        in(34) = a; in(35) = b; in(36) = c; xor_add_u32_e32;
    };

    #[cfg(not(any(
        feature = "ordered-region-dynamic-v31",
        feature = "ordered-region-divergent-v31",
        feature = "ordered-region-alias-v31",
    )))]
    let region_value = amdgpu_ordered_region! {
        gfx942_xnack_off_wave64; scratch(32); out(33);
        in(34) = a; in(35) = b; in(36) = c; xor_add_u32_e32;
    };

    #[cfg(feature = "ordered-region-unused-v31")]
    let value = {
        let _ = region_value;
        a
    };
    #[cfg(not(feature = "ordered-region-unused-v31"))]
    let value = region_value;

    let index = thread::index_1d();
    if let Some(element) = output.get_mut(index) {
        *element = value;
    }
}
