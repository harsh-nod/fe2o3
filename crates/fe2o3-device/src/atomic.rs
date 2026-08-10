//! Bounded standard-Rust atomic surface for the reviewed gfx942 profile.
//!
//! These are ordinary [`core::sync::atomic`] types, not replacements. The
//! compiler contract maps operations to Kernel IR atomics with system scope.
//! System-scoped global operations require coherent-allocation launch evidence.
//! The executable subset is deliberately limited to 32-bit and 64-bit integers.

pub use core::sync::atomic::{AtomicI32, AtomicI64, AtomicU32, AtomicU64, Ordering};

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
