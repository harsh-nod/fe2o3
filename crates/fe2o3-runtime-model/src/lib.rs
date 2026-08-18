#![no_std]
#![forbid(unsafe_code)]

//! Pure Rust executable model for issue #137 runtime lifecycles.
//!
//! The model performs no I/O and grants no KFD, DRM, load, dispatch, completion,
//! or proof authority. It is the finite state-machine carrier that future Verus
//! specifications and syscall refinement layers can relate to concrete runtime
//! execution.

extern crate alloc;

mod identity;
mod model;

pub use identity::*;
pub use model::*;

#[cfg(test)]
mod tests;
