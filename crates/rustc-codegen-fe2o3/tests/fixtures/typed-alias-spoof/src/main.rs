#![allow(non_camel_case_types)]

#[cfg(not(feature = "general-genuine"))]
use fe2o3_device::kernel;
#[cfg(not(any(feature = "general-genuine", feature = "general-lookalike")))]
use fe2o3_device::{DisjointSlice, thread};

// The macro sees the token `f32`, but rustc normalizes this alias to `f64`.
// The semantic layout extractor must reject it before artifact generation.
#[cfg(not(feature = "general-genuine"))]
type f32 = f64;

#[cfg(not(any(feature = "general-genuine", feature = "general-lookalike")))]
#[kernel(
    typed,
    namespace = "1b3f523833ae188d124b710c48983f3911bd8be1ea82408ffc78121e843e6271"
)]
pub fn typed_alias_spoof(a: &[f32], b: &[f32], mut output: DisjointSlice<f32>) {
    let index = thread::index_1d();
    let i = index.get();
    if let Some(value) = output.get_mut(index) {
        *value = a[i] + b[i];
    }
}

#[cfg(feature = "general-lookalike")]
mod general_lookalike {
    use super::kernel;
    use core::marker::PhantomData;

    pub enum Index1D {}

    #[repr(C)]
    pub struct DisjointSlice<T, IndexSpace = Index1D> {
        pointer: *mut T,
        length: usize,
        index_space: PhantomData<fn() -> IndexSpace>,
    }

    #[kernel(
        typed,
        namespace = "1b3f523833ae188d124b710c48983f3911bd8be1ea82408ffc78121e843e6271"
    )]
    pub fn general_type_spoof(scale: f32, input: &[f32], _output: DisjointSlice<f32, Index1D>) {
        let _ = (scale, input);
    }
}

#[cfg(feature = "general-genuine")]
mod general_genuine {
    use fe2o3_device::{DisjointSlice, device_export, kernel, thread};

    #[device_export(
        symbol = "general_v3_identity_v1",
        target = "gfx942:xnack-",
        code_object = 6,
        effects = "none",
        semantic = "5656565656565656565656565656565656565656565656565656565656565656"
    )]
    pub unsafe extern "C" fn general_v3_identity(value: u32) -> u32 {
        value
    }

    #[kernel(
        typed,
        namespace = "1b3f523833ae188d124b710c48983f3911bd8be1ea82408ffc78121e843e6271"
    )]
    pub fn alpha(scale: f32, input: &[f32], mut output: DisjointSlice<f32>) {
        let index = thread::index_1d();
        let i = index.get();
        if let Some(value) = output.get_mut(index) {
            *value = input[i] * scale;
        }
    }

    #[kernel(
        typed,
        namespace = "1b3f523833ae188d124b710c48983f3911bd8be1ea82408ffc78121e843e6271"
    )]
    pub fn zeta(a: &[f32], b: &[f32], bias: f32, mut output: DisjointSlice<f32>) {
        let index = thread::index_1d();
        let i = index.get();
        if let Some(value) = output.get_mut(index) {
            *value = a[i] + b[i] + bias;
        }
    }

    pub fn validate_backend_witness() {
        fe2o3_host::__generated::validate_compiler_generated_semantic_witness_v1::<
            alpha_gpu::Marker,
        >()
        .expect("backend-issued alpha semantic witness");
        fe2o3_host::__generated::validate_compiler_generated_semantic_witness_v1::<
            zeta_gpu::Marker,
        >()
        .expect("backend-issued zeta semantic witness");
    }
}

#[cfg(feature = "general-genuine")]
fn main() {
    general_genuine::validate_backend_witness();
}

#[cfg(not(feature = "general-genuine"))]
fn main() {}
