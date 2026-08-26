use fe2o3_device::{DisjointSlice, kernel, thread};

#[kernel]
pub fn vecadd_f64(a: &[f64], b: &[f64], mut c: DisjointSlice<f64>) {
    let idx = thread::index_1d();
    let i = idx.get();
    let Some(value) = c.get_mut(idx) else {
        return;
    };
    if i >= a.len() || i >= b.len() {
        fe2o3_device::trap();
        return;
    }
    *value = a[i] + b[i];
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "the production Worker V3 application verifier is not wired for fe2o3-vecadd_f64",
    )
    .into())
}
