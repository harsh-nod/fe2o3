use fe2o3_device::{DisjointSlice, kernel, thread};

#[kernel]
pub fn scale_stage(alpha: f32, x: &[f32], mut tmp: DisjointSlice<f32>) {
    let idx = thread::index_1d();
    let i = idx.get();
    let Some(value) = tmp.get_mut(idx) else {
        return;
    };
    if i >= x.len() {
        fe2o3_device::trap();
    }
    *value = alpha * x[i];
}

#[kernel]
pub fn bias_stage(tmp: &[f32], beta: f32, mut out: DisjointSlice<f32>) {
    let idx = thread::index_1d();
    let i = idx.get();
    let Some(value) = out.get_mut(idx) else {
        return;
    };
    if i >= tmp.len() {
        fe2o3_device::trap();
    }
    *value = tmp[i] + beta;
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "the production Worker V3 application verifier is not wired for fe2o3-pipeline",
    )
    .into())
}
