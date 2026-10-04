//! Actual source corpus for the explicit composition importer, not singleton V32.
//! Every feature is selected alone. No marker identity is inferred from a name.
use fe2o3_device::{DisjointSlice, amdgpu_ordered_program, kernel, thread};

macro_rules! region {
    ($a:expr, $b:expr, $c:expr) => {
        amdgpu_ordered_program! {
            gfx942_xnack_off_wave64;
            scratch(32); out(33); in(34) = $a; in(35) = $b; in(36) = $c;
            xor(scratch, input0, input1);
            and(out, scratch, input2);
        }
    };
}

#[cfg(any(
    feature = "ordered-composition-helper",
    feature = "ordered-composition-two-calls",
    feature = "ordered-composition-root-helper",
    feature = "ordered-composition-nested",
    feature = "ordered-composition-conditional",
    feature = "ordered-composition-too-many",
))]
#[inline(never)]
fn helper(a: u32, b: u32, c: u32) -> u32 {
    let first = region!(a, b, c);
    // Ordinary Rust transport/arithmetic is outside the assembly marker.
    (first ^ b) | c
}

#[cfg(feature = "ordered-composition-const-monos")]
#[inline(never)]
fn mono<const PACKED: u64>(a: u32, b: u32, c: u32) -> u32 {
    fe2o3_device::diagnostics::__amdgpu_ordered_program_e32_v1::<1, PACKED, 0, 0, 0>(
        a, b, c, 32, 33, 34, 35, 36,
    )
}

#[cfg(feature = "ordered-composition-scalar-helper")]
#[inline(never)]
fn scalar(a: u32, b: u32, c: u32) -> u32 {
    (a ^ b) & !c
}

#[cfg(feature = "ordered-composition-wrapping")]
#[inline(never)]
fn wrapping(a: u32, b: u32, c: u32) -> u32 {
    region!(a, b, c).wrapping_add(b).wrapping_sub(c)
}

#[cfg(feature = "ordered-composition-nested")]
#[inline(never)]
fn nested(a: u32, b: u32, c: u32) -> u32 {
    helper(a, b, c) ^ 1
}

#[cfg(feature = "ordered-composition-wrong-abi")]
#[inline(never)]
fn wrong_abi(a: u32, b: u64, c: u32) -> u32 {
    region!(a, b as u32, c)
}

#[cfg(feature = "ordered-composition-foreign-marker")]
#[inline(never)]
fn __amdgpu_ordered_program_e32_v1<
    const COUNT: u8,
    const A: u64,
    const B: u64,
    const C: u64,
    const D: u64,
>(
    a: u32,
    b: u32,
    c: u32,
    scratch: u8,
    output: u8,
    i0: u8,
    i1: u8,
    i2: u8,
) -> u32 {
    let _ = (COUNT, A, B, C, D, scratch, output, i0, i1, i2);
    a ^ b ^ c
}

#[cfg_attr(feature = "ordered-composition-wrong-launch",
    kernel(typed, launch(required = [32, 1, 1], max = [32, 1, 1])))]
#[cfg_attr(all(not(feature = "ordered-composition-wrong-launch"),
    not(feature = "ordered-composition-finite-grid")),
    kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1])))]
#[cfg_attr(all(not(feature = "ordered-composition-wrong-launch"),
    feature = "ordered-composition-finite-grid"),
    kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1], max_grid = [2, 1, 1])))]
pub fn composed_u32(mut output: DisjointSlice<u32>, a: u32, b: u32, c: u32) {
    #[cfg(any(
        feature = "ordered-composition-root",
        feature = "ordered-composition-wrong-launch"
    ))]
    let value = {
        let first = region!(a, b, c);
        region!(first ^ c, b, a)
    };

    #[cfg(feature = "ordered-composition-helper")]
    let value = helper(a, b, c);

    #[cfg(feature = "ordered-composition-two-calls")]
    let value = {
        let first = helper(a, b, c);
        helper(first ^ c, b, a)
    };

    #[cfg(feature = "ordered-composition-root-helper")]
    let value = {
        let first = region!(a, b, c);
        helper(first ^ c, b, a)
    };

    #[cfg(feature = "ordered-composition-const-monos")]
    let value = {
        let first = mono::<8>(a, b, c); // mov output,input0
        mono::<24>(first ^ c, b, a) // same item, distinct const Instance: input1
    };

    #[cfg(feature = "ordered-composition-scalar-helper")]
    let value = {
        let first = scalar(a, b, c);
        region!(first, b, c)
    };

    #[cfg(feature = "ordered-composition-wrapping")]
    let value = wrapping(a, b, c);

    #[cfg(feature = "ordered-composition-nested")]
    let value = nested(a, b, c);

    #[cfg(feature = "ordered-composition-conditional")]
    let value = if a == 0 { helper(a, b, c) } else { c };

    #[cfg(feature = "ordered-composition-wrong-abi")]
    let value = wrong_abi(a, u64::from(b), c);

    #[cfg(feature = "ordered-composition-foreign-marker")]
    let value = {
        let first = region!(a, b, c);
        __amdgpu_ordered_program_e32_v1::<1, 8, 0, 0, 0>(first, b, c, 32, 33, 34, 35, 36)
    };

    #[cfg(feature = "ordered-composition-too-many")]
    let value = {
        let v1 = helper(a, b, c);
        let v2 = helper(v1, b, c);
        let v3 = helper(v2, b, c);
        let v4 = helper(v3, b, c);
        let v5 = helper(v4, b, c);
        let v6 = helper(v5, b, c);
        let v7 = helper(v6, b, c);
        let v8 = helper(v7, b, c);
        helper(v8, b, c)
    };

    #[cfg(feature = "ordered-composition-dynamic-register")]
    let value = fe2o3_device::diagnostics::__amdgpu_ordered_program_e32_v1::<1, 8, 0, 0, 0>(
        a, b, c, a as u8, 33, 34, 35, 36,
    );

    #[cfg(feature = "ordered-composition-mixed-marker")]
    let value = {
        let first = region!(a, b, c);
        fe2o3_device::amdgpu_asm!(v_mov_b32(first))
    };

    let index = thread::index_1d();
    if let Some(element) = output.get_mut(index) {
        *element = value;
    }
}
