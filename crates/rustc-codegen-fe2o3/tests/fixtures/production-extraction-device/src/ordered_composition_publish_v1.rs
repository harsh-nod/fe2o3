//! Actual-source publication inputs. Each feature is compiled separately.
use fe2o3_device::{DisjointSlice, kernel, thread};

#[cfg(feature = "ordered-composition-publish-wrapper")]
macro_rules! wrapped {
    ($a:expr, $b:expr, $c:expr) => {
        fe2o3_device::amdgpu_ordered_program! {
            gfx942_xnack_off_wave64;
            scratch(8); out(9); in(10) = $a; in(11) = $b; in(12) = $c;
            xor(scratch, input0, input1);
            and(out, scratch, input2);
        }
    };
}

#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
pub fn publish_region(mut output: DisjointSlice<u32>, a: u32, b: u32, c: u32) {
    #[cfg(feature = "ordered-composition-publish-collision")]
    let __fe2o3_region_0123456789abcdef = a;
    #[cfg(any(
        feature = "ordered-composition-publish-direct",
        feature = "ordered-composition-publish-collision"
    ))]
    let value = fe2o3_device::amdgpu_ordered_program! {
        gfx942_xnack_off_wave64;
        scratch(8); out(9); in(10) = a; in(11) = b; in(12) = c;
        xor(scratch, input0, input1);
        and(out, scratch, input2);
    };

    #[cfg(feature = "ordered-composition-publish-const")]
    const CAPTURED: u32 = 7;
    #[cfg(feature = "ordered-composition-publish-const")]
    let value = fe2o3_device::amdgpu_ordered_program! {
        gfx942_xnack_off_wave64;
        scratch(8); out(9); in(10) = a; in(11) = b; in(12) = CAPTURED;
        xor(scratch, input0, input1);
        and(out, scratch, input2);
    };

    #[cfg(feature = "ordered-composition-publish-local")]
    let local = a ^ b;
    #[cfg(feature = "ordered-composition-publish-local")]
    let value = fe2o3_device::amdgpu_ordered_program! {
        gfx942_xnack_off_wave64;
        scratch(8); out(9); in(10) = local; in(11) = b; in(12) = c;
        xor(scratch, input0, input1);
        and(out, scratch, input2);
    };

    #[cfg(feature = "ordered-composition-publish-wrapper")]
    let value = wrapped!(a, b, c);

    let index = thread::index_1d();
    if let Some(element) = output.get_mut(index) {
        *element = value;
    }
}
