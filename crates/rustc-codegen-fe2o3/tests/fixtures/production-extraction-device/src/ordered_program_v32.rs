//! Real source fixtures. Malformed descriptors use direct typed marker calls
//! so backend rejection is not confused with macro compile-time validation.
use fe2o3_device::{DisjointSlice, amdgpu_asm, amdgpu_ordered_program, kernel, thread};

#[cfg_attr(feature = "ordered-program-wrong-launch-v32",
    kernel(typed, launch(required = [32, 1, 1], max = [32, 1, 1])))]
#[cfg_attr(not(feature = "ordered-program-wrong-launch-v32"),
    kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1])))]
pub fn ordered_u32_program(mut output: DisjointSlice<u32>, a: u32, b: u32, c: u32) {
    let a = amdgpu_asm!(v_mov_b32(a));

    #[cfg(feature = "ordered-program-one-v32")]
    let region_value = amdgpu_ordered_program! {
        gfx942_xnack_off_wave64;
        scratch(32); out(33); in(34) = a; in(35) = b; in(36) = c;
        mov(out, input0);
    };

    #[cfg(feature = "ordered-program-sixteen-v32")]
    let region_value = amdgpu_ordered_program! {
        gfx942_xnack_off_wave64;
        scratch(32); out(33); in(34) = a; in(35) = b; in(36) = c;
        mov(scratch, input0);
        xor(out, input0, input1);
        and(scratch, out, input2);
        or(out, scratch, input1);
        add(scratch, out, input2);
        sub(out, scratch, input0);
        mov(scratch, out);
        xor(scratch, scratch, input1);
        or(out, scratch, input0);
        and(out, out, input2);
        add(out, out, input0);
        sub(scratch, out, input1);
        mov(out, scratch);
        xor(out, out, input2);
        mov(scratch, input1);
        mov(out, out);
    };

    #[cfg(feature = "ordered-program-dynamic-v32")]
    let region_value = fe2o3_device::diagnostics::__amdgpu_ordered_program_e32_v1::<1, 8, 0, 0, 0>(
        a, b, c, a as u8, 33, 34, 35, 36,
    );

    #[cfg(feature = "ordered-program-alias-v32")]
    let region_value = amdgpu_ordered_program! {
        gfx942_xnack_off_wave64;
        scratch(32); out(32); in(34) = a; in(35) = b; in(36) = c;
        mov(out, input0);
    };

    #[cfg(feature = "ordered-program-divergent-v32")]
    let region_value = if a == 0 {
        amdgpu_ordered_program! {
            gfx942_xnack_off_wave64;
            scratch(32); out(33); in(34) = a; in(35) = b; in(36) = c;
            mov(out, input0);
        }
    } else {
        a
    };

    #[cfg(feature = "ordered-program-invalid-count-v32")]
    let region_value = fe2o3_device::diagnostics::__amdgpu_ordered_program_e32_v1::<0, 8, 0, 0, 0>(
        a, b, c, 32, 33, 34, 35, 36,
    );

    #[cfg(feature = "ordered-program-invalid-opcode-v32")]
    let region_value = fe2o3_device::diagnostics::__amdgpu_ordered_program_e32_v1::<1, 14, 0, 0, 0>(
        a, b, c, 32, 33, 34, 35, 36,
    );

    #[cfg(feature = "ordered-program-read-before-init-v32")]
    let region_value = fe2o3_device::diagnostics::__amdgpu_ordered_program_e32_v1::<1, 56, 0, 0, 0>(
        a, b, c, 32, 33, 34, 35, 36,
    );

    #[cfg(feature = "ordered-program-padding-v32")]
    let region_value =
        fe2o3_device::diagnostics::__amdgpu_ordered_program_e32_v1::<1, 65544, 0, 0, 0>(
            a, b, c, 32, 33, 34, 35, 36,
        );

    #[cfg(not(any(
        feature = "ordered-program-one-v32",
        feature = "ordered-program-sixteen-v32",
        feature = "ordered-program-dynamic-v32",
        feature = "ordered-program-alias-v32",
        feature = "ordered-program-divergent-v32",
        feature = "ordered-program-invalid-count-v32",
        feature = "ordered-program-invalid-opcode-v32",
        feature = "ordered-program-read-before-init-v32",
        feature = "ordered-program-padding-v32",
    )))]
    let region_value = amdgpu_ordered_program! {
        gfx942_xnack_off_wave64;
        scratch(32); out(33); in(34) = a; in(35) = b; in(36) = c;
        xor(scratch, input0, input1);
        and(scratch, scratch, input2);
        xor(out, input1, scratch);
    };

    #[cfg(feature = "ordered-program-unused-v32")]
    let value = {
        let _ = region_value;
        a
    };
    #[cfg(not(feature = "ordered-program-unused-v32"))]
    let value = region_value;

    let index = thread::index_1d();
    if let Some(element) = output.get_mut(index) {
        *element = value;
    }
}
