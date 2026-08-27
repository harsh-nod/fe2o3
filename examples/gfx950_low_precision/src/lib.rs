#![forbid(unsafe_code)]
#![cfg_attr(target_arch = "amdgpu", no_std)]

//! Rust-first gfx950 low-precision source examples.
//!
//! The attributed functions in [`kernel`] are the fe2o3 kernel source. The
//! neighboring HIP file is a compiler/ISA/hardware fixture, not the source of
//! these Rust kernels.

pub mod kernel;
#[cfg(not(target_arch = "amdgpu"))]
pub mod reference;

/// The ordinary Rust kernel source exists and is checked by host compilation.
pub const GFX950_RUST_KERNEL_SOURCE_PRESENT_V1: bool = true;

/// The production fe2o3 compiler cannot yet lower the gfx950 device terminals.
pub const GFX950_RUST_TO_HSACO_LOWERING_SUPPORTED_V1: bool = false;

/// Exact missing boundary between these sources and a fe2o3-produced HSACO.
pub const GFX950_RUST_TO_HSACO_BLOCKER_V1: &str = "the rustc semantic importer, Kernel IR schema, production target profile, and AMDGPU module lowering do not yet consume the gfx950 scaled-MFMA, LDS-transpose, subgroup, or DeviceMath exp_f32 terminals";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lowering_boundary_is_explicit() {
        let source = include_str!("lib.rs");
        assert!(source.contains("GFX950_RUST_KERNEL_SOURCE_PRESENT_V1: bool = true"));
        assert!(source.contains("GFX950_RUST_TO_HSACO_LOWERING_SUPPORTED_V1: bool = false"));
        assert!(GFX950_RUST_TO_HSACO_BLOCKER_V1.contains("semantic importer"));
        assert!(GFX950_RUST_TO_HSACO_BLOCKER_V1.contains("Kernel IR"));
        assert!(GFX950_RUST_TO_HSACO_BLOCKER_V1.contains("production target"));
    }
}
