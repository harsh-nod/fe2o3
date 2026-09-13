//! Dialect-only integration checks for the pinned Pliron LLVM vocabulary.

/// Exact Pliron v0.17.0 revision shared by `pliron` and `pliron-llvm`.
pub const PLIRON_REVISION: &str = "9de42fc6ca7b8f3500ccf2346d69ebbb36e889cd";

/// License declared by the pinned upstream Pliron workspace.
pub const PLIRON_LLVM_LICENSE: &str = "Apache-2.0";

/// Features intentionally enabled on the dialect-only dependency.
pub const PLIRON_LLVM_FEATURES: &[&str] = &["std"];
