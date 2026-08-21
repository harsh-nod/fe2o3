#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Bounded host/model foundation for Qwen3 causal GQA prefill attention.
//!
//! This standalone crate admits only the target and draft Qwen3 geometries and
//! the four finite M1 B3 prefill buckets. It supplies exact tensor layouts,
//! GQA head mapping, a strict BF16-input/FP32 host evaluation order, checked
//! effect and resource records, transactional output, an independent `f64`
//! differential oracle, and canonical inert structural identities.
//!
//! It contains no GPU source or GPU schedule. Its records are not Verus proof
//! evidence, compiler custody, artifacts, load handles, or launch authority.
//! The production compiler boundary remains closed pending the same-session
//! Rust MIR authority join tracked by fe2o3 issue #174 and all later proof and
//! machine boundaries.

mod bf16;
mod contract;
mod identity;
mod reference;

pub use bf16::*;
pub use contract::*;
pub use identity::*;
pub use reference::*;

/// Whether source-to-KIR compiler authority exists for this foundation.
pub const GQA_PREFILL_SOURCE_TO_KIR_SUPPORTED_V1: bool = false;
/// Whether this foundation contains or executes a Verus proof.
pub const GQA_PREFILL_VERUS_PROOF_SUPPORTED_V1: bool = false;
/// Whether this foundation can create or publish an artifact.
pub const GQA_PREFILL_ARTIFACT_PUBLICATION_SUPPORTED_V1: bool = false;
/// Whether this foundation can load an artifact.
pub const GQA_PREFILL_ARTIFACT_LOAD_SUPPORTED_V1: bool = false;
/// Whether this foundation can dispatch or launch work.
pub const GQA_PREFILL_LAUNCH_SUPPORTED_V1: bool = false;
/// Whether IEEE or source-to-machine refinement is established.
pub const GQA_PREFILL_MACHINE_REFINEMENT_PROVED_V1: bool = false;
/// Current fail-closed production boundary.
pub const GQA_PREFILL_PRODUCTION_BLOCKER_V1: &str = "the host/model record has no owner-consuming same-session Rust MIR authority join, Verus discharge, machine refinement, artifact admission, or protected runtime join";
