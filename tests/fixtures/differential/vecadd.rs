use fe2o3_device::{DisjointSlice, kernel, thread};

#[kernel(
    typed,
    namespace = "d9cd7befa27a5224f889fb794b18408bc0f38ad150dca01d2d173479f2945908"
)]
pub fn differential_vecadd(a: &[f32], b: &[f32], mut output: DisjointSlice<f32>) {
    let index = thread::index_1d();
    let offset = index.get();
    if offset < a.len()
        && offset < b.len()
        && let Some(value) = output.get_mut(index)
    {
        *value = a[offset] + b[offset];
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "the production Worker V3 differential application verifier is not wired",
    )
    .into())
}
