#![no_std]
#![feature(core_float_math)]
#![feature(rustc_attrs)]
#![allow(internal_features)]

//! Device-side API for fe2o3 kernels.
//!
//! The internal diagnostic-item attributes are semantic identities consumed by
//! the backend pinned to this repository's nightly toolchain. They do not
//! authenticate this crate's package source or contents.

#[cfg(test)]
extern crate std;

use core::marker::PhantomData;

pub mod ffi;
pub mod fp8;
pub mod group;
pub mod half;
pub mod lds;
pub mod math;
pub mod mx;
pub mod simd;
pub mod sync;
pub mod thread;
pub mod views;
pub mod wave;

pub use fe2o3_macros::{device_export, device_import, kernel};
pub use ffi::{
    DeviceConstantPtr, DeviceFfiAbiTypeV1, DeviceGlobalConstPtr, DeviceGlobalMutPtr,
    DevicePrivateConstPtr, DevicePrivateMutPtr, DeviceWorkgroupConstPtr, DeviceWorkgroupMutPtr,
};
pub use fp8::{Fp8E4M3Fnuz, Fp8E4M3Fnuzx4, Fp8E5M2Fnuz, Fp8E5M2Fnuzx4};
pub use group::{
    ActiveLaneGroup, Grid, Group, GroupMemoryOrdering, GroupMemorySpace, GroupScope, SubgroupTile,
    SynchronizationContract, UnsupportedSynchronization, ValidWave64TileWidth, Wave64TileWidth,
    Workgroup, WorkgroupSynchronization,
};
pub use half::{Bf16, Bf16x2, F16};
pub use lds::{
    DynamicLds, DynamicLdsError, LdsElement, LdsInitialized, LdsUninitialized,
    MAX_DYNAMIC_LDS_ALIGNMENT, WorkgroupLdsScope,
};
pub use math::{DEVICE_MATH_CONTRACT_VERSION_V1, DeviceMath};
pub use mx::{MxScaleConversionError, MxScaleE8M0, MxScaleE8M0x4};
pub use simd::{GpuSimd, GpuSimdElement, GpuSimdLaneCount, ValidGpuSimdLaneCount};
pub use thread::{
    GlobalGridSize, GlobalWorkitemId, GridSize, Index1D, Index2D, Invocation3D, ThreadIndex,
    WorkgroupId, WorkgroupSize, WorkitemId,
};
pub use views::{StaticIndex, StaticView, StaticViewError, StaticViewMut};
pub use wave::{Wave32, Wave64, WaveLane, WaveWidth};

/// Version of the type-level kernel marker contract emitted by [`kernel`].
///
/// This versions the Rust trait contract. It intentionally does not identify a
/// compiled artifact, target, proof, or launch authorization.
pub const KERNEL_MARKER_CONTRACT_VERSION_V1: u16 = 1;

/// Type-level identity for one function and its v1 kernel registration.
///
/// The associated function-pointer type preserves the function's argument and
/// result types, safety, and calling convention. It is not a packed argument
/// layout and carries no promise about a compiled artifact.
///
/// # Safety
///
/// An implementation is a compiler contract. It must be emitted for exactly
/// one kernel function and satisfy all of the following requirements:
///
/// - [`Self::Function`] is the exact function-pointer type of that function,
///   including its argument and result types, safety, and calling convention;
/// - [`Self::FUNCTION`] is that function, not a wrapper or a different item;
/// - [`Self::LOGICAL_NAME`] and [`Self::EXPORT_NAME`] exactly match the names in
///   the associated collector registration;
/// - [`Self::Registration`] is the exact type of that registration, including
///   its final function-pointer field; and
/// - [`Self::REGISTRATION`] refers to that collector-visible registration.
///
/// Satisfying this contract does not establish an artifact digest, target
/// identity, packed ABI layout, verification result, or runtime launch safety.
/// Manual implementations are unsafe and must justify every association above.
pub unsafe trait KernelMarkerV1 {
    /// Exact function-pointer type of the registered kernel.
    type Function: Copy + Send + Sync + 'static;

    /// Exact tuple type of the collector-visible registration.
    type Registration: Sync + 'static;

    /// Logical source-level kernel name stored in the registration.
    const LOGICAL_NAME: &'static str;

    /// Exported symbol name stored in the registration.
    const EXPORT_NAME: &'static str;

    /// Exact registered kernel function.
    const FUNCTION: Self::Function;

    /// Exact collector-visible registration for this marker.
    const REGISTRATION: &'static Self::Registration;
}

#[derive(Debug)]
#[repr(C)]
#[rustc_diagnostic_item = "fe2o3_device_disjoint_slice"]
pub struct DisjointSlice<T, IndexSpace = Index1D> {
    ptr: *mut T,
    len: usize,
    _index_space: PhantomData<fn() -> IndexSpace>,
}

impl<T, IndexSpace> DisjointSlice<T, IndexSpace> {
    /// Returns host-rustc layout facts used by the generated ABI evidence
    /// profile. These values are data only and grant no artifact authority.
    #[doc(hidden)]
    pub const fn __fe2o3_rust_layout_v1() -> (usize, usize, usize, usize) {
        (
            core::mem::size_of::<Self>(),
            core::mem::align_of::<Self>(),
            core::mem::offset_of!(Self, ptr),
            core::mem::offset_of!(Self, len),
        )
    }

    /// Constructs a device slice from its raw representation.
    ///
    /// # Safety
    ///
    /// `ptr` must be aligned and valid for reads and writes of `len` consecutive
    /// `T` values for every use of the returned slice. Those values must not be
    /// accessed through another alias while this slice is used.
    /// `IndexSpace` must describe the invocation-to-element mapping used by
    /// every safe access to the returned view.
    pub unsafe fn from_raw_parts(ptr: *mut T, len: usize) -> Self {
        Self {
            ptr,
            len,
            _index_space: PhantomData,
        }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    #[rustc_diagnostic_item = "fe2o3_device_disjoint_slice_get_mut"]
    pub fn get_mut(&mut self, index: ThreadIndex<IndexSpace>) -> Option<&mut T> {
        // SAFETY: `ThreadIndex` can only be produced for the current device
        // invocation and its index-space type matches this view, so its index
        // preserves the view's per-invocation write partition.
        // `from_raw_parts` establishes pointer validity and aliasing.
        unsafe { self.get_mut_at(index.get()) }
    }

    /// Returns mutable access to an arbitrary integer index in this view.
    ///
    /// Returns `None` when `index` is outside the view's element extent.
    ///
    /// # Safety
    ///
    /// In addition to satisfying the validity and aliasing requirements of
    /// [`Self::from_raw_parts`], the caller must prove that no concurrently
    /// executing invocation can access the selected element through this view
    /// or any alias unless those accesses are synchronized and compatible.
    /// The index calculation must not undermine the exclusive-write partition
    /// represented by this `DisjointSlice`.
    #[rustc_diagnostic_item = "fe2o3_device_disjoint_slice_get_mut_at"]
    pub unsafe fn get_mut_at(&mut self, index: usize) -> Option<&mut T> {
        if index >= self.len {
            return None;
        }
        Some(unsafe { &mut *self.ptr.add(index) })
    }
}

#[cfg(test)]
mod tests {
    use super::{DisjointSlice, Index1D, Index2D, KERNEL_MARKER_CONTRACT_VERSION_V1};
    use core::mem::{align_of, size_of};

    #[test]
    fn index_space_markers_do_not_change_the_slice_abi() {
        let expected_size = size_of::<*mut u32>() + size_of::<usize>();
        let expected_align = align_of::<*mut u32>().max(align_of::<usize>());

        assert_eq!(size_of::<DisjointSlice<u32, Index1D>>(), expected_size);
        assert_eq!(align_of::<DisjointSlice<u32, Index1D>>(), expected_align);
        assert_eq!(size_of::<DisjointSlice<u32, Index2D<64>>>(), expected_size);
        assert_eq!(
            align_of::<DisjointSlice<u32, Index2D<64>>>(),
            expected_align
        );
        assert_eq!(
            DisjointSlice::<u32, Index1D>::__fe2o3_rust_layout_v1(),
            (expected_size, expected_align, 0, size_of::<*mut u32>())
        );
        assert_eq!(
            DisjointSlice::<u32, Index2D<64>>::__fe2o3_rust_layout_v1(),
            (expected_size, expected_align, 0, size_of::<*mut u32>())
        );
    }

    #[test]
    fn kernel_marker_contract_version_is_stable() {
        assert_eq!(KERNEL_MARKER_CONTRACT_VERSION_V1, 1);
    }
}
