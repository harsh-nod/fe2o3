//! Shared borrowed membership only. Each caller prepays its own scan policy.
use fe2o3_compiler_ffi::{
    CompilerModuleSymbolManifestV1 as Manifest, CompilerModuleSymbolRoleV1 as Role,
};

pub(crate) fn contains(manifest: &Manifest, role: Role, name: &str) -> bool {
    manifest.symbols(role).any(|symbol| symbol == name)
}

pub(crate) fn count_matches(manifest: &Manifest, role: Role, count: usize) -> bool {
    manifest.role_count(role) == count
}
