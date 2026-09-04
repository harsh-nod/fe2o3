#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Bounded host/model foundation for Qwen3 causal paged GQA decode attention.
//!
//! This standalone crate admits only the exact target and draft Qwen3
//! geometries and the seven finite M1 B3 decode/speculative buckets. It models
//! read-only attention over already projected, QK-normalized, rotary-encoded,
//! and K3-initialized P16 KV pages. It does not implement projection, RoPE, KV
//! writes, verification, acceptance, rollback, or output projection.
//!
//! The crate has no GPU source or GPU schedule. Its records are not Verus proof
//! evidence, compiler custody, artifacts, load handles, or launch authority.
//! The production compiler boundary remains closed pending the same-session
//! Rust MIR authority join tracked by fe2o3 issue #174 and later boundaries.

mod bf16;
mod contract;
mod identity;
mod reference;

pub use bf16::*;
pub use contract::*;
pub use identity::*;
pub use reference::*;

/// Whether source-to-KIR compiler authority exists for this foundation.
pub const PAGED_GQA_DECODE_SOURCE_TO_KIR_SUPPORTED_V1: bool = false;
/// Whether this foundation contains or executes a Verus proof.
pub const PAGED_GQA_DECODE_VERUS_PROOF_SUPPORTED_V1: bool = false;
/// Whether this foundation can create or publish an artifact.
pub const PAGED_GQA_DECODE_ARTIFACT_PUBLICATION_SUPPORTED_V1: bool = false;
/// Whether this foundation can load an artifact.
pub const PAGED_GQA_DECODE_ARTIFACT_LOAD_SUPPORTED_V1: bool = false;
/// Whether this foundation can dispatch or launch work.
pub const PAGED_GQA_DECODE_LAUNCH_SUPPORTED_V1: bool = false;
/// Whether IEEE or source-to-machine refinement is established.
pub const PAGED_GQA_DECODE_MACHINE_REFINEMENT_PROVED_V1: bool = false;
/// Current fail-closed production boundary.
pub const PAGED_GQA_DECODE_PRODUCTION_BLOCKER_V1: &str = "the paged host/model record has no owner-consuming same-session Rust MIR authority join, Verus discharge, machine refinement, artifact admission, or protected runtime join";
