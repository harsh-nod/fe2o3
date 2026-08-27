//! Dialect-only integration checks for the pinned Pliron LLVM vocabulary.

/// Exact Pliron v0.17.0 revision shared by `pliron` and `pliron-llvm`.
pub const PLIRON_REVISION: &str = "5bdf861bf03e7f20242b25717fb653336d02e487";

/// License declared by the pinned upstream Pliron workspace.
pub const PLIRON_LLVM_LICENSE: &str = "Apache-2.0";

/// Features intentionally enabled on the dialect-only dependency.
pub const PLIRON_LLVM_FEATURES: &[&str] = &["std"];
