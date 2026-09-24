//! Actual-source complete-body compiler qualification. The diagnostic marker
//! is an experimental frontend ABI; these words are records, not AMD encodings.
use fe2o3_device::diagnostics::__amdgpu_complete_body_gfx942_v1 as body;
use fe2o3_device::{DisjointSlice, kernel};

#[cfg(feature = "complete-body-one-v19")]
#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1], max_grid = [2, 1, 1]))]
pub fn assembly_one(output: DisjointSlice<u32>, a: u32, b: u32, c: u32, selector: u32) {
    // Label 255: mov(out, input0); compiler-guarded output store and end.
    body::<1, 1, 0x41ff, 0, 0, 0, 0x0008, 0, 0, 0>(output, a, b, c, selector, 32, 33, 34, 35, 36);
}

#[cfg(feature = "complete-body-diamond-v19")]
#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1], max_grid = [2, 1, 1]))]
pub fn assembly_diamond(output: DisjointSlice<u32>, a: u32, b: u32, c: u32, selector: u32) {
    // Labels 240/17/2/4: zero selector writes a, nonzero writes b.
    // Each arm defines out before merging at the compiler-guarded store.
    body::<4, 2, 0x0002_0111_0108_a0f0, 0x0000_4004_0002_0102, 0, 0, 0x0018_0008, 0, 0, 0>(
        output, a, b, c, selector, 32, 33, 34, 35, 36,
    );
}

#[cfg(feature = "complete-body-wrong-launch-v19")]
// Valid general typed launch: isolate the complete-body exact64 profile check.
#[kernel(typed, launch(required = [128, 1, 1], max = [128, 1, 1], max_grid = [2, 1, 1]))]
pub fn wrong_launch(output: DisjointSlice<u32>, a: u32, b: u32, c: u32, selector: u32) {
    body::<1, 1, 0x41ff, 0, 0, 0, 0x0008, 0, 0, 0>(output, a, b, c, selector, 32, 33, 34, 35, 36);
}

#[cfg(feature = "complete-body-reserved-register-v19")]
#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1], max_grid = [2, 1, 1]))]
pub fn reserved_register(output: DisjointSlice<u32>, a: u32, b: u32, c: u32, selector: u32) {
    // v7 belongs to the compiler-owned boundary.
    body::<1, 1, 0x41ff, 0, 0, 0, 0x0008, 0, 0, 0>(output, a, b, c, selector, 7, 33, 34, 35, 36);
}

#[cfg(feature = "complete-body-foreign-input-v19")]
#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1], max_grid = [2, 1, 1]))]
pub fn foreign_input(output: DisjointSlice<u32>, a: u32, b: u32, c: u32, selector: u32) {
    // No helper call or arithmetic: substitute another root argument in a's
    // slot to isolate the exact source-argument transport refusal.
    let _ = a;
    body::<1, 1, 0x41ff, 0, 0, 0, 0x0008, 0, 0, 0>(output, b, b, c, selector, 32, 33, 34, 35, 36);
}

#[cfg(feature = "complete-body-undefined-merge-v19")]
#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1], max_grid = [2, 1, 1]))]
pub fn undefined_merge(output: DisjointSlice<u32>, a: u32, b: u32, c: u32, selector: u32) {
    // Only the zero arm defines out. Label 4's terminal must be refused.
    body::<4, 1, 0x0002_0111_0108_a0f0, 0x0000_4004_0002_0002, 0, 0, 0x0008, 0, 0, 0>(
        output, a, b, c, selector, 32, 33, 34, 35, 36,
    );
}

#[cfg(feature = "complete-body-dynamic-grid-v19")]
#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
pub fn dynamic_grid(output: DisjointSlice<u32>, a: u32, b: u32, c: u32, selector: u32) {
    // No finite max_grid: the exact source envelope stays dynamic and refuses.
    body::<1, 1, 0x41ff, 0, 0, 0, 0x0008, 0, 0, 0>(output, a, b, c, selector, 32, 33, 34, 35, 36);
}
