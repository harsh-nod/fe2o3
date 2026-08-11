//! Target-neutral semantic kernel IR for fe2o3.
//!
//! The crate intentionally has no dependency on rustc, LLVM, or a GPU vendor.
//! Frontends construct this IR, target-independent passes verify and transform
//! it, and target backends lower it to their native representation.
//!
//! [`encode_module_v1`] and [`decode_module_v1`] preserve the original bounded
//! canonical wire representation. [`encode_module_v2`] adds synchronization,
//! LDS, exact wave-width records, and typed canonical integer switches.
//! [`encode_module_v3`] adds source-bound inline assembly without changing the
//! frozen V1/V2 encodings. [`encode_module_v4`] adds 128-bit scalar carrier
//! types without changing the frozen V1/V2/V3 encodings. Decoding establishes
//! wire well-formedness only; consumers must call [`verify_module`] before
//! relying on semantic invariants.
//! V1/V2/V3/V4 reconstruct kernel-entry and import roles from their legacy records;
//! they reject device-FFI exports because the frozen function records cannot
//! distinguish those definitions from internal helpers.
//!
//! SemanticOperation is the versioned extension boundary for typed
//! target-neutral operation families. Its separate schema and payload-bearing
//! instance codecs do not alter or extend any frozen module wire format.

mod control_flow;
mod effect_extraction;
mod formal_memory_obligations;
mod ir;
mod matrix;
mod region_effects;
pub mod scalar_ops_v2;
mod semantic_operations;
mod standard_atomics;
mod types;
mod verify;
mod wire;

pub use control_flow::*;
pub use effect_extraction::*;
pub use formal_memory_obligations::*;
pub use ir::*;
pub use matrix::*;
pub use region_effects::*;
pub use semantic_operations::*;
pub use standard_atomics::*;
pub use types::*;
pub use verify::*;
pub use wire::*;
