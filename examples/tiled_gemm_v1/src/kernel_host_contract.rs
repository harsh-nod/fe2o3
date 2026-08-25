//! Authority-free host contract for the attributed Slice 1 kernel.
//!
//! The standalone package enables this module with its default
//! `host-contract` feature. It preserves the public constants and exact Rust
//! function-pointer contract used by host tests, but its basic kernel marker
//! carries neither a compiler-derived crate binding nor typed cross-crate
//! production authority. The managed cargo-fe2o3 fixture compiles `kernel.rs`
//! directly and never uses this module.
//!
//! A host marker cannot be substituted for a compiler-bound typed marker:
//!
//! ```compile_fail
//! use fe2o3_device::CrossCrateTypedKernelV1;
//! use fe2o3_tiled_gemm_v1::kernel::__fe2o3_kernel_marker_tiled_gemm_lds_slice1;
//!
//! fn require_production_marker<T: CrossCrateTypedKernelV1>() {}
//! require_production_marker::<__fe2o3_kernel_marker_tiled_gemm_lds_slice1>();
//! ```

#![allow(missing_docs)] // The generated basic-kernel marker lacks rustdoc.

use fe2o3_device::{Blocked, DisjointSlice, Index1D, kernel};

/// Exact workgroup dimensions required by the Slice 1 source contract.
pub const LDS_SLICE1_WORKGROUP_V1: [u32; 3] = [64, 1, 1];
/// Number of BF16 elements in each XOR4-staged operand tile.
pub const LDS_SLICE1_OPERAND_ELEMENTS_V1: usize = 16 * 16;
/// Number of bytes occupied by each XOR4-staged BF16 operand tile.
pub const LDS_SLICE1_OPERAND_BYTES_V1: usize = LDS_SLICE1_OPERAND_ELEMENTS_V1 * 2;
/// Total LDS bytes required for the separate A and transposed-B tiles.
pub const LDS_SLICE1_TOTAL_BYTES_V1: usize = 2 * LDS_SLICE1_OPERAND_BYTES_V1;

/// Whether the attributed Rust source reaches the verified canonical LDS IR.
pub const LDS_SLICE1_SOURCE_TO_IR_SUPPORTED_V1: bool = true;

/// Whether the current source frontend lowers this kernel through LLVM/HSACO.
pub const LDS_SLICE1_SOURCE_LOWERING_SUPPORTED_V1: bool = false;

/// Current fail-closed reason for the Slice 1 source lowering boundary.
pub const LDS_SLICE1_SOURCE_BLOCKER_V1: &str =
    "the source-to-IR receipt stops before compiler descriptor construction";

/// Complete current compiler worklist before this source can become executable.
pub const LDS_SLICE1_SOURCE_BLOCKERS_V1: [&str; 4] = [
    LDS_SLICE1_SOURCE_BLOCKER_V1,
    "the authenticated source path is not joined to the dedicated upstream-LLVM LDS lowering",
    "the reviewed source-to-IR correspondence is not a compiler-refinement proof",
    "protected Worker V2 publication, HSACO load, and launch remain fail-closed",
];

/// Authority-free host representation of the production kernel's exact Rust ABI.
///
/// Calling it always fails before it can observe or mutate an argument. This
/// declaration exists only to exercise [`fe2o3_device::KernelMarkerV1`] from an
/// ordinary host build; it is not a device kernel or production registration.
#[kernel]
pub fn tiled_gemm_lds_slice1(
    _a: &[u16],
    _b: &[u16],
    _c: DisjointSlice<f32, Blocked<Index1D, 16, 4>>,
) {
    panic!("host-contract-only kernel marker cannot execute")
}
