#![no_std]
#![forbid(unsafe_code)]

//! Executable-free persistent GPU service semantics.
//!
//! This crate models the P0 identity and state-machine foundation from issue
//! #135. It does not implement persistent execution and grants no proof,
//! artifact, load, launch, runtime, progress, or performance authority.

extern crate alloc;

mod identity;
mod property;
mod state;

pub use identity::*;
pub use property::*;
pub use state::*;

#[cfg(test)]
mod tests;
