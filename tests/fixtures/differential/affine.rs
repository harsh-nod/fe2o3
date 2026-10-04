use fe2o3_device::{DisjointSlice, kernel, thread};

#[kernel(
    typed,
    namespace = "98a268e9215ed046e24166228506ede6fd074c2ac7f8734a27b4a716b93e7aec"
)]
pub fn differential_affine(
    alpha: f32,
    bias: f32,
    input: &[f32],
    mut output: DisjointSlice<f32>,
) {
    let index = thread::index_1d();
    let offset = index.get();
    if offset < input.len()
        && let Some(value) = output.get_mut(index)
    {
        *value = alpha * input[offset] + bias;
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "the production Worker V3 differential application verifier is not wired",
    )
    .into())
}
