use fe2o3_device::{DisjointSlice, kernel, thread};

#[kernel]
pub fn axpy_inplace(alpha: f32, x: &[f32], mut y: DisjointSlice<f32>) {
    let idx = thread::index_1d();
    let i = idx.get();
    let Some(y_value) = y.get_mut(idx) else {
        return;
    };
    if i >= x.len() {
        fe2o3_device::trap();
    }
    *y_value = alpha * x[i] + *y_value;
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "the production Worker V3 application verifier is not wired for fe2o3-axpy_inplace",
    )
    .into())
}
