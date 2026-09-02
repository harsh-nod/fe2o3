use gpu_device::{DisjointSlice, kernel};
use gpu_host::__generated::{GeneratedKfdReadSlice, GeneratedKfdReadWriteSlice};

#[kernel(
    typed,
    namespace = "7c0e8b256bc76d2d17529f43ca8e2ee3480c40dfd019491bd4fb1fc22c4f5f2d"
)]
pub fn retained(scale: f32, input: &[f32], output: DisjointSlice<f32>) {
    let _ = (scale, input, output);
}

fn escape<'short>(
    input: &'short [f32],
    output: &'short mut [f32],
) -> retained_gpu::Arguments<'static> {
    let input = GeneratedKfdReadSlice::new(input);
    let output = GeneratedKfdReadWriteSlice::new(output);
    retained_gpu::Arguments::new(1.0, input, output)
}

fn main() {
    let _ = escape;
}
