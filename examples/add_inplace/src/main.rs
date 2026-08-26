use fe2o3_device::{DisjointSlice, kernel, thread};

#[kernel]
pub fn add_inplace(delta: f32, mut values: DisjointSlice<f32>) {
    let idx = thread::index_1d();
    let Some(value) = values.get_mut(idx) else {
        return;
    };
    *value += delta;
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "the production Worker V3 application verifier is not wired for fe2o3-add_inplace",
    )
    .into())
}
