use core::marker::PhantomData;

use gpu_device::{DisjointSlice, kernel};

struct KernelContext<'kernel>(PhantomData<&'kernel mut &'kernel ()>);

#[kernel(
    typed,
    namespace = "7c0e8b256bc76d2d17529f43ca8e2ee3480c40dfd019491bd4fb1fc22c4f5f2d"
)]
pub fn context_lookalike(
    context: KernelContext<'_>,
    a: &[f32],
    b: &[f32],
    mut c: DisjointSlice<f32>,
) {
    let _ = (context, a, b, &mut c);
}

fn main() {}
