use gpu_device::kernel;

#[kernel(
    typed,
    namespace = "53bf3c83481a081d4ab0e2b32039f9c89be5de3937a84aca0c40800c8d6b0413",
    launch(required = [256, 1, 1], max = [256, 1, 1])
)]
pub fn scalar_gemm_v1(
    a: &[f32],
    b: &[f32],
    c: &[f32],
    m: u32,
    n: u32,
    k: u32,
) {
    let _ = (a, b, c, m, n, k);
}

fn require_adapter<T>()
where
    T: gpu_host::__generated::CompilerGeneratedScalarGemmV1Arguments<
        'static,
        scalar_gemm_v1_gpu::Marker,
    >,
{
}

fn main() {
    require_adapter::<scalar_gemm_v1_gpu::Arguments<'static>>();
}
