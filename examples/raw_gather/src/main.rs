use fe2o3_device::{DisjointSlice, kernel, thread};

#[kernel]
pub fn raw_gather(x: &[f32], mut out: DisjointSlice<f32>) {
    let idx = thread::index_1d();
    let raw_idx = idx.get();
    let Some(value) = out.get_mut(idx) else {
        return;
    };
    let source = raw_idx * 2 + 1;
    if source >= x.len() {
        fe2o3_device::trap();
        return;
    }
    *value = x[source];
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "the production Worker V3 application verifier is not wired for fe2o3-raw_gather",
    )
    .into())
}
