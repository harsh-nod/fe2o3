#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Exact B3 Qwen3 logits-projection, argmax, and compact-record host model.
//!
//! This standalone crate binds target/draft hidden widths, vocabulary 151936,
//! all exact B3 active-row buckets, BF16 activation/weight storage, explicit
//! FP32 projection order, lowest-token-ID argmax ties, and generation/epoch/
//! plan-bound compact records. The reference streams logits and publishes only
//! a fully completed compact-record batch.
//!
//! It contains no GPU code, compiler integration, proof evidence, artifact,
//! load handle, or launch capability. Issue #174 and every later production
//! authority boundary remain closed.

mod bf16;
mod contract;
mod identity;
mod reference;

pub use bf16::*;
pub use contract::*;
pub use identity::*;
pub use reference::*;

/// Whether source-to-KIR authority exists.
pub const LOGITS_COMPACT_SOURCE_TO_KIR_SUPPORTED_V1: bool = false;
/// Whether this crate contains a Verus proof.
pub const LOGITS_COMPACT_VERUS_PROOF_SUPPORTED_V1: bool = false;
/// Whether artifact publication is supported.
pub const LOGITS_COMPACT_ARTIFACT_PUBLICATION_SUPPORTED_V1: bool = false;
/// Whether artifact loading is supported.
pub const LOGITS_COMPACT_ARTIFACT_LOAD_SUPPORTED_V1: bool = false;
/// Whether dispatch or launch is supported.
pub const LOGITS_COMPACT_LAUNCH_SUPPORTED_V1: bool = false;
/// Whether IEEE or source-to-machine refinement is established.
pub const LOGITS_COMPACT_MACHINE_REFINEMENT_PROVED_V1: bool = false;
/// Current fail-closed production boundary.
pub const LOGITS_COMPACT_PRODUCTION_BLOCKER_V1: &str = "issue #174 same-session Rust MIR authority join, Verus discharge, machine refinement, artifact admission, and protected runtime completion join remain absent";
