use gpu_device::{DisjointSlice, kernel};

#[kernel(
    typed,
    namespace = "9c0e8b256bc76d2d17529f43ca8e2ee3480c40dfd019491bd4fb1fc22c4f5f2d"
)]
pub fn zeta(scale: f32, input: &[f32], output: DisjointSlice<f32>) {
    let _ = (scale, input, output);
}

fn require_adapter<'allocation, T>()
where
    T: gpu_host::__generated::CompilerGeneratedAlphaZetaCov6ArgumentsV1<
            'allocation,
            zeta_gpu::Marker,
        >,
{
}

fn main() {
    require_adapter::<zeta_gpu::Arguments<'static>>();
}
