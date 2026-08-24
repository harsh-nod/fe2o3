use fe2o3_device::{DisjointSlice, kernel, thread};

#[kernel(
    typed,
    namespace = "a7a1a891225e50ca13e48ef7fba98a25a5eaa25211d2247a5ed536c35bc06a54",
    launch(required = [64, 1, 1], max = [64, 1, 1])
)]
pub fn fill(mut output: DisjointSlice<u32>) {
    let index = thread::index_1d();
    if let Some(element) = output.get_mut(index) {
        *element = 17;
    }
}
