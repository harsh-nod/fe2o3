//! Bounded standard-Rust atomic surface for the reviewed gfx942 profile.
//!
//! These are ordinary [`core::sync::atomic`] types, not replacements. The
//! compiler contract maps operations to Kernel IR atomics with system scope.
//! System-scoped global operations require coherent-allocation launch evidence.
//! The executable subset is deliberately limited to 32-bit and 64-bit integers.

use crate::DeviceGlobalMutPtr;

pub use core::sync::atomic::{AtomicI32, AtomicI64, AtomicU32, AtomicU64, Ordering};

macro_rules! global_atomic_view {
    ($element:ty, $atomic:ty, $diagnostic:literal) => {
        impl DeviceGlobalMutPtr<$element> {
            /// Borrows this global pointer as a shared core atomic object.
            ///
            /// The returned reference cannot outlive the pointer token borrow.
            /// Every operation still names its Rust [`Ordering`] explicitly;
            /// fe2o3 assigns those operations system scope.
            ///
            /// The unsafe construction or compiler ABI admission of this
            /// pointer must establish that it remains live and aligned for
            /// the concrete core atomic type throughout the borrow, identifies
            /// coherent global storage, and has no conflicting non-atomic or
            /// differently sized aliases. Safe callers can only access the
            /// pointee atomically through the returned reference.
            #[rustc_diagnostic_item = $diagnostic]
            pub fn as_atomic(&self) -> &$atomic {
                // SAFETY: DeviceGlobalMutPtr construction/ABI admission owns
                // the validity, lifetime, alignment, and aliasing invariant
                // documented above. The concrete impl fixes the atomic type
                // and width, and this borrow bounds the returned reference.
                unsafe { <$atomic>::from_ptr(self.as_raw()) }
            }
        }
    };
}

global_atomic_view!(
    u32,
    AtomicU32,
    "fe2o3_device_global_mut_ptr_u32_as_atomic_v1"
);
global_atomic_view!(
    i32,
    AtomicI32,
    "fe2o3_device_global_mut_ptr_i32_as_atomic_v1"
);
global_atomic_view!(
    u64,
    AtomicU64,
    "fe2o3_device_global_mut_ptr_u64_as_atomic_v1"
);
global_atomic_view!(
    i64,
    AtomicI64,
    "fe2o3_device_global_mut_ptr_i64_as_atomic_v1"
);

/// Default synchronization scope assigned to ordinary Rust atomic operations.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum CoreAtomicDefaultScope {
    /// Rust atomics are visible to the complete system.
    System,
}

/// Scope used when source code does not request an extension.
pub const CORE_ATOMIC_DEFAULT_SCOPE: CoreAtomicDefaultScope = CoreAtomicDefaultScope::System;

/// Integer widths admitted by the bounded gfx942 standard-atomic lowering.
pub const GFX942_CORE_ATOMIC_WIDTHS: &[u16] = &[32, 64];

/// Returns whether `width_bits` is in the bounded executable subset.
pub const fn gfx942_supports_core_atomic_width(width_bits: u16) -> bool {
    matches!(width_bits, 32 | 64)
}
