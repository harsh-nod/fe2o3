use fe2o3_device::{WriteOnlyDisjointSlice, kernel, thread};

#[cfg(feature = "reference-write-only-positive")]
fn reference(_point: usize, output: &mut u32) {
    *output = 17;
}

#[cfg(feature = "reference-write-only-coordinate-read")]
fn reference(_point: usize, output: &mut u32) {
    *output += 1;
}

#[kernel(
    typed,
    reference = reference,
    launch(required = [64, 1, 1], max = [64, 1, 1])
)]
pub fn fill(mut output: WriteOnlyDisjointSlice<u32>) {
    let _ = output.write(thread::index_1d(), 17);
}
