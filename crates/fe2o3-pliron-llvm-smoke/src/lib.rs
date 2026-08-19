//! Dialect-only integration checks for the pinned Pliron LLVM vocabulary.

/// Exact Pliron v0.17.0 revision shared by `pliron` and `pliron-llvm`.
pub const PLIRON_REVISION: &str = "2610651306ea3ba670f68d5d8b1e1159bcd521ed";

/// License declared by the pinned upstream Pliron workspace.
pub const PLIRON_LLVM_LICENSE: &str = "Apache-2.0";

/// Features intentionally enabled on the dialect-only dependency.
pub const PLIRON_LLVM_FEATURES: &[&str] = &["std"];
