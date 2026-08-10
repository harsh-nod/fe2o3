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

pub mod atomic;
pub mod collective;
pub mod diagnostics;
pub mod ffi;
pub mod fp8;
pub mod group;
pub mod half;
pub mod lds;
pub mod math;
pub mod memory;
pub mod mx;
pub mod simd;
pub mod sync;
pub mod tensor;
pub mod thread;
pub mod views;
pub mod wave;

pub use collective::{
    GFX942_COLLECTIVE_CONTRACT_VERSION_V1, Gfx942CollectiveElement, Gfx942Collectives,
    MAX_GFX942_WORKGROUP_COLLECTIVE_SIZE, WorkgroupCollectiveScratch,
    WorkgroupCollectiveScratchError,
};
pub use diagnostics::{clock32, debugtrap, trap};
pub use fe2o3_macros::{device_export, device_import, import_device, import_kernel, kernel};
pub use ffi::{
    DeviceConstantPtr, DeviceFfiAbiTypeV1, DeviceGlobalConstPtr, DeviceGlobalMutPtr,
    DevicePrivateConstPtr, DevicePrivateMutPtr, DeviceWorkgroupConstPtr, DeviceWorkgroupMutPtr,
};
pub use fp8::{Fp8E4M3Fnuz, Fp8E4M3Fnuzx4, Fp8E5M2Fnuz, Fp8E5M2Fnuzx4};
pub use group::{
    ActiveLaneGroup, Grid, Group, GroupMemoryOrdering, GroupMemorySpace, GroupScope, SubgroupTile,
    SynchronizationContract, TYPED_GROUP_CONTRACT_VERSION_V1, UnsupportedSynchronization,
    ValidWave64TileWidth, Wave64TileWidth, Workgroup, WorkgroupSynchronization,
};
pub use half::{Bf16, Bf16x2, F16};
pub use lds::{
    DynamicLds, DynamicLdsError, LdsElement, LdsInitialized, LdsUninitialized,
    MAX_DYNAMIC_LDS_ALIGNMENT, WorkgroupLdsScope,
};
pub use math::{DEVICE_MATH_CONTRACT_VERSION_V1, DeviceMath};
pub use mx::{MxScaleConversionError, MxScaleE8M0, MxScaleE8M0x4};
pub use simd::{GpuSimd, GpuSimdElement, GpuSimdLaneCount, ValidGpuSimdLaneCount};
pub use sync::{
    AmdBarrierTarget, BarrierInitializationError, BarrierPending, BarrierReady,
    BarrierUninitialized, Gfx12, Gfx942, ManagedBarrier, NamedBarrierSlot,
    NativeSplitBarrierTarget, ValidNamedBarrierSlot,
};
pub use tensor::{
    BF16_F32_MFMA_M, BF16_F32_MFMA_N, BF16_F32_MFMA_REDUCTION, BF16_F32_MFMA_WAVE_LANES,
    Bf16F32M16N16K16, Bf16MfmaFragment, DeviceMatrix, F32AccumulatorFragment, LdsTile16x16,
    LdsTileShapeError, MATRIX_CONTRACT_VERSION_V1, RowMajorXor4,
};
pub use thread::{
    GlobalGridSize, GlobalWorkitemId, GridSize, Index1D, Index2D, Invocation3D, ThreadIndex,
    WorkgroupId, WorkgroupSize, WorkitemId,
};
pub use views::{
    DisjointStaticTileMut, StaticIndex, StaticTileRegionWitness, StaticView, StaticViewError,
    StaticViewMut,
};
pub use wave::{Wave32, Wave64, WaveLane, WaveWidth};

/// Executes one operation from the closed, typed gfx942 vector-ALU allowlist.
#[macro_export]
macro_rules! amdgpu_asm {
    (v_mov_b32($value:expr)) => {
        $crate::diagnostics::__amdgpu_v_mov_b32_v1($value)
    };
    (v_add_u32($lhs:expr, $rhs:expr)) => {
        $crate::diagnostics::__amdgpu_v_add_u32_v1($lhs, $rhs)
    };
    (v_sub_u32($lhs:expr, $rhs:expr)) => {
        $crate::diagnostics::__amdgpu_v_sub_u32_v1($lhs, $rhs)
    };
    (v_and_b32($lhs:expr, $rhs:expr)) => {
        $crate::diagnostics::__amdgpu_v_and_b32_v1($lhs, $rhs)
    };
    (v_or_b32($lhs:expr, $rhs:expr)) => {
        $crate::diagnostics::__amdgpu_v_or_b32_v1($lhs, $rhs)
    };
    (v_xor_b32($lhs:expr, $rhs:expr)) => {
        $crate::diagnostics::__amdgpu_v_xor_b32_v1($lhs, $rhs)
    };
    ($($unsupported:tt)*) => {
        compile_error!("unsupported amdgpu_asm! operation; use the typed gfx942 V1 allowlist")
    };
}

/// Emits one bounded diagnostic-format event with at most two `u32` values.
#[macro_export]
macro_rules! gpu_printf {
    ($format:literal $(,)?) => {{
        const FORMAT_ID: u32 = match $crate::diagnostics::__checked_format_id_v1($format, 0) {
            Some(id) => id,
            None => panic!("gpu_printf! format is outside the bounded V1 grammar"),
        };
        $crate::diagnostics::__gpu_printf_0_v1(FORMAT_ID)
    }};
    ($format:literal, $value0:expr $(,)?) => {{
        const FORMAT_ID: u32 = match $crate::diagnostics::__checked_format_id_v1($format, 1) {
            Some(id) => id,
            None => panic!("gpu_printf! format is outside the bounded V1 grammar"),
        };
        $crate::diagnostics::__gpu_printf_1_v1(FORMAT_ID, $value0)
    }};
    ($format:literal, $value0:expr, $value1:expr $(,)?) => {{
        const FORMAT_ID: u32 = match $crate::diagnostics::__checked_format_id_v1($format, 2) {
            Some(id) => id,
            None => panic!("gpu_printf! format is outside the bounded V1 grammar"),
        };
        $crate::diagnostics::__gpu_printf_2_v1(FORMAT_ID, $value0, $value1)
    }};
    ($($unsupported:tt)*) => {
        compile_error!("gpu_printf! requires a literal V1 format and at most two u32 values")
    };
}

/// Traps without unwinding when a device assertion fails.
#[macro_export]
macro_rules! gpu_assert {
    ($condition:expr $(,)?) => {{
        if !$condition {
            const SITE_ID: u32 =
                $crate::diagnostics::__site_id_v1(concat!(file!(), ":", stringify!($condition)));
            $crate::diagnostics::__gpu_assert_fail_v1(SITE_ID, line!())
        }
    }};
    ($condition:expr, $message:literal $(,)?) => {{
        const SITE_ID: u32 = match $crate::diagnostics::__checked_format_id_v1($message, 0) {
            Some(id) => id,
            None => panic!("gpu_assert! message is outside the bounded V1 grammar"),
        };
        if !$condition {
            $crate::diagnostics::__gpu_assert_fail_v1(SITE_ID, line!())
        }
    }};
    ($($unsupported:tt)*) => {
        compile_error!("gpu_assert! accepts a condition and an optional literal message")
    };
}

/// Emits a target-gated gfx942 profiling marker in the range `0..=65535`.
#[macro_export]
macro_rules! profiling_marker {
    ($marker:literal $(,)?) => {{
        const MARKER: u32 = $marker;
        const _: () = assert!(
            MARKER <= u16::MAX as u32,
            "profiling marker exceeds V1 range"
        );
        $crate::diagnostics::__profiling_marker_v1(MARKER)
    }};
    ($($unsupported:tt)*) => {
        compile_error!("profiling_marker! requires one u16-range integer literal")
    };
}

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

/// Producer-owned identity needed to import one typed kernel from an upstream crate.
///
/// # Safety
///
/// Every identity must describe the exact function and registration exposed by
/// the supertrait. Implementations are generated only by the kernel macro.
pub unsafe trait CrossCrateTypedKernelV1: KernelMarkerV1 {
    const REGISTRATION_VERSION: u16;
    const REGISTRATION_KIND: u16;
    const CRATE_BINDING: &'static str;
    const KERNEL_BINDING: &'static str;
}

/// Producer-owned identity needed to retain one upstream standalone device export.
///
/// # Safety
///
/// The contract identity and function pointer must name the same generated
/// device_export declaration. Implementations are generated by that macro.
pub unsafe trait CrossCrateDeviceExportV1 {
    type Function: Copy + Send + Sync + 'static;

    const CONTRACT_ID: &'static str;
    const FUNCTION: Self::Function;
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

    /// Checks and borrows one fixed-size tile relative to this exact region.
    ///
    /// The returned tile embeds a private witness carrying this
    /// `DisjointSlice`'s pointer, element extent, and checked start offset. The
    /// extent check occurs only here; constant-index accesses on the returned
    /// tile do not repeat it. The mutable borrow prevents the parent view from
    /// being accessed until the tile is dropped.
    pub fn checked_static_tile_mut<const N: usize>(
        &mut self,
        start_element: usize,
    ) -> Result<DisjointStaticTileMut<'_, T, IndexSpace, N>, StaticViewError> {
        let ptr = self.ptr;
        let len = self.len;
        DisjointStaticTileMut::from_disjoint_region(self, ptr, len, start_element)
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
