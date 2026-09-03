#![no_std]

#[cfg(feature = "atomic-rmw")]
use fe2o3_device::DeviceGlobalMutPtr;
#[cfg(any(feature = "write-only-output", feature = "write-only-disjoint-output"))]
use fe2o3_device::WriteOnlyDisjointSlice;
#[cfg(feature = "atomic-rmw")]
use fe2o3_device::atomic::Ordering;
use fe2o3_device::{DisjointSlice, kernel, thread};

#[cfg(not(any(
    feature = "atomic-rmw",
    feature = "multi-root-ownership",
    feature = "multi-root-target-lineage",
    feature = "three-root-ownership",
    feature = "write-only-output",
    feature = "write-only-disjoint-output",
    feature = "reference-positive",
    feature = "reference-mutated",
    feature = "reference-unsafe",
    feature = "reference-abi-mismatch",
    feature = "reference-loop",
    feature = "reference-call",
    feature = "reference-dynamic-loop",
    feature = "reference-nested-call",
    feature = "reference-slice-read",
    feature = "reference-helper-memory",
    feature = "reference-helper-unsafe",
    feature = "reference-helper-recursive",
    feature = "reference-loop-overflow",
    feature = "reference-non-function",
    feature = "reference-generic-mismatch",
    feature = "reference-missing",
    feature = "reference-no-output",
    feature = "reference-duplicate",
    feature = "reference-orphan",
    feature = "reference-two-output-positive",
    feature = "reference-two-output-substitution",
    feature = "reference-two-output-alias",
    feature = "reference-two-output-schedule",
    feature = "scalar-transmute",
    feature = "fabs-f32",
)))]
#[kernel(typed)]
pub fn fill(mut output: DisjointSlice<u32>) {
    let index = thread::index_1d();
    if let Some(element) = output.get_mut(index) {
        *element = 17;
    }
}

#[cfg(feature = "scalar-transmute")]
#[kernel(typed)]
pub fn scalar_transmute(bits: u32, mut output: DisjointSlice<f32>) {
    let index = thread::index_1d();
    if let Some(element) = output.get_mut(index) {
        *element = f32::from_bits(bits);
    }
}

#[cfg(feature = "fabs-f32")]
#[kernel(typed)]
pub fn fabs_f32(value: f32, mut output: DisjointSlice<f32>) {
    let index = thread::index_1d();
    if let Some(element) = output.get_mut(index) {
        *element = value.abs();
    }
}

#[cfg(feature = "atomic-rmw")]
#[kernel(
    typed,
    launch(required = [64, 1, 1], max = [64, 1, 1])
)]
pub fn core_atomic_rmw_v1(unsigned: DeviceGlobalMutPtr<u32>, signed: DeviceGlobalMutPtr<i32>) {
    let unsigned = unsigned.as_atomic();
    let _ = unsigned.swap(1, Ordering::SeqCst);
    let _ = unsigned.fetch_add(2, Ordering::Relaxed);
    let _ = unsigned.fetch_sub(3, Ordering::Acquire);
    let _ = unsigned.fetch_and(4, Ordering::Release);
    let _ = unsigned.fetch_or(5, Ordering::AcqRel);
    let _ = unsigned.fetch_xor(6, Ordering::SeqCst);
    let _ = unsigned.fetch_min(7, Ordering::Relaxed);
    let _ = unsigned.fetch_max(8, Ordering::Acquire);

    let signed = signed.as_atomic();
    let _ = signed.fetch_min(-9, Ordering::Release);
    let _ = signed.fetch_max(10, Ordering::AcqRel);
}

#[cfg(feature = "write-only-output")]
#[kernel(
    typed,
    launch(required = [64, 1, 1], max = [64, 1, 1])
)]
pub fn fill_write_only(mut output: WriteOnlyDisjointSlice<u32>) {
    let index = thread::index_1d();
    let value = index.get() as u32;
    let _ = output.write(index, value);
}

#[cfg(feature = "write-only-disjoint-output")]
#[kernel(
    typed,
    launch(required = [64, 1, 1], max = [64, 1, 1])
)]
pub fn fill_write_only_disjoint(mut output: WriteOnlyDisjointSlice<u32>) {
    let index = thread::index_1d();
    let value = index.get() as u32;
    let index = index.into_disjoint();
    let _ = output.write_disjoint(index, value);
}

#[cfg(any(feature = "multi-root-ownership", feature = "three-root-ownership"))]
#[kernel(
    typed,
    launch(required = [64, 1, 1], max = [64, 1, 1])
)]
pub fn alpha(mut output: DisjointSlice<u32>) {
    let index = thread::index_1d();
    if let Some(element) = output.get_mut(index) {
        *element = 17;
    }
}

#[cfg(any(feature = "multi-root-ownership", feature = "three-root-ownership"))]
#[kernel(
    typed,
    launch(required = [64, 1, 1], max = [64, 1, 1])
)]
pub fn zeta(mut output: DisjointSlice<u32>) {
    let index = thread::index_1d();
    if let Some(element) = output.get_mut(index) {
        *element = 23;
    }
}

#[cfg(feature = "three-root-ownership")]
#[kernel(
    typed,
    launch(required = [64, 1, 1], max = [64, 1, 1])
)]
pub fn omega(mut output: DisjointSlice<u32>) {
    let index = thread::index_1d();
    if let Some(element) = output.get_mut(index) {
        *element = 29;
    }
}

#[cfg(feature = "multi-root-target-lineage")]
fn alpha_reference(_point: usize, output: &mut u32) {
    *output = 17;
}

#[cfg(feature = "multi-root-target-lineage")]
fn zeta_reference(_point: usize, output: &mut u32) {
    *output = 23;
}

#[cfg(feature = "multi-root-target-lineage")]
#[kernel(
    typed,
    reference = alpha_reference,
    launch(required = [64, 1, 1], max = [64, 1, 1])
)]
pub fn alpha(mut output: DisjointSlice<u32>) {
    let index = thread::index_1d();
    if let Some(element) = output.get_mut(index) {
        *element = 17;
    }
}

#[cfg(feature = "multi-root-target-lineage")]
#[kernel(
    typed,
    reference = zeta_reference,
    launch(required = [64, 1, 1], max = [64, 1, 1])
)]
pub fn zeta(mut output: DisjointSlice<u32>) {
    let index = thread::index_1d();
    if let Some(element) = output.get_mut(index) {
        *element = 23;
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
fn cpu_reference(_point: usize, output: &mut u32) {
    let mut value = 0_u32;
    let mut result = 11_u32;
    while value < 4 {
        result += value;
        value += 1;
    }
    *output = result;
}

#[cfg(feature = "reference-call")]
fn cpu_reference(_point: usize, output: &mut u32) {
    *output = reference_helper(16);
}

#[cfg(feature = "reference-call")]
fn reference_helper(value: u32) -> u32 {
    value + 1
}

#[cfg(feature = "reference-dynamic-loop")]
fn cpu_reference(_point: usize, limit: u32, output: &mut u32) {
    let mut value = 0_u32;
    while value < limit {
        value += 1;
    }
    *output = value;
}

#[cfg(feature = "reference-nested-call")]
fn cpu_reference(_point: usize, output: &mut u32) {
    *output = outer_helper(16);
}

#[cfg(feature = "reference-nested-call")]
fn outer_helper(value: u32) -> u32 {
    inner_helper(value)
}

#[cfg(feature = "reference-nested-call")]
fn inner_helper(value: u32) -> u32 {
    value + 1
}

#[cfg(feature = "reference-slice-read")]
fn cpu_reference(point: usize, input: &[u32], output: &mut u32) {
    *output = input[point];
}

#[cfg(any(
    feature = "reference-two-output-positive",
    feature = "reference-two-output-substitution",
    feature = "reference-two-output-alias",
    feature = "reference-two-output-schedule",
))]
fn cpu_reference(_point: usize, first: &mut u32, second: &mut u32) {
    *first = 17;
    *second = 23;
}

#[cfg(feature = "reference-helper-memory")]
fn cpu_reference(_point: usize, output: &mut u32) {
    let input = 17_u32;
    *output = memory_helper(&input);
}

#[cfg(feature = "reference-helper-memory")]
fn memory_helper(input: &u32) -> u32 {
    *input
}

#[cfg(feature = "reference-helper-unsafe")]
fn cpu_reference(_point: usize, output: &mut u32) {
    // The reference authenticator must reject this source before summarizing.
    *output = unsafe { unsafe_helper(17) };
}

#[cfg(feature = "reference-helper-unsafe")]
unsafe fn unsafe_helper(value: u32) -> u32 {
    value
}

#[cfg(feature = "reference-helper-recursive")]
fn cpu_reference(_point: usize, output: &mut u32) {
    *output = recursive_helper(17);
}

#[cfg(feature = "reference-helper-recursive")]
#[allow(unconditional_recursion)]
fn recursive_helper(value: u32) -> u32 {
    recursive_helper(value)
}

#[cfg(feature = "reference-loop-overflow")]
fn cpu_reference(_point: usize, output: &mut u32) {
    let mut iteration = 0_u32;
    let mut result = u32::MAX;
    while iteration < 1 {
        result += 1;
        iteration += 1;
    }
    *output = result;
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
    feature = "reference-nested-call",
    feature = "reference-helper-memory",
    feature = "reference-helper-unsafe",
    feature = "reference-helper-recursive",
    feature = "reference-loop-overflow",
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

#[cfg(feature = "reference-dynamic-loop")]
#[kernel(
    typed,
    reference = cpu_reference,
    launch(required = [64, 1, 1], max = [64, 1, 1])
)]
pub fn fill(limit: u32, mut output: DisjointSlice<u32>) {
    let index = thread::index_1d();
    if let Some(element) = output.get_mut(index) {
        *element = limit;
    }
}

#[cfg(feature = "reference-slice-read")]
#[kernel(
    typed,
    reference = cpu_reference,
    launch(required = [64, 1, 1], max = [64, 1, 1])
)]
pub fn fill(input: &[u32], mut output: DisjointSlice<u32>) {
    let index = thread::index_1d();
    let offset = index.get();
    if let Some(element) = output.get_mut(index) {
        *element = input[offset];
    }
}

#[cfg(feature = "reference-two-output-positive")]
#[kernel(
    typed,
    reference = cpu_reference,
    launch(required = [64, 1, 1], max = [64, 1, 1])
)]
pub fn fill(mut first: DisjointSlice<u32>, mut second: DisjointSlice<u32>) {
    if let Some(element) = first.get_mut(thread::index_1d()) {
        *element = 17;
    }
    if let Some(element) = second.get_mut(thread::index_1d()) {
        *element = 23;
    }
}

#[cfg(feature = "reference-two-output-substitution")]
#[kernel(
    typed,
    reference = cpu_reference,
    launch(required = [64, 1, 1], max = [64, 1, 1])
)]
pub fn fill(mut first: DisjointSlice<u32>, mut second: DisjointSlice<u32>) {
    if let Some(element) = first.get_mut(thread::index_1d()) {
        *element = 23;
    }
    if let Some(element) = second.get_mut(thread::index_1d()) {
        *element = 17;
    }
}

#[cfg(feature = "reference-two-output-alias")]
#[kernel(
    typed,
    reference = cpu_reference,
    launch(required = [64, 1, 1], max = [64, 1, 1])
)]
pub fn fill(mut first: DisjointSlice<u32>, _second: DisjointSlice<u32>) {
    if let Some(element) = first.get_mut(thread::index_1d()) {
        *element = 17;
    }
    if let Some(element) = first.get_mut(thread::index_1d()) {
        *element = 23;
    }
}

#[cfg(feature = "reference-two-output-schedule")]
#[kernel(
    typed,
    reference = cpu_reference,
    launch(required = [64, 1, 1], max = [64, 1, 1])
)]
pub fn fill(mut first: DisjointSlice<u32>, mut second: DisjointSlice<u32>) {
    if let Some(element) = first.get_mut(thread::index_1d()) {
        *element = 17;
    }
    if thread::index_1d().get() % 2 == 0
        && let Some(element) = second.get_mut(thread::index_1d())
    {
        *element = 23;
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
