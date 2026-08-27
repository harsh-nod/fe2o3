use fe2o3_device::{DisjointSlice, kernel, thread};

#[kernel]
pub fn shift(x: &[f32], mut out: DisjointSlice<f32>) {
    let idx = thread::index_1d();
    let source = idx.offset(1);
    let Some(value) = out.get_mut(idx) else {
        return;
    };
    if source >= x.len() {
        fe2o3_device::trap();
    }
    *value = x[source];
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "the production Worker V3 application verifier is not wired for fe2o3-shift",
    )
    .into())
}
