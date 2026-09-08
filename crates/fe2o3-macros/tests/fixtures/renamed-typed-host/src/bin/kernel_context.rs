use gpu_device::{DisjointSlice, KernelContext, kernel};

#[kernel(
    typed,
    namespace = "7c0e8b256bc76d2d17529f43ca8e2ee3480c40dfd019491bd4fb1fc22c4f5f2d"
)]
pub fn context_vecadd(
    context: KernelContext<'_>,
    a: &[f32],
    b: &[f32],
    mut c: DisjointSlice<f32>,
) {
    let _ = (context.invocation(), a, b, &mut c);
}

fn main() {
    assert_eq!(
        <context_vecadd_gpu::Marker as gpu_device::KernelMarkerV1>::REGISTRATION.2,
        4,
    );
}
