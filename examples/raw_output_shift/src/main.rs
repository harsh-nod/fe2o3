use fe2o3_device::{DisjointSlice, Index1D, Shifted, kernel, thread};

#[kernel]
pub fn raw_output_shift(x: &[f32], mut out: DisjointSlice<f32, Shifted<Index1D, 1>>) {
    let idx = thread::index_1d();
    let source = idx.get();
    if source >= x.len() {
        return;
    }
    let Some(target) = idx.checked_shift::<1>() else {
        fe2o3_device::trap();
        return;
    };
    let Some(value) = out.get_disjoint_mut(target) else {
        fe2o3_device::trap();
        return;
    };
    *value = x[source] * 2.0;
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "the production Worker V3 application verifier is not wired for fe2o3-raw_output_shift",
    )
    .into())
}
