#![no_std]

use fe2o3_device::{DisjointSlice, kernel, thread};

#[cfg(not(any(
    feature = "reference-positive",
    feature = "reference-mutated",
    feature = "reference-unsafe",
    feature = "reference-abi-mismatch",
    feature = "reference-loop",
    feature = "reference-call",
    feature = "reference-non-function",
    feature = "reference-generic-mismatch",
    feature = "reference-missing",
    feature = "reference-no-output",
    feature = "reference-duplicate",
    feature = "reference-orphan",
)))]
#[kernel(typed)]
pub fn fill(mut output: DisjointSlice<u32>) {
    let index = thread::index_1d();
    if let Some(element) = output.get_mut(index) {
        *element = 17;
    }
}

#[cfg(any(feature = "reference-positive", feature = "reference-duplicate"))]
fn cpu_reference(_point: usize, output: &mut u32) {
    *output = 17;
}

#[cfg(feature = "reference-mutated")]
fn cpu_reference(_point: usize, output: &mut u32) {
    *output = 18;
}

#[cfg(feature = "reference-unsafe")]
unsafe fn cpu_reference(_point: usize, output: &mut u32) {
    *output = 17;
}

#[cfg(feature = "reference-no-output")]
fn cpu_reference(_point: usize, _output: &mut u32) {}

#[cfg(feature = "reference-abi-mismatch")]
fn cpu_reference(output: &mut f32) {
    *output = 17.0;
}

#[cfg(feature = "reference-loop")]
fn cpu_reference(output: &mut u32) {
    for value in 0..4 {
        *output += value;
    }
}

#[cfg(feature = "reference-call")]
fn cpu_reference(output: &mut u32) {
    reference_helper(output);
}

#[cfg(feature = "reference-call")]
fn reference_helper(output: &mut u32) {
    *output = 17;
}

#[cfg(feature = "reference-non-function")]
const CPU_REFERENCE: u32 = 17;

#[cfg(feature = "reference-generic-mismatch")]
fn cpu_reference<T>(output: &mut T) {
    let _ = output;
}

#[cfg(any(
    feature = "reference-positive",
    feature = "reference-mutated",
    feature = "reference-unsafe",
    feature = "reference-abi-mismatch",
    feature = "reference-loop",
    feature = "reference-call",
    feature = "reference-generic-mismatch",
    feature = "reference-no-output",
    feature = "reference-duplicate",
))]
#[kernel(
    typed,
    reference = cpu_reference,
    launch(required = [64, 1, 1], max = [64, 1, 1])
)]
pub fn fill(mut output: DisjointSlice<u32>) {
    let index = thread::index_1d();
    if let Some(element) = output.get_mut(index) {
        *element = 17;
    }
}

#[cfg(feature = "reference-orphan")]
#[kernel(typed)]
pub fn fill(mut output: DisjointSlice<u32>) {
    let index = thread::index_1d();
    if let Some(element) = output.get_mut(index) {
        *element = 17;
    }
}

#[cfg(feature = "reference-duplicate")]
mod duplicate_binding {
    use super::{DisjointSlice, cpu_reference};

    fn detached_kernel(_: DisjointSlice<u32>) {}

    fn __fe2o3_kernel_reference_anchor_v1_fill() {
        let _ = core::hint::black_box(cpu_reference);
    }

    #[allow(non_upper_case_globals)]
    #[used]
    static __fe2o3_kernel_reference_binding_v1_fill: (
        u64,
        u16,
        u16,
        &'static str,
        fn(DisjointSlice<u32>),
        fn(),
    ) = (
        5063543736373495110,
        1,
        1,
        "fill",
        detached_kernel,
        __fe2o3_kernel_reference_anchor_v1_fill,
    );
}

#[cfg(feature = "reference-orphan")]
mod orphan_binding {
    use super::DisjointSlice;

    fn detached_kernel(_: DisjointSlice<u32>) {}

    fn cpu_reference(output: &mut u32) {
        *output = 17;
    }

    fn __fe2o3_kernel_reference_anchor_v1_orphan() {
        let _ = core::hint::black_box(cpu_reference);
    }

    #[allow(non_upper_case_globals)]
    #[used]
    static __fe2o3_kernel_reference_binding_v1_orphan: (
        u64,
        u16,
        u16,
        &'static str,
        fn(DisjointSlice<u32>),
        fn(),
    ) = (
        5063543736373495110,
        1,
        1,
        "orphan",
        detached_kernel,
        __fe2o3_kernel_reference_anchor_v1_orphan,
    );
}

#[cfg(feature = "reference-non-function")]
#[kernel(
    typed,
    reference = CPU_REFERENCE
)]
pub fn fill(mut output: DisjointSlice<u32>) {
    let index = thread::index_1d();
    if let Some(element) = output.get_mut(index) {
        *element = 17;
    }
}

#[cfg(feature = "reference-missing")]
#[kernel(
    typed,
    reference = missing_cpu_reference
)]
pub fn fill(mut output: DisjointSlice<u32>) {
    let index = thread::index_1d();
    if let Some(element) = output.get_mut(index) {
        *element = 17;
    }
}
