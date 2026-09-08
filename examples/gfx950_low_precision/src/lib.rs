#![forbid(unsafe_code)]
#![cfg_attr(target_arch = "amdgpu", no_std)]

//! Rust-first gfx950 low-precision source examples.
//!
//! The attributed functions in [`kernel`] are the fe2o3 kernel source. The
//! neighboring HIP file is a compiler/ISA/hardware fixture, not the source of
//! these Rust kernels.

#[cfg(all(
    target_arch = "amdgpu",
    not(any(
        feature = "kernel-fp4-gemm",
        feature = "kernel-fp8-gemm",
        feature = "kernel-fp4-attention",
        feature = "kernel-fp8-attention",
    ))
))]
compile_error!("an AMDGPU build must select exactly one gfx950 kernel feature");

#[cfg(all(
    target_arch = "amdgpu",
    any(
        all(feature = "kernel-fp4-gemm", feature = "kernel-fp8-gemm"),
        all(feature = "kernel-fp4-gemm", feature = "kernel-fp4-attention"),
        all(feature = "kernel-fp4-gemm", feature = "kernel-fp8-attention"),
        all(feature = "kernel-fp8-gemm", feature = "kernel-fp4-attention"),
        all(feature = "kernel-fp8-gemm", feature = "kernel-fp8-attention"),
        all(feature = "kernel-fp4-attention", feature = "kernel-fp8-attention"),
    )
))]
compile_error!("an AMDGPU build must not select more than one gfx950 kernel feature");

pub mod kernel;
#[cfg(not(target_arch = "amdgpu"))]
pub mod reference;
#[cfg(all(not(target_arch = "amdgpu"), feature = "bundle-v8-simulator"))]
pub mod simulator;

/// The ordinary Rust kernel source exists and is checked by host compilation.
pub const GFX950_RUST_KERNEL_SOURCE_PRESENT_V1: bool = true;

/// Every root receives compiler-issued invocation, subgroup, matrix, and policy authority.
pub const GFX950_KERNEL_CONTEXT_CAPABILITIES_V1: bool = true;

/// Whether every physical memory argument has a typed source capability.
pub const GFX950_FULLY_TYPED_MEMORY_V1: bool = true;

/// Whether both GEMM roots typecheck through the capability-safe gfx950 surface.
pub const GFX950_GEMM_CAPABILITY_SOURCE_SUPPORTED_V1: bool = true;

/// Whether transpose publication composes with next-epoch MFMA in safe source.
pub const GFX950_ATTENTION_CAPABILITY_SOURCE_SUPPORTED_V1: bool = true;

/// Whether the current production extractor lowers every migrated root.
pub const GFX950_RUST_TO_HSACO_LOWERING_SUPPORTED_V1: bool = false;

/// Whether exact compiler-produced canonical KIR V13 Bundle V8 fixtures are available.
pub const GFX950_BUNDLE_V8_SUPPORTED_V1: bool = false;

/// Exact boundaries preventing issue #272 qualification for this package.
pub const GFX950_CAPABILITY_BLOCKER_V1: &str = "the typed transpose publication and next-epoch MFMA source path now typechecks; production V13 extraction, genuine Bundle V8 differential simulation, protected publication, and fresh gfx950 hardware qualification remain required";

/// Exact production finalization contract used by the runnable examples.
pub const GFX950_PRODUCTION_FINALIZER_V1: &str = "ROCm 7.2.1 clang/LLD with implicit device libraries disabled and the manifest-pinned nine-file gfx950 OCML closure";

/// Remaining boundary for protected Worker V3 publication, not Rust lowering.
pub const GFX950_PROTECTED_WORKER_BUILD_BOUNDARY_V1: &str = "the fixed root-owned protected client profile was not available in this test environment, so no current protected publication or hardware qualification was attempted";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capability_and_protected_boundaries_are_explicit() {
        let source = include_str!("lib.rs");
        assert!(source.contains("GFX950_RUST_KERNEL_SOURCE_PRESENT_V1: bool = true"));
        assert!(source.contains("GFX950_KERNEL_CONTEXT_CAPABILITIES_V1: bool = true"));
        assert!(source.contains("GFX950_FULLY_TYPED_MEMORY_V1: bool = true"));
        assert!(source.contains("GFX950_GEMM_CAPABILITY_SOURCE_SUPPORTED_V1: bool = true"));
        assert!(source.contains("GFX950_ATTENTION_CAPABILITY_SOURCE_SUPPORTED_V1: bool = true"));
        assert!(source.contains("GFX950_RUST_TO_HSACO_LOWERING_SUPPORTED_V1: bool = false"));
        assert!(source.contains("GFX950_BUNDLE_V8_SUPPORTED_V1: bool = false"));
        assert!(GFX950_CAPABILITY_BLOCKER_V1.contains("source path now typechecks"));
        assert!(GFX950_CAPABILITY_BLOCKER_V1.contains("Bundle V8"));
        assert!(GFX950_PRODUCTION_FINALIZER_V1.contains("implicit device libraries disabled"));
        assert!(GFX950_PRODUCTION_FINALIZER_V1.contains("nine-file gfx950 OCML closure"));
        assert!(GFX950_PROTECTED_WORKER_BUILD_BOUNDARY_V1.contains("protected client profile"));
        assert!(GFX950_PROTECTED_WORKER_BUILD_BOUNDARY_V1.contains("not available"));
    }
}
