//! Shared hard limits for production semantic-anchor replay and admission.

use fe2o3_kernel_ir::MAX_TEXT_BYTES_V1;

/// Frozen replay V1 permits exactly four MiB of canonical evidence.
pub const MAX_PRODUCTION_KIR_TO_LLVM_REPLAY_EVIDENCE_BYTES_V1: usize = 4 * 1024 * 1024;
/// Magic plus fixed scalar and identity fields before the variable kernel id and LLVM text.
pub const PRODUCTION_KIR_TO_LLVM_REPLAY_FIXED_BYTES_V1: usize = 122;
/// Frozen replay V1 LLVM field limit. Short kernel ids can use this full historical range.
pub const MAX_PRODUCTION_LEGACY_REPLAY_LLVM_TEXT_BYTES_V1: usize =
    MAX_PRODUCTION_KIR_TO_LLVM_REPLAY_EVIDENCE_BYTES_V1 - 256;
/// Largest LLVM field that fits replay V1 even when the kernel id has its maximum length.
pub const MAX_PRODUCTION_SEMANTIC_ANCHOR_LLVM_TEXT_BYTES_V1: usize =
    MAX_PRODUCTION_KIR_TO_LLVM_REPLAY_EVIDENCE_BYTES_V1
        - PRODUCTION_KIR_TO_LLVM_REPLAY_FIXED_BYTES_V1
        - MAX_TEXT_BYTES_V1;
/// Maximum number of semantic anchor records emitted and admitted by V1.
///
/// The 16K bound admits production multi-wave kernels whose checked slice-offset and ownership
/// paths expand beyond the original 4K teaching-kernel profile while remaining well below the
/// compiler-module and ranked-analysis operation limits.
pub const MAX_PRODUCTION_SEMANTIC_ANCHORS_V1: usize = 16 * 1024;
