use fe2o3_device::{DisjointSlice, kernel, thread};

#[kernel]
pub fn previous(x: &[f32], mut out: DisjointSlice<f32>) {
    let idx = thread::index_1d();
    let i = idx.get();
    let Some(value) = out.get_mut(idx) else {
        return;
    };
    if i == 0 {
        *value = 0.0;
        return;
    }
    let source = i - 1;
    if source >= x.len() {
        fe2o3_device::trap();
        return;
    }
    *value = x[source];
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "the production Worker V3 application verifier is not wired for fe2o3-previous",
    )
    .into())
}
