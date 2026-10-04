use fe2o3_device::{DisjointSlice, kernel, thread};

#[kernel(
    typed,
    namespace = "28762ac0d47345e92fd1a2650f1406792fb0dee73da58c4e5ab8830ba33760f1"
)]
pub fn differential_fill(bounds: &[f32], mut output: DisjointSlice<f32>) {
    let index = thread::index_1d();
    if index.get() < bounds.len()
        && let Some(value) = output.get_mut(index)
    {
        *value = 42.5;
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "the production Worker V3 differential application verifier is not wired",
    )
    .into())
}
