use fe2o3_device::{DisjointSlice, kernel, thread};

#[kernel]
pub fn stencil(x: &[f32], mut out: DisjointSlice<f32>) {
    let idx = thread::index_1d();
    let center = idx.get();
    let right = idx.offset(1);
    let Some(value) = out.get_mut(idx) else {
        return;
    };
    if center == 0 || right >= x.len() {
        *value = 0.0;
        return;
    }
    let left = center - 1;
    *value = 0.25 * x[left] + 0.5 * x[center] + 0.25 * x[right];
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "the production Worker V3 application verifier is not wired for fe2o3-stencil",
    )
    .into())
}
