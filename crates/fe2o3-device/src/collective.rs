//! Bounded gfx942 wave64 and workgroup sum collectives.
//!
//! The public algorithms expose their exact shuffle, LDS, and barrier shape,
//! while target operations remain compiler-recognized hooks that panic closed
//! on a host or unsupported compilation path. The initial profile admits only
//! `u32`, `i32`, and `f32`, a full wave64, and power-of-two workgroups no larger
//! than 256 invocations.
//! The current Rust frontend does not recognize these reserved hooks, so this
//! source API is not yet an executable kernel path.

use core::fmt;
use core::marker::PhantomData;
use core::mem::align_of;

use crate::{Group, SubgroupTile, Workgroup};

/// Version of the bounded gfx942 collective contract.
pub const GFX942_COLLECTIVE_CONTRACT_VERSION_V1: u16 = 1;

/// Largest workgroup admitted by the first LDS collective profile.
pub const MAX_GFX942_WORKGROUP_COLLECTIVE_SIZE: u32 = 256;

mod sealed {
    pub trait CollectiveElement {}
}

/// A 32-bit value supported by the first gfx942 sum-collective profile.
///
/// This trait is sealed. Integer addition wraps modulo 2^32; floating-point
/// addition follows the compiler-authenticated strict gfx942 policy. The
/// reduction tree is deterministic but is not a sequential left fold.
pub trait Gfx942CollectiveElement: sealed::CollectiveElement + Copy {
    #[doc(hidden)]
    const ZERO: Self;

    #[doc(hidden)]
    fn __fe2o3_add(self, rhs: Self) -> Self;

    #[doc(hidden)]
    unsafe fn __fe2o3_wave64_shuffle_index(
        context: &Gfx942Collectives,
        value: Self,
        source_lane: u32,
    ) -> Self;

    #[doc(hidden)]
    unsafe fn __fe2o3_lds_store(
        context: &Gfx942Collectives,
        base: *mut Self,
        index: u32,
        value: Self,
    );

    #[doc(hidden)]
    unsafe fn __fe2o3_lds_load(context: &Gfx942Collectives, base: *mut Self, index: u32) -> Self;
}

macro_rules! collective_element {
    (
        $ty:ty,
        $zero:expr,
        $add:expr,
        $shuffle_marker:literal,
        $store_marker:literal,
        $load_marker:literal
    ) => {
        impl sealed::CollectiveElement for $ty {}

        impl Gfx942CollectiveElement for $ty {
            const ZERO: Self = $zero;

            fn __fe2o3_add(self, rhs: Self) -> Self {
                ($add)(self, rhs)
            }

            #[inline(never)]
            #[rustc_diagnostic_item = $shuffle_marker]
            unsafe fn __fe2o3_wave64_shuffle_index(
                context: &Gfx942Collectives,
                value: Self,
                source_lane: u32,
            ) -> Self {
                let _ = (context, value, source_lane);
                unreachable!("gfx942 wave64 shuffle must be lowered by the fe2o3 backend")
            }

            #[inline(never)]
            #[rustc_diagnostic_item = $store_marker]
            unsafe fn __fe2o3_lds_store(
                context: &Gfx942Collectives,
                base: *mut Self,
                index: u32,
                value: Self,
            ) {
                let _ = (context, base, index, value);
                unreachable!("gfx942 LDS store must be lowered by the fe2o3 backend")
            }

            #[inline(never)]
            #[rustc_diagnostic_item = $load_marker]
            unsafe fn __fe2o3_lds_load(
                context: &Gfx942Collectives,
                base: *mut Self,
                index: u32,
            ) -> Self {
                let _ = (context, base, index);
                unreachable!("gfx942 LDS load must be lowered by the fe2o3 backend")
            }
        }
    };
}

collective_element!(
    u32,
    0,
    u32::wrapping_add,
    "fe2o3_device_gfx942_wave64_shuffle_u32_v1",
    "fe2o3_device_gfx942_lds_store_u32_v1",
    "fe2o3_device_gfx942_lds_load_u32_v1"
);
collective_element!(
    i32,
    0,
    i32::wrapping_add,
    "fe2o3_device_gfx942_wave64_shuffle_i32_v1",
    "fe2o3_device_gfx942_lds_store_i32_v1",
    "fe2o3_device_gfx942_lds_load_i32_v1"
);
collective_element!(
    f32,
    0.0,
    |lhs, rhs| lhs + rhs,
    "fe2o3_device_gfx942_wave64_shuffle_f32_v1",
    "fe2o3_device_gfx942_lds_store_f32_v1",
    "fe2o3_device_gfx942_lds_load_f32_v1"
);

/// Compiler-created authority for the exact gfx942 wave64 collective profile.
///
/// The capability is neither `Copy`, `Clone`, `Send`, nor `Sync`. It does not
/// authenticate a launch or convergence point; those obligations remain on
/// each unsafe collective operation.
#[rustc_diagnostic_item = "fe2o3_device_gfx942_collectives_context_v1"]
pub struct Gfx942Collectives {
    _private: (),
    _not_send_sync: PhantomData<*mut ()>,
}

impl Gfx942Collectives {
    /// Creates target authority for compiler-generated device code.
    ///
    /// # Safety
    ///
    /// The backend must replace this call only after authenticating gfx942,
    /// wave64 mode, the strict floating-point policy, and the collective
    /// lowering contract represented by this crate version.
    #[doc(hidden)]
    #[inline(never)]
    #[rustc_diagnostic_item = "fe2o3_device_gfx942_collectives_from_compiler_v1"]
    pub unsafe fn from_compiler() -> Self {
        unreachable!("gfx942 collective authority must be created by authenticated lowering")
    }

    #[cfg(test)]
    fn for_host_test() -> Self {
        Self {
            _private: (),
            _not_send_sync: PhantomData,
        }
    }
}

/// Rejection returned while binding workgroup LDS collective scratch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkgroupCollectiveScratchError {
    NullBase,
    MisalignedBase { address: usize, alignment: usize },
    UnsupportedWorkgroupSize { size: u64 },
    SlotCountMismatch { required: u32, provided: u32 },
}

/// Raw LDS scratch bound to one typed workgroup snapshot.
///
/// The first profile requires exactly one 32-bit slot per invocation. The
/// value is neither `Copy`, `Clone`, `Send`, nor `Sync`, and its pointer is not
/// exposed as a Rust reference because every work-item names the shared LDS
/// allocation concurrently.
pub struct WorkgroupCollectiveScratch<'group, T: Gfx942CollectiveElement> {
    base: *mut T,
    slots: u32,
    _group: PhantomData<&'group Workgroup<'group>>,
    _not_send_sync: PhantomData<*mut ()>,
}

impl<'group, T: Gfx942CollectiveElement> WorkgroupCollectiveScratch<'group, T> {
    /// Binds a raw workgroup-address-space allocation to a group snapshot.
    ///
    /// # Safety
    ///
    /// `base` must identify exactly `slots` live, aligned `T` elements in the
    /// current workgroup's LDS allocation. Every invocation in `group` must
    /// construct an equivalent binding and use it only in the same uniform
    /// collective sequence. No other operation may access the slots until the
    /// collective returns. `group` must describe the current gfx942 workgroup.
    /// Invalid null, alignment, size, and slot-count inputs may be supplied for
    /// validation and return `Err` without requiring pointer validity.
    pub unsafe fn from_raw_parts(
        group: &'group Workgroup<'group>,
        base: *mut T,
        slots: u32,
    ) -> Result<Self, WorkgroupCollectiveScratchError> {
        let size = group.size();
        if size == 0
            || size > u64::from(MAX_GFX942_WORKGROUP_COLLECTIVE_SIZE)
            || !size.is_power_of_two()
        {
            return Err(WorkgroupCollectiveScratchError::UnsupportedWorkgroupSize { size });
        }
        let required = size as u32;
        if slots != required {
            return Err(WorkgroupCollectiveScratchError::SlotCountMismatch {
                required,
                provided: slots,
            });
        }
        if base.is_null() {
            return Err(WorkgroupCollectiveScratchError::NullBase);
        }
        let address = base as usize;
        let alignment = align_of::<T>();
        if !address.is_multiple_of(alignment) {
            return Err(WorkgroupCollectiveScratchError::MisalignedBase { address, alignment });
        }
        Ok(Self {
            base,
            slots,
            _group: PhantomData,
            _not_send_sync: PhantomData,
        })
    }

    pub const fn slots(&self) -> u32 {
        self.slots
    }
}

impl<T: Gfx942CollectiveElement> fmt::Debug for WorkgroupCollectiveScratch<'_, T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("WorkgroupCollectiveScratch")
            .field("slots", &self.slots)
            .finish_non_exhaustive()
    }
}

impl SubgroupTile<'_, 64> {
    /// Returns the wave64 sum to every lane using a fixed XOR shuffle tree.
    ///
    /// # Safety
    ///
    /// The context and tile must describe the current gfx942 wave64 epoch, all
    /// 64 lanes must be active and execute this call uniformly, and the backend
    /// must authenticate and lower every diagnostic item in this operation.
    #[rustc_diagnostic_item = "fe2o3_device_gfx942_wave64_reduce_sum_v1"]
    pub unsafe fn reduce_sum<T: Gfx942CollectiveElement>(
        &self,
        context: &Gfx942Collectives,
        value: T,
    ) -> T {
        let mut result = value;
        let mut offset = 32;
        while offset != 0 {
            let source = self.thread_rank() as u32 ^ offset;
            let peer = unsafe { T::__fe2o3_wave64_shuffle_index(context, result, source) };
            result = result.__fe2o3_add(peer);
            offset >>= 1;
        }
        result
    }

    /// Returns the inclusive wave64 prefix sum in increasing lane order.
    ///
    /// # Safety
    ///
    /// The safety requirements of [`Self::reduce_sum`] apply.
    #[rustc_diagnostic_item = "fe2o3_device_gfx942_wave64_inclusive_scan_sum_v1"]
    pub unsafe fn inclusive_scan_sum<T: Gfx942CollectiveElement>(
        &self,
        context: &Gfx942Collectives,
        value: T,
    ) -> T {
        unsafe { wave64_inclusive_scan(self.thread_rank() as u32, context, value) }
    }

    /// Returns the exclusive wave64 prefix sum in increasing lane order.
    ///
    /// Lane zero receives `T::ZERO`.
    ///
    /// # Safety
    /// The safety requirements of [`Self::reduce_sum`] apply.
    #[rustc_diagnostic_item = "fe2o3_device_gfx942_wave64_exclusive_scan_sum_v1"]
    pub unsafe fn exclusive_scan_sum<T: Gfx942CollectiveElement>(
        &self,
        context: &Gfx942Collectives,
        value: T,
    ) -> T {
        let rank = self.thread_rank() as u32;
        let inclusive = unsafe { wave64_inclusive_scan(rank, context, value) };
        let source = rank.saturating_sub(1);
        let previous = unsafe { T::__fe2o3_wave64_shuffle_index(context, inclusive, source) };
        if rank == 0 { T::ZERO } else { previous }
    }
}

unsafe fn wave64_inclusive_scan<T: Gfx942CollectiveElement>(
    rank: u32,
    context: &Gfx942Collectives,
    value: T,
) -> T {
    let mut result = value;
    let mut offset = 1;
    while offset < 64 {
        let source = rank.saturating_sub(offset);
        let prefix = unsafe { T::__fe2o3_wave64_shuffle_index(context, result, source) };
        if rank >= offset {
            result = prefix.__fe2o3_add(result);
        }
        offset <<= 1;
    }
    result
}

impl Workgroup<'_> {
    /// Returns the workgroup sum to every invocation through LDS scratch.
    ///
    /// # Safety
    ///
    /// The context, workgroup, and scratch binding must describe the current
    /// gfx942 execution. Every work-item must execute the same call uniformly.
    /// The compiler must preserve each LDS access and convergent barrier.
    #[rustc_diagnostic_item = "fe2o3_device_gfx942_workgroup_reduce_sum_v1"]
    pub unsafe fn reduce_sum<T: Gfx942CollectiveElement>(
        &self,
        context: &Gfx942Collectives,
        scratch: &mut WorkgroupCollectiveScratch<'_, T>,
        value: T,
    ) -> T {
        let rank = self.thread_rank() as u32;
        let size = self.size() as u32;
        debug_assert_eq!(size, scratch.slots);
        unsafe { T::__fe2o3_lds_store(context, scratch.base, rank, value) };
        unsafe { self.synchronize() };

        let mut offset = size >> 1;
        while offset != 0 {
            let pair = if rank < offset {
                let lhs = unsafe { T::__fe2o3_lds_load(context, scratch.base, rank) };
                let rhs = unsafe { T::__fe2o3_lds_load(context, scratch.base, rank + offset) };
                Some(lhs.__fe2o3_add(rhs))
            } else {
                None
            };
            unsafe { self.synchronize() };
            if let Some(sum) = pair {
                unsafe { T::__fe2o3_lds_store(context, scratch.base, rank, sum) };
            }
            unsafe { self.synchronize() };
            offset >>= 1;
        }

        let result = unsafe { T::__fe2o3_lds_load(context, scratch.base, 0) };
        unsafe { self.synchronize() };
        result
    }

    /// Returns the inclusive workgroup prefix sum in increasing thread rank.
    ///
    /// # Safety
    ///
    /// The safety requirements of [`Self::reduce_sum`] apply.
    #[rustc_diagnostic_item = "fe2o3_device_gfx942_workgroup_inclusive_scan_sum_v1"]
    pub unsafe fn inclusive_scan_sum<T: Gfx942CollectiveElement>(
        &self,
        context: &Gfx942Collectives,
        scratch: &mut WorkgroupCollectiveScratch<'_, T>,
        value: T,
    ) -> T {
        unsafe { workgroup_inclusive_scan(self, context, scratch, value) }
    }

    /// Returns the exclusive workgroup prefix sum in increasing thread rank.
    ///
    /// Thread rank zero receives `T::ZERO`.
    ///
    /// # Safety
    /// The safety requirements of [`Self::reduce_sum`] apply.
    #[rustc_diagnostic_item = "fe2o3_device_gfx942_workgroup_exclusive_scan_sum_v1"]
    pub unsafe fn exclusive_scan_sum<T: Gfx942CollectiveElement>(
        &self,
        context: &Gfx942Collectives,
        scratch: &mut WorkgroupCollectiveScratch<'_, T>,
        value: T,
    ) -> T {
        let rank = self.thread_rank() as u32;
        let inclusive = unsafe { workgroup_inclusive_scan(self, context, scratch, value) };
        let result = if rank == 0 {
            T::ZERO
        } else {
            unsafe { T::__fe2o3_lds_load(context, scratch.base, rank - 1) }
        };
        unsafe { self.synchronize() };
        let _ = inclusive;
        result
    }
}

unsafe fn workgroup_inclusive_scan<T: Gfx942CollectiveElement>(
    group: &Workgroup<'_>,
    context: &Gfx942Collectives,
    scratch: &mut WorkgroupCollectiveScratch<'_, T>,
    value: T,
) -> T {
    let rank = group.thread_rank() as u32;
    let size = group.size() as u32;
    debug_assert_eq!(size, scratch.slots);
    unsafe { T::__fe2o3_lds_store(context, scratch.base, rank, value) };
    unsafe { group.synchronize() };

    let mut offset = 1;
    while offset < size {
        let current = unsafe { T::__fe2o3_lds_load(context, scratch.base, rank) };
        let prefix = if rank >= offset {
            Some(unsafe { T::__fe2o3_lds_load(context, scratch.base, rank - offset) })
        } else {
            None
        };
        unsafe { group.synchronize() };
        let next = prefix.map_or(current, |prefix| prefix.__fe2o3_add(current));
        unsafe { T::__fe2o3_lds_store(context, scratch.base, rank, next) };
        unsafe { group.synchronize() };
        offset <<= 1;
    }

    let result = unsafe { T::__fe2o3_lds_load(context, scratch.base, rank) };
    unsafe { group.synchronize() };
    result
}

#[cfg(test)]
mod tests;
