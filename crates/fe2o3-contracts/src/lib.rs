#![no_std]
#![forbid(unsafe_code)]

//! Target-neutral vocabulary for describing a kernel's launch, memory indexing,
//! and verification artifacts.
//!
//! This is intentionally a small contract layer, not a SIMT execution model.

mod artifact;
mod index;
mod launch;

pub use artifact::{
    ArtifactDigest, ArtifactIdentity, KernelIdentity, ProofArtifact, ProofIdentity, ProofStatus,
    ToolIdentity,
};
pub use index::{BoundedIndex, IdentityWriteIndex};
pub use launch::{LaunchDomain1d, LaunchGeometry1d, ThreadId1d, ThreadInDomain1d};
