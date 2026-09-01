//! Target-specific replay and compiler-lineage validation.
//!
//! This adapter deliberately lives in the target-backend layer. The general
//! proof verifier stays independent of AMD lowering while target-aware callers
//! can still request exact deterministic KIR-to-LLVM replay.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod compiler_multi_root_target_lineage_v1;
mod compiler_target_lineage_v1;
mod production_kir_to_llvm_replay_v1;

pub use compiler_multi_root_target_lineage_v1::{
    ValidatedCompilerMultiRootTargetLineageV1, validate_compiler_multi_root_target_lineage_v1,
};
pub use compiler_target_lineage_v1::{
    CompilerTargetLineageValidationErrorV1, ValidatedCompilerTargetLineageV1,
    validate_compiler_target_lineage_v1,
};
pub use production_kir_to_llvm_replay_v1::{
    CompilerKirToLlvmReplayValidationErrorV1, ValidatedCompilerKirToLlvmReplayV1,
    validate_compiler_kir_to_llvm_replay_v1,
};
