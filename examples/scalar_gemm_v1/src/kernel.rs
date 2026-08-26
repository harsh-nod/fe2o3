use fe2o3_device::{DisjointSlice, kernel, thread};

// The compiler qualification profile authenticates this expanded recurrence syntax.
#[allow(clippy::assign_op_pattern)]
#[kernel(
    typed,
    namespace = "53bf3c83481a081d4ab0e2b32039f9c89be5de3937a84aca0c40800c8d6b0413",
    control_flow(loop_bounds(4294967295))
)]
pub fn scalar_gemm_v1(a: &[f32], b: &[f32], mut c: DisjointSlice<f32>, m: u32, n: u32, k: u32) {
    let index = thread::index_1d();
    let p = index.get();
    let output_extent = (m as usize) * (n as usize);
    if p < output_extent {
        let row = p / (n as usize);
        let col = p % (n as usize);
        let mut accumulator = 0.0_f32;
        let mut t = 0_u32;
        while t < k {
            let a_index = row * (k as usize) + (t as usize);
            let b_index = (t as usize) * (n as usize) + col;
            let product = a[a_index] * b[b_index];
            accumulator = accumulator + product;
            t = t + 1;
        }
        if let Some(output) = c.get_mut(index) {
            *output = accumulator;
        }
    }
}
