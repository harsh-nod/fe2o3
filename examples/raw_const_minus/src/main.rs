use fe2o3_device::{DisjointSlice, kernel, thread};

const N: usize = 1024;
const LAST: usize = N - 1;

#[kernel]
pub fn raw_const_minus(x: &[f32], mut out: DisjointSlice<f32>) {
    let idx = thread::index_1d();
    let base = idx.get();
    let Some(value) = out.get_mut(idx) else {
        return;
    };
    if base > LAST {
        fe2o3_device::trap();
        return;
    }
    let source = LAST - base;
    if source >= x.len() {
        fe2o3_device::trap();
        return;
    }
    *value = x[source];
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "the production Worker V3 application verifier is not wired for fe2o3-raw_const_minus",
    )
    .into())
}
