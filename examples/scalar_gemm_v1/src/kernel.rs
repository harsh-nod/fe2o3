use fe2o3_device::{DisjointSlice, kernel, thread};

// The compiler qualification profile authenticates this explicit recurrence and division guard.
#[allow(
    clippy::assign_op_pattern,
    clippy::collapsible_if,
    clippy::manual_checked_ops
)]
#[kernel(
    typed,
    launch(required = [256, 1, 1], max = [256, 1, 1]),
    control_flow(loop_bounds(4294967295))
)]
pub fn scalar_gemm_v1(a: &[f32], b: &[f32], mut c: DisjointSlice<f32>, m: u32, n: u32, k: u32) {
    let index = thread::index_1d();
    let p = index.get();
    let n_index = n as usize;
    let output_extent = (m as usize) * n_index;
    if n_index != 0 {
        if p < output_extent {
            let row = (p / n_index) as u32;
            let col = (p % n_index) as u32;
            let mut accumulator = 0.0_f32;
            let mut t = 0_u32;
            while t < k {
                let a_index = (row as usize) * (k as usize) + (t as usize);
                let b_index = (t as usize) * n_index + (col as usize);
                let product = a[a_index] * b[b_index];
                accumulator = accumulator + product;
                t = t + 1;
            }
            if let Some(output) = c.get_mut(index) {
                *output = accumulator;
            }
        }
    }
}
