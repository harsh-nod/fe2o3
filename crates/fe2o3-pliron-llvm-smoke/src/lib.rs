//! Dialect-only integration checks for the pinned Pliron LLVM vocabulary.

/// Exact Pliron v0.17.0 revision shared by `pliron` and `pliron-llvm`.
pub const PLIRON_REVISION: &str = "e054e5b2e53c7330470f9202c35c8c0e4e102092";

/// License declared by the pinned upstream Pliron workspace.
pub const PLIRON_LLVM_LICENSE: &str = "Apache-2.0";

/// Features intentionally enabled on the dialect-only dependency.
pub const PLIRON_LLVM_FEATURES: &[&str] = &["std"];
