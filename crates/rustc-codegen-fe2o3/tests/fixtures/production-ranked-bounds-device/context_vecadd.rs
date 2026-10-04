use fe2o3_device::{DisjointSlice, KernelContext, kernel, thread};

// Keep the manifest's indexing, guarded output and f32 arithmetic body unchanged.
#[cfg(feature = "provider_context_protocol")]
#[macro_use]
mod context_body {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../../../examples/vecadd/src/vecadd_body.rs"
    ));
}

macro_rules! production_f32_add {
    ($lhs:expr, $rhs:expr) => {{ $lhs + $rhs }};
}

// Diagnostic consumer: _ctx covers issuance/ABI only; indexing stays on thread::index_1d.
#[kernel(typed, launch(required = [256, 1, 1], max = [256, 1, 1]))]
pub fn vecadd(_ctx: KernelContext<'_>, a: &[f32], b: &[f32], mut c: DisjointSlice<f32>) {
    vecadd_kernel_body!(thread, (), production_f32_add, a, b, c);
}
