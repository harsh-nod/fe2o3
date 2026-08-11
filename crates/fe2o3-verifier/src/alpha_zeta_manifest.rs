use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path, PathBuf};
use std::str::FromStr;
use std::sync::Arc;

use fe2o3_artifacts::DigestAlgorithm;
use proc_macro2::{TokenStream, TokenTree};
use syn::parse::Parser;
use syn::visit::Visit;
use syn::{Attribute, Expr, Lit, LitStr, Meta, UseTree};

use crate::{AlphaZetaProofErrorV1, Digest, Text, TrustedItem};

#[cfg(target_os = "linux")]
#[path = "alpha_zeta_snapshot_linux.rs"]
mod snapshot_linux;
#[cfg(target_os = "linux")]
use snapshot_linux::{SnapshotFilesystem, SnapshotLease};

#[cfg(not(target_os = "linux"))]
mod snapshot_unsupported {
    use super::*;

    #[derive(Debug)]
    pub(super) struct SnapshotLease;

    impl SnapshotLease {
        pub(super) fn generation_identity(&self) -> Digest {
            unreachable!("snapshot discovery is unavailable on this platform")
        }

        pub(super) fn revalidate(&self) -> Result<(), AlphaZetaProofErrorV1> {
            Err(manifest_structure(
                ".",
                "alpha/zeta discovery requires Linux openat2",
            ))
        }
    }

    pub(super) struct SnapshotFilesystem;

    impl SnapshotFilesystem {
        pub(super) fn open(_root: &Path) -> Result<Self, AlphaZetaProofErrorV1> {
            Err(manifest_structure(
                ".",
                "alpha/zeta discovery requires Linux openat2",
            ))
        }

        pub(super) fn read_file(&mut self, _path: &str) -> Result<Vec<u8>, AlphaZetaProofErrorV1> {
            unreachable!("snapshot discovery is unavailable on this platform")
        }

        pub(super) fn regular_file_exists(
            &mut self,
            _path: &str,
        ) -> Result<bool, AlphaZetaProofErrorV1> {
            unreachable!("snapshot discovery is unavailable on this platform")
        }

        pub(super) fn finish(self) -> Result<SnapshotLease, AlphaZetaProofErrorV1> {
            unreachable!("snapshot discovery is unavailable on this platform")
        }
    }
}
#[cfg(not(target_os = "linux"))]
use snapshot_unsupported::{SnapshotFilesystem, SnapshotLease};

pub const MAX_GFX942_ALPHA_ZETA_SOURCE_FILES_V1: usize = 64;
pub const MAX_GFX942_ALPHA_ZETA_DEPENDENCY_EDGES_V1: usize = 128;
pub const MAX_GFX942_ALPHA_ZETA_SOURCE_BYTES_V1: u64 = 1024 * 1024;
pub const MAX_GFX942_ALPHA_ZETA_SOURCE_TREE_BYTES_V1: u64 = 8 * 1024 * 1024;
pub const MAX_GFX942_ALPHA_ZETA_TRUSTED_CONSTRUCTS_V1: usize = 128;

pub const ALPHA_ZETA_WORKSPACE_MANIFEST_PATH_V1: &str = "Cargo.toml";
pub const ALPHA_ZETA_LOCKFILE_PATH_V1: &str = "Cargo.lock";
pub const ALPHA_ZETA_TOOLCHAIN_PATH_V1: &str = "rust-toolchain.toml";
pub const ALPHA_ZETA_PACKAGE_MANIFEST_PATH_V1: &str = "examples/verus_vecadd/Cargo.toml";
pub const ALPHA_ZETA_RUST_MODEL_PATH_V1: &str = "examples/verus_vecadd/src/lib.rs";
pub const ALPHA_ZETA_SHARED_BODY_PATH_V1: &str = "examples/verus_vecadd/src/two_kernel_bodies.rs";
pub const ALPHA_ZETA_PERMISSION_MODEL_PATH_V1: &str =
    "examples/verus_vecadd/verus/permission_core.rs";
pub const ALPHA_ZETA_PROOF_HARNESS_PATH_V1: &str = "examples/verus_vecadd/verus/two_kernel.rs";

const CONTRACTS_MANIFEST_PATH: &str = "crates/fe2o3-contracts/Cargo.toml";
const SOURCE_TREE_DOMAIN: &[u8; 8] = b"FE2AZST\0";
const DEPENDENCY_TREE_DOMAIN: &[u8; 8] = b"FE2AZDT\0";
const TRUSTED_INVENTORY_DOMAIN: &[u8; 8] = b"FE2AZTI\0";
const MANIFEST_VERSION: u16 = 3;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum AlphaZetaSourceRoleV1 {
    WorkspaceManifest,
    PackageManifest,
    DependencyManifest,
    Lockfile,
    ToolchainConfiguration,
    CargoConfiguration,
    RustModel,
    SharedRustSource,
    ProofHarness,
    PermissionModel,
    ContractSource,
}

impl AlphaZetaSourceRoleV1 {
    const fn tag(self) -> u8 {
        match self {
            Self::WorkspaceManifest => 1,
            Self::PackageManifest => 2,
            Self::DependencyManifest => 3,
            Self::Lockfile => 4,
            Self::ToolchainConfiguration => 5,
            Self::CargoConfiguration => 6,
            Self::RustModel => 7,
            Self::SharedRustSource => 8,
            Self::ProofHarness => 9,
            Self::PermissionModel => 10,
            Self::ContractSource => 11,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum AlphaZetaDependencyKindV1 {
    WorkspaceInput,
    CargoTarget,
    CargoDependency,
    RustInclude,
    RustModule,
    RustPathModule,
}

impl AlphaZetaDependencyKindV1 {
    const fn tag(self) -> u8 {
        match self {
            Self::WorkspaceInput => 1,
            Self::CargoTarget => 2,
            Self::CargoDependency => 3,
            Self::RustInclude => 4,
            Self::RustModule => 5,
            Self::RustPathModule => 6,
        }
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct AlphaZetaSourceFileIdentityV1 {
    role: AlphaZetaSourceRoleV1,
    path: Text,
    byte_len: u64,
    digest: Digest,
    snapshot: Vec<u8>,
}

impl AlphaZetaSourceFileIdentityV1 {
    pub fn measure(
        role: AlphaZetaSourceRoleV1,
        path: &str,
        bytes: &[u8],
    ) -> Result<Self, AlphaZetaProofErrorV1> {
        if bytes.is_empty() || bytes.len() as u64 > MAX_GFX942_ALPHA_ZETA_SOURCE_BYTES_V1 {
            return Err(AlphaZetaProofErrorV1::SourceLengthOutOfRange {
                max: MAX_GFX942_ALPHA_ZETA_SOURCE_BYTES_V1,
            });
        }
        Ok(Self {
            role,
            path: Text::new("alpha/zeta source path", path)
                .map_err(AlphaZetaProofErrorV1::Model)?,
            byte_len: bytes.len() as u64,
            digest: sha256(bytes),
            snapshot: bytes.to_vec(),
        })
    }

    pub const fn role(&self) -> AlphaZetaSourceRoleV1 {
        self.role
    }

    pub const fn path(&self) -> &Text {
        &self.path
    }

    pub const fn byte_len(&self) -> u64 {
        self.byte_len
    }

    pub const fn digest(&self) -> Digest {
        self.digest
    }

    /// Immutable bytes retained from the one discovery read.
    pub fn snapshot_bytes(&self) -> &[u8] {
        &self.snapshot
    }

    pub fn matches(&self, path: &str, bytes: &[u8]) -> bool {
        self.path.as_str() == path
            && self.byte_len == bytes.len() as u64
            && self.digest == sha256(bytes)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum AlphaZetaTrustedConstructKindV1 {
    ExternalBody,
    Assume,
    Admit,
    TrustedAttribute,
    TrustedImport,
}

impl AlphaZetaTrustedConstructKindV1 {
    const fn tag(self) -> u8 {
        match self {
            Self::ExternalBody => 1,
            Self::Assume => 2,
            Self::Admit => 3,
            Self::TrustedAttribute => 4,
            Self::TrustedImport => 5,
        }
    }

    const fn name(self) -> &'static str {
        match self {
            Self::ExternalBody => "external_body",
            Self::Assume => "assume",
            Self::Admit => "admit",
            Self::TrustedAttribute => "trusted_attribute",
            Self::TrustedImport => "trusted_import",
        }
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct AlphaZetaTrustedConstructV1 {
    source_path: Text,
    kind: AlphaZetaTrustedConstructKindV1,
    token_index: u32,
    identity: Digest,
}

impl AlphaZetaTrustedConstructV1 {
    pub const fn source_path(&self) -> &Text {
        &self.source_path
    }

    pub const fn kind(&self) -> AlphaZetaTrustedConstructKindV1 {
        self.kind
    }

    pub const fn token_index(&self) -> u32 {
        self.token_index
    }

    pub const fn identity(&self) -> Digest {
        self.identity
    }
}

/// Trusted syntax derived from immutable proof-source tokens.
///
/// `unmeasured_imports` records external Verus library APIs whose source and
/// runtime closure is not part of this bounded project snapshot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AlphaZetaTrustedInventoryV1 {
    constructs: Vec<AlphaZetaTrustedConstructV1>,
    trusted_items: Vec<TrustedItem>,
    unmeasured_imports: Vec<Text>,
    identity: Digest,
}

impl AlphaZetaTrustedInventoryV1 {
    pub fn constructs(&self) -> &[AlphaZetaTrustedConstructV1] {
        &self.constructs
    }

    pub fn trusted_items(&self) -> &[TrustedItem] {
        &self.trusted_items
    }

    pub fn unmeasured_imports(&self) -> &[Text] {
        &self.unmeasured_imports
    }

    pub const fn identity(&self) -> Digest {
        self.identity
    }

    pub const fn has_complete_verifier_runtime_closure(&self) -> bool {
        false
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct AlphaZetaDependencyEdgeV1 {
    parent: Text,
    child: Text,
    kind: AlphaZetaDependencyKindV1,
}

impl AlphaZetaDependencyEdgeV1 {
    pub const fn parent(&self) -> &Text {
        &self.parent
    }

    pub const fn child(&self) -> &Text {
        &self.child
    }

    pub const fn kind(&self) -> AlphaZetaDependencyKindV1 {
        self.kind
    }
}

#[derive(Clone, Debug)]
pub struct AlphaZetaProofSourcesV1 {
    files: Vec<AlphaZetaSourceFileIdentityV1>,
    edges: Vec<AlphaZetaDependencyEdgeV1>,
    source_tree_identity: Digest,
    dependency_tree_identity: Digest,
    trusted_inventory: AlphaZetaTrustedInventoryV1,
    snapshot_lease: Arc<SnapshotLease>,
}

impl PartialEq for AlphaZetaProofSourcesV1 {
    fn eq(&self, other: &Self) -> bool {
        self.files == other.files
            && self.edges == other.edges
            && self.source_tree_identity == other.source_tree_identity
            && self.dependency_tree_identity == other.dependency_tree_identity
            && self.trusted_inventory == other.trusted_inventory
    }
}

impl Eq for AlphaZetaProofSourcesV1 {}

impl AlphaZetaProofSourcesV1 {
    /// Discovers a bounded project-input snapshot from structural Rust and
    /// Cargo inputs rooted at `workspace_root`.
    ///
    /// This is not the complete compiler or Verus runtime closure.
    pub fn discover_workspace(
        workspace_root: impl AsRef<Path>,
    ) -> Result<Self, AlphaZetaProofErrorV1> {
        Discovery::new(workspace_root.as_ref())?.discover()
    }

    pub fn files(&self) -> &[AlphaZetaSourceFileIdentityV1] {
        &self.files
    }

    pub fn edges(&self) -> &[AlphaZetaDependencyEdgeV1] {
        &self.edges
    }

    pub const fn source_tree_identity(&self) -> Digest {
        self.source_tree_identity
    }

    pub const fn dependency_tree_identity(&self) -> Digest {
        self.dependency_tree_identity
    }

    pub const fn trusted_inventory(&self) -> &AlphaZetaTrustedInventoryV1 {
        &self.trusted_inventory
    }

    /// Environmental identity for the retained discovery descriptors.
    ///
    /// It is deliberately excluded from proof-input canonical identities.
    pub fn snapshot_generation_identity(&self) -> Digest {
        self.snapshot_lease.generation_identity()
    }

    pub fn validate_snapshot_lease(&self) -> Result<(), AlphaZetaProofErrorV1> {
        self.snapshot_lease.revalidate()
    }

    pub const fn recorder_consumes_source_snapshot(&self) -> bool {
        false
    }

    pub const fn has_complete_source_closure(&self) -> bool {
        false
    }

    pub const fn has_complete_verifier_runtime_closure(&self) -> bool {
        false
    }

    pub fn validate_workspace(
        &self,
        workspace_root: impl AsRef<Path>,
    ) -> Result<(), AlphaZetaProofErrorV1> {
        self.validate_snapshot_lease()?;
        let actual = Self::discover_workspace(workspace_root)?;
        if self == &actual {
            Ok(())
        } else {
            Err(AlphaZetaProofErrorV1::SourceManifestMutation)
        }
    }

    pub fn validate_declared_files(
        &self,
        declared: &[AlphaZetaSourceFileIdentityV1],
    ) -> Result<(), AlphaZetaProofErrorV1> {
        let mut declared = declared.to_vec();
        declared.sort_unstable_by(|left, right| left.path.cmp(&right.path));
        if declared.len() != self.files.len() {
            return Err(AlphaZetaProofErrorV1::IncompleteSourceSet);
        }
        for (expected, actual) in self.files.iter().zip(&declared) {
            if expected.path != actual.path {
                return Err(AlphaZetaProofErrorV1::SourceManifestMutation);
            }
            if expected.role != actual.role {
                return Err(AlphaZetaProofErrorV1::SourceRoleSubstitution);
            }
            if expected != actual {
                return Err(AlphaZetaProofErrorV1::SourceMutation);
            }
        }
        Ok(())
    }

    pub fn validate_file(&self, path: &str, bytes: &[u8]) -> Result<(), AlphaZetaProofErrorV1> {
        let file = self
            .files
            .iter()
            .find(|file| file.path.as_str() == path)
            .ok_or(AlphaZetaProofErrorV1::UnexpectedSourcePath)?;
        if file.matches(path, bytes) {
            Ok(())
        } else {
            Err(AlphaZetaProofErrorV1::SourceMutation)
        }
    }

    pub fn dependency_bindings(&self) -> Vec<(String, Digest)> {
        self.files
            .iter()
            .enumerate()
            .map(|(index, file)| (format!("closure-{index:03}"), file.digest))
            .collect()
    }

    pub fn to_canonical_manifest_bytes(&self) -> Vec<u8> {
        canonical_manifest_bytes(&self.files, &self.edges)
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum RustSyntaxContext {
    Items,
    Expression,
    Statements,
    Pattern,
    Type,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct ModuleSearchContext {
    default_dir: PathBuf,
    path_attr_dir: PathBuf,
    inside_inline_module: bool,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct RustVisitKey {
    path: String,
    syntax_context: RustSyntaxContext,
    module_context: ModuleSearchContext,
}

struct Discovery {
    snapshot: SnapshotFilesystem,
    files: BTreeMap<String, AlphaZetaSourceFileIdentityV1>,
    edges: BTreeSet<AlphaZetaDependencyEdgeV1>,
    rust_visited: BTreeSet<RustVisitKey>,
    package_visited: BTreeSet<String>,
    total_bytes: u64,
    workspace_manifest: toml::Table,
}

struct SourceClosureWalker<'a> {
    discovery: &'a mut Discovery,
    owner: String,
    module_context: ModuleSearchContext,
    block_depth: usize,
    error: Option<AlphaZetaProofErrorV1>,
}

impl<'a> SourceClosureWalker<'a> {
    fn new(discovery: &'a mut Discovery, owner: &str, module_context: ModuleSearchContext) -> Self {
        Self {
            discovery,
            owner: owner.to_owned(),
            module_context,
            block_depth: 0,
            error: None,
        }
    }

    fn finish(self) -> Result<(), AlphaZetaProofErrorV1> {
        self.error.map_or(Ok(()), Err)
    }

    fn process_macro(&mut self, mac: &syn::Macro, syntax_context: Option<RustSyntaxContext>) {
        if self.error.is_some() {
            return;
        }
        if mac.path.is_ident("include") {
            let Some(syntax_context) = syntax_context else {
                self.error = Some(manifest_structure(
                    &self.owner,
                    "include! appears in an unsupported structural context",
                ));
                return;
            };
            let included = match syn::parse2::<LitStr>(mac.tokens.clone()) {
                Ok(included) => included,
                Err(_) => {
                    self.error = Some(manifest_structure(
                        &self.owner,
                        "include! path is not one literal string",
                    ));
                    return;
                }
            };
            if let Err(error) = self.discovery.discover_include(
                &self.owner,
                &included,
                syntax_context,
                self.module_context.clone(),
            ) {
                self.error = Some(error);
            }
            return;
        }
        if mac
            .path
            .segments
            .last()
            .is_some_and(|segment| segment.ident == "include")
            || opaque_tokens_contain_include(mac.tokens.clone())
        {
            self.error = Some(manifest_structure(
                &self.owner,
                "include! inside a qualified or opaque macro is not structurally incorporated",
            ));
        }
    }
}

impl<'ast> Visit<'ast> for SourceClosureWalker<'_> {
    fn visit_item_mod(&mut self, item: &'ast syn::ItemMod) {
        if self.error.is_some() {
            return;
        }
        if let Err(error) = self.discovery.discover_module(
            &self.owner,
            &self.module_context,
            self.block_depth,
            item,
        ) {
            self.error = Some(error);
        }
    }

    fn visit_block(&mut self, block: &'ast syn::Block) {
        self.block_depth += 1;
        syn::visit::visit_block(self, block);
        self.block_depth -= 1;
    }

    fn visit_item_macro(&mut self, item: &'ast syn::ItemMacro) {
        self.process_macro(&item.mac, Some(RustSyntaxContext::Items));
    }

    fn visit_expr_macro(&mut self, expression: &'ast syn::ExprMacro) {
        self.process_macro(&expression.mac, Some(RustSyntaxContext::Expression));
    }

    fn visit_stmt_macro(&mut self, statement: &'ast syn::StmtMacro) {
        self.process_macro(&statement.mac, Some(RustSyntaxContext::Statements));
    }

    fn visit_pat(&mut self, pattern: &'ast syn::Pat) {
        if let syn::Pat::Macro(pattern) = pattern {
            self.process_macro(&pattern.mac, Some(RustSyntaxContext::Pattern));
        } else {
            syn::visit::visit_pat(self, pattern);
        }
    }

    fn visit_type_macro(&mut self, ty: &'ast syn::TypeMacro) {
        self.process_macro(&ty.mac, Some(RustSyntaxContext::Type));
    }

    fn visit_macro(&mut self, mac: &'ast syn::Macro) {
        self.process_macro(mac, None);
    }
}

impl Discovery {
    fn new(root: &Path) -> Result<Self, AlphaZetaProofErrorV1> {
        let mut snapshot = SnapshotFilesystem::open(root)?;
        let workspace_bytes = snapshot.read_file(ALPHA_ZETA_WORKSPACE_MANIFEST_PATH_V1)?;
        let workspace_manifest =
            parse_toml(ALPHA_ZETA_WORKSPACE_MANIFEST_PATH_V1, &workspace_bytes)?;
        Ok(Self {
            snapshot,
            files: BTreeMap::new(),
            edges: BTreeSet::new(),
            rust_visited: BTreeSet::new(),
            package_visited: BTreeSet::new(),
            total_bytes: 0,
            workspace_manifest,
        })
    }

    fn discover(mut self) -> Result<AlphaZetaProofSourcesV1, AlphaZetaProofErrorV1> {
        for path in [
            ALPHA_ZETA_WORKSPACE_MANIFEST_PATH_V1,
            ALPHA_ZETA_LOCKFILE_PATH_V1,
            ALPHA_ZETA_TOOLCHAIN_PATH_V1,
        ] {
            self.add_file(path)?;
        }
        for path in [".cargo/config.toml", ".cargo/config"] {
            if self.snapshot.regular_file_exists(path)? {
                self.add_file(path)?;
                self.add_edge(
                    ALPHA_ZETA_WORKSPACE_MANIFEST_PATH_V1,
                    path,
                    AlphaZetaDependencyKindV1::WorkspaceInput,
                )?;
            }
        }
        for path in [ALPHA_ZETA_LOCKFILE_PATH_V1, ALPHA_ZETA_TOOLCHAIN_PATH_V1] {
            self.add_edge(
                ALPHA_ZETA_WORKSPACE_MANIFEST_PATH_V1,
                path,
                AlphaZetaDependencyKindV1::WorkspaceInput,
            )?;
        }

        self.discover_package(ALPHA_ZETA_PACKAGE_MANIFEST_PATH_V1)?;
        self.discover_rust_root(ALPHA_ZETA_PROOF_HARNESS_PATH_V1)?;
        self.add_edge(
            ALPHA_ZETA_PACKAGE_MANIFEST_PATH_V1,
            ALPHA_ZETA_PROOF_HARNESS_PATH_V1,
            AlphaZetaDependencyKindV1::CargoTarget,
        )?;

        let files = self.files.into_values().collect::<Vec<_>>();
        let edges = self.edges.into_iter().collect::<Vec<_>>();
        if files.len() > MAX_GFX942_ALPHA_ZETA_SOURCE_FILES_V1
            || edges.len() > MAX_GFX942_ALPHA_ZETA_DEPENDENCY_EDGES_V1
        {
            return Err(AlphaZetaProofErrorV1::SourceManifestCapacity);
        }
        let source_tree_identity = source_tree_identity(&files);
        let dependency_tree_identity = dependency_tree_identity(&files, &edges);
        let trusted_inventory = trusted_inventory(&files, &edges)?;
        let snapshot_lease = Arc::new(self.snapshot.finish()?);
        Ok(AlphaZetaProofSourcesV1 {
            files,
            edges,
            source_tree_identity,
            dependency_tree_identity,
            trusted_inventory,
            snapshot_lease,
        })
    }

    fn discover_package(&mut self, manifest_path: &str) -> Result<(), AlphaZetaProofErrorV1> {
        if !self.package_visited.insert(manifest_path.to_owned()) {
            return Ok(());
        }
        let bytes = self.add_file(manifest_path)?;
        let manifest = parse_toml(manifest_path, &bytes)?;
        let manifest_dir = Path::new(manifest_path).parent().unwrap_or(Path::new(""));
        let lib_relative = manifest
            .get("lib")
            .and_then(|value| value.get("path"))
            .and_then(toml::Value::as_str)
            .unwrap_or("src/lib.rs");
        let lib_path = normalize_relative(manifest_dir.join(lib_relative))?;
        self.discover_rust_root(&lib_path)?;
        self.add_edge(
            manifest_path,
            &lib_path,
            AlphaZetaDependencyKindV1::CargoTarget,
        )?;

        let Some(dependencies) = manifest.get("dependencies").and_then(toml::Value::as_table)
        else {
            return Ok(());
        };
        for (name, specification) in dependencies {
            let dependency_path = if specification
                .get("workspace")
                .and_then(toml::Value::as_bool)
                == Some(true)
            {
                self.workspace_manifest
                    .get("workspace")
                    .and_then(|value| value.get("dependencies"))
                    .and_then(|value| value.get(name))
                    .and_then(|value| value.get("path"))
                    .and_then(toml::Value::as_str)
                    .map(PathBuf::from)
            } else {
                specification
                    .get("path")
                    .and_then(toml::Value::as_str)
                    .map(|path| manifest_dir.join(path))
            };
            let Some(dependency_path) = dependency_path else {
                continue;
            };
            let dependency_manifest = normalize_relative(dependency_path.join("Cargo.toml"))?;
            self.add_edge(
                manifest_path,
                &dependency_manifest,
                AlphaZetaDependencyKindV1::CargoDependency,
            )?;
            self.discover_package(&dependency_manifest)?;
        }
        Ok(())
    }

    fn discover_rust_root(&mut self, path: &str) -> Result<(), AlphaZetaProofErrorV1> {
        let parent = Path::new(path)
            .parent()
            .unwrap_or_else(|| Path::new(""))
            .to_path_buf();
        self.discover_rust_source(
            path,
            RustSyntaxContext::Items,
            ModuleSearchContext {
                default_dir: parent.clone(),
                path_attr_dir: parent,
                inside_inline_module: false,
            },
        )
    }

    fn discover_rust_source(
        &mut self,
        path: &str,
        syntax_context: RustSyntaxContext,
        module_context: ModuleSearchContext,
    ) -> Result<(), AlphaZetaProofErrorV1> {
        let visit = RustVisitKey {
            path: path.to_owned(),
            syntax_context,
            module_context: module_context.clone(),
        };
        if !self.rust_visited.insert(visit) {
            return Ok(());
        }
        let bytes = self.add_file(path)?;
        let source = std::str::from_utf8(&bytes)
            .map_err(|_| manifest_structure(path, "Rust source is not UTF-8"))?;
        self.walk_rust_source(path, source, syntax_context, module_context)
    }

    fn walk_rust_source(
        &mut self,
        owner: &str,
        source: &str,
        syntax_context: RustSyntaxContext,
        module_context: ModuleSearchContext,
    ) -> Result<(), AlphaZetaProofErrorV1> {
        let tokens = TokenStream::from_str(source)
            .map_err(|_| manifest_structure(owner, "Rust tokenization failed"))?;
        let mut walker = SourceClosureWalker::new(self, owner, module_context);
        match syntax_context {
            RustSyntaxContext::Items => {
                let syntax = syn::parse2::<syn::File>(tokens)
                    .map_err(|_| manifest_structure(owner, "Rust item source did not parse"))?;
                reject_configured_file_attributes(owner, &syntax.attrs)?;
                walker.visit_file(&syntax);
            }
            RustSyntaxContext::Expression => {
                let syntax = syn::parse2::<syn::Expr>(tokens)
                    .map_err(|_| manifest_structure(owner, "included expression did not parse"))?;
                walker.visit_expr(&syntax);
            }
            RustSyntaxContext::Statements => {
                let wrapped = TokenStream::from_str(&format!("{{{source}}}")).map_err(|_| {
                    manifest_structure(owner, "included statements did not tokenize")
                })?;
                let syntax = syn::parse2::<syn::Block>(wrapped)
                    .map_err(|_| manifest_structure(owner, "included statements did not parse"))?;
                walker.visit_block(&syntax);
            }
            RustSyntaxContext::Pattern => {
                let syntax = syn::Pat::parse_single
                    .parse2(tokens)
                    .map_err(|_| manifest_structure(owner, "included pattern did not parse"))?;
                walker.visit_pat(&syntax);
            }
            RustSyntaxContext::Type => {
                let syntax = syn::parse2::<syn::Type>(tokens)
                    .map_err(|_| manifest_structure(owner, "included type did not parse"))?;
                walker.visit_type(&syntax);
            }
        }
        walker.finish()
    }

    fn discover_include(
        &mut self,
        owner: &str,
        included: &LitStr,
        syntax_context: RustSyntaxContext,
        module_context: ModuleSearchContext,
    ) -> Result<(), AlphaZetaProofErrorV1> {
        let parent = Path::new(owner).parent().unwrap_or_else(|| Path::new(""));
        let child = normalize_relative(parent.join(included.value()))?;
        self.add_edge(owner, &child, AlphaZetaDependencyKindV1::RustInclude)?;
        self.discover_rust_source(&child, syntax_context, module_context)
    }

    fn discover_module(
        &mut self,
        owner: &str,
        module_context: &ModuleSearchContext,
        block_depth: usize,
        item: &syn::ItemMod,
    ) -> Result<(), AlphaZetaProofErrorV1> {
        if block_depth != 0 {
            return Err(manifest_structure(
                owner,
                "module declarations inside block expressions are not supported",
            ));
        }
        let attributes = module_attributes(owner, &item.attrs)?;
        if !attributes.enabled {
            return Ok(());
        }
        let ident = item.ident.to_string();
        if let Some((_, nested)) = &item.content {
            let directory = if let Some(explicit) = attributes.path {
                module_context.path_attr_dir.join(explicit)
            } else {
                module_context.default_dir.join(&ident)
            };
            let nested_context = ModuleSearchContext {
                default_dir: directory.clone(),
                path_attr_dir: directory,
                inside_inline_module: true,
            };
            let mut walker = SourceClosureWalker::new(self, owner, nested_context);
            for nested_item in nested {
                walker.visit_item(nested_item);
            }
            return walker.finish();
        }

        let (child, kind) = if let Some(explicit) = attributes.path {
            (
                normalize_relative(module_context.path_attr_dir.join(explicit))?,
                AlphaZetaDependencyKindV1::RustPathModule,
            )
        } else {
            let direct =
                normalize_relative(module_context.default_dir.join(format!("{ident}.rs")))?;
            let nested =
                normalize_relative(module_context.default_dir.join(&ident).join("mod.rs"))?;
            match (
                self.snapshot.regular_file_exists(&direct)?,
                self.snapshot.regular_file_exists(&nested)?,
            ) {
                (true, false) => (direct, AlphaZetaDependencyKindV1::RustModule),
                (false, true) => (nested, AlphaZetaDependencyKindV1::RustModule),
                _ => {
                    return Err(manifest_structure(
                        owner,
                        "module path is missing or ambiguous",
                    ));
                }
            }
        };
        let source_parent = Path::new(&child)
            .parent()
            .unwrap_or_else(|| Path::new(""))
            .to_path_buf();
        let is_mod_rs = Path::new(&child)
            .file_name()
            .is_some_and(|name| name == "mod.rs");
        let default_dir = if is_mod_rs {
            source_parent.clone()
        } else {
            source_parent.join(&ident)
        };
        self.add_edge(owner, &child, kind)?;
        self.discover_rust_source(
            &child,
            RustSyntaxContext::Items,
            ModuleSearchContext {
                default_dir,
                path_attr_dir: source_parent,
                inside_inline_module: false,
            },
        )
    }

    fn add_file(&mut self, path: &str) -> Result<Vec<u8>, AlphaZetaProofErrorV1> {
        if let Some(existing) = self.files.get(path) {
            return Ok(existing.snapshot.clone());
        }
        let bytes = self.snapshot.read_file(path)?;
        let role = role_for_path(path)?;
        let measured = AlphaZetaSourceFileIdentityV1::measure(role, path, &bytes)?;
        if self.files.len() >= MAX_GFX942_ALPHA_ZETA_SOURCE_FILES_V1 {
            return Err(AlphaZetaProofErrorV1::SourceManifestCapacity);
        }
        self.total_bytes = self
            .total_bytes
            .checked_add(measured.byte_len)
            .ok_or(AlphaZetaProofErrorV1::SourceManifestCapacity)?;
        if self.total_bytes > MAX_GFX942_ALPHA_ZETA_SOURCE_TREE_BYTES_V1 {
            return Err(AlphaZetaProofErrorV1::SourceManifestCapacity);
        }
        self.files.insert(path.to_owned(), measured);
        Ok(bytes)
    }

    fn add_edge(
        &mut self,
        parent: &str,
        child: &str,
        kind: AlphaZetaDependencyKindV1,
    ) -> Result<(), AlphaZetaProofErrorV1> {
        if self.edges.len() >= MAX_GFX942_ALPHA_ZETA_DEPENDENCY_EDGES_V1 {
            return Err(AlphaZetaProofErrorV1::SourceManifestCapacity);
        }
        self.edges.insert(AlphaZetaDependencyEdgeV1 {
            parent: Text::new("dependency parent", parent).map_err(AlphaZetaProofErrorV1::Model)?,
            child: Text::new("dependency child", child).map_err(AlphaZetaProofErrorV1::Model)?,
            kind,
        });
        Ok(())
    }
}

struct ModuleAttributes {
    enabled: bool,
    path: Option<String>,
}

fn module_attributes(
    owner: &str,
    attributes: &[Attribute],
) -> Result<ModuleAttributes, AlphaZetaProofErrorV1> {
    let mut enabled = true;
    let mut path = None;
    for attribute in attributes {
        if attribute.path().is_ident("cfg_attr") {
            return Err(manifest_structure(
                owner,
                "cfg_attr on a module is not evaluated by this source snapshot",
            ));
        }
        if attribute.path().is_ident("cfg") {
            let Meta::List(list) = &attribute.meta else {
                return Err(manifest_structure(
                    owner,
                    "module cfg attribute is malformed",
                ));
            };
            let predicate = syn::parse2::<syn::Path>(list.tokens.clone()).map_err(|_| {
                manifest_structure(
                    owner,
                    "only the pinned disabled cfg(test) module predicate is supported",
                )
            })?;
            if !predicate.is_ident("test") {
                return Err(manifest_structure(
                    owner,
                    "module cfg predicate is outside the pinned discovery environment",
                ));
            }
            enabled = false;
            continue;
        }
        if !attribute.path().is_ident("path") {
            continue;
        }
        if path.is_some() {
            return Err(manifest_structure(
                owner,
                "module has multiple path attributes",
            ));
        }
        let Meta::NameValue(name_value) = &attribute.meta else {
            return Err(manifest_structure(owner, "path attribute is malformed"));
        };
        let Expr::Lit(expression) = &name_value.value else {
            return Err(manifest_structure(owner, "path attribute is not literal"));
        };
        let Lit::Str(value) = &expression.lit else {
            return Err(manifest_structure(owner, "path attribute is not a string"));
        };
        path = Some(value.value());
    }
    Ok(ModuleAttributes { enabled, path })
}

fn reject_configured_file_attributes(
    owner: &str,
    attributes: &[Attribute],
) -> Result<(), AlphaZetaProofErrorV1> {
    for attribute in attributes {
        if attribute.path().is_ident("cfg") || attribute.path().is_ident("cfg_attr") {
            return Err(manifest_structure(
                owner,
                "file-level cfg and cfg_attr are outside the pinned discovery environment",
            ));
        }
    }
    Ok(())
}

fn role_for_path(path: &str) -> Result<AlphaZetaSourceRoleV1, AlphaZetaProofErrorV1> {
    let role = match path {
        ALPHA_ZETA_WORKSPACE_MANIFEST_PATH_V1 => AlphaZetaSourceRoleV1::WorkspaceManifest,
        ALPHA_ZETA_PACKAGE_MANIFEST_PATH_V1 => AlphaZetaSourceRoleV1::PackageManifest,
        CONTRACTS_MANIFEST_PATH => AlphaZetaSourceRoleV1::DependencyManifest,
        ALPHA_ZETA_LOCKFILE_PATH_V1 => AlphaZetaSourceRoleV1::Lockfile,
        ALPHA_ZETA_TOOLCHAIN_PATH_V1 => AlphaZetaSourceRoleV1::ToolchainConfiguration,
        ".cargo/config.toml" | ".cargo/config" => AlphaZetaSourceRoleV1::CargoConfiguration,
        ALPHA_ZETA_RUST_MODEL_PATH_V1 => AlphaZetaSourceRoleV1::RustModel,
        ALPHA_ZETA_PROOF_HARNESS_PATH_V1 => AlphaZetaSourceRoleV1::ProofHarness,
        ALPHA_ZETA_PERMISSION_MODEL_PATH_V1 => AlphaZetaSourceRoleV1::PermissionModel,
        path if path.starts_with("crates/fe2o3-contracts/src/") && path.ends_with(".rs") => {
            AlphaZetaSourceRoleV1::ContractSource
        }
        path if path.starts_with("examples/verus_vecadd/src/") && path.ends_with(".rs") => {
            AlphaZetaSourceRoleV1::SharedRustSource
        }
        _ => return Err(AlphaZetaProofErrorV1::UnexpectedSourcePath),
    };
    Ok(role)
}

fn trusted_inventory(
    files: &[AlphaZetaSourceFileIdentityV1],
    edges: &[AlphaZetaDependencyEdgeV1],
) -> Result<AlphaZetaTrustedInventoryV1, AlphaZetaProofErrorV1> {
    let mut reachable = BTreeSet::from([ALPHA_ZETA_PROOF_HARNESS_PATH_V1.to_owned()]);
    loop {
        let before = reachable.len();
        for edge in edges {
            if reachable.contains(edge.parent.as_str())
                && matches!(
                    edge.kind,
                    AlphaZetaDependencyKindV1::RustInclude
                        | AlphaZetaDependencyKindV1::RustModule
                        | AlphaZetaDependencyKindV1::RustPathModule
                )
            {
                reachable.insert(edge.child.as_str().to_owned());
            }
        }
        if reachable.len() == before {
            break;
        }
    }

    let mut constructs = Vec::new();
    let mut unmeasured_imports = BTreeSet::new();
    let mut structural_imports = Vec::new();
    for (file_index, file) in files.iter().enumerate() {
        if !reachable.contains(file.path.as_str()) {
            continue;
        }
        scan_trusted_tokens(file, &mut constructs)?;
        structural_imports.extend(collect_structural_imports(file_index, file)?);
    }
    inventory_structural_imports(
        files,
        &structural_imports,
        &mut constructs,
        &mut unmeasured_imports,
    )?;
    constructs.sort_unstable();
    constructs.dedup();
    if constructs.len() > MAX_GFX942_ALPHA_ZETA_TRUSTED_CONSTRUCTS_V1 {
        return Err(AlphaZetaProofErrorV1::SourceManifestCapacity);
    }
    let unmeasured_imports = unmeasured_imports.into_iter().collect::<Vec<_>>();
    let identity = trusted_inventory_identity(&constructs, &unmeasured_imports);
    let trusted_items = constructs
        .iter()
        .enumerate()
        .map(|(index, construct)| {
            TrustedItem::new(
                format!("source_{}_{index:03}", construct.kind.name()),
                construct.identity,
            )
            .map_err(AlphaZetaProofErrorV1::Model)
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(AlphaZetaTrustedInventoryV1 {
        constructs,
        trusted_items,
        unmeasured_imports,
        identity,
    })
}

fn scan_trusted_tokens(
    file: &AlphaZetaSourceFileIdentityV1,
    constructs: &mut Vec<AlphaZetaTrustedConstructV1>,
) -> Result<(), AlphaZetaProofErrorV1> {
    let source = std::str::from_utf8(file.snapshot_bytes())
        .map_err(|_| manifest_structure(file.path.as_str(), "Rust source is not UTF-8"))?;
    let stream = TokenStream::from_str(source)
        .map_err(|_| manifest_structure(file.path.as_str(), "Rust tokenization failed"))?;
    let mut tokens = Vec::new();
    flatten_tokens(stream, &mut tokens);

    let mut in_import = false;
    for (index, token) in tokens.iter().enumerate() {
        if is_punctuation(token, ';') {
            in_import = false;
            continue;
        }
        let TokenTree::Ident(identifier) = token else {
            continue;
        };
        let name = identifier.to_string();
        if in_import {
            continue;
        }
        if name == "use"
            || (name == "extern"
                && matches!(
                    tokens.get(index + 1),
                    Some(TokenTree::Ident(next)) if next == "crate"
                ))
        {
            in_import = true;
            continue;
        }
        let kind = match name.as_str() {
            "external_body" => Some(AlphaZetaTrustedConstructKindV1::ExternalBody),
            "assume" => Some(AlphaZetaTrustedConstructKindV1::Assume),
            "admit" => Some(AlphaZetaTrustedConstructKindV1::Admit),
            "trusted"
            | "axiom"
            | "external"
            | "assume_specification"
            | "external_fn_specification"
            | "external_type_specification"
            | "external_trait_specification" => {
                Some(AlphaZetaTrustedConstructKindV1::TrustedAttribute)
            }
            _ => None,
        };
        if let Some(kind) = kind {
            constructs.push(trusted_construct(file, kind, index as u32));
        }
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct StructuralImport {
    file_index: usize,
    ordinal: u32,
    leading_colon: bool,
    path: Vec<String>,
    rename: Option<String>,
    glob: bool,
    extern_crate: bool,
}

struct StructuralImportCollector<'a> {
    file: &'a AlphaZetaSourceFileIdentityV1,
    file_index: usize,
    imports: Vec<StructuralImport>,
    error: Option<AlphaZetaProofErrorV1>,
}

impl StructuralImportCollector<'_> {
    fn push_use_tree(&mut self, leading_colon: bool, prefix: &mut Vec<String>, tree: &UseTree) {
        if self.error.is_some() {
            return;
        }
        match tree {
            UseTree::Path(path) => {
                prefix.push(path.ident.to_string());
                self.push_use_tree(leading_colon, prefix, &path.tree);
                prefix.pop();
            }
            UseTree::Name(name) => {
                prefix.push(name.ident.to_string());
                self.push_leaf(leading_colon, prefix.clone(), None, false);
                prefix.pop();
            }
            UseTree::Rename(rename) => {
                prefix.push(rename.ident.to_string());
                self.push_leaf(
                    leading_colon,
                    prefix.clone(),
                    Some(rename.rename.to_string()),
                    false,
                );
                prefix.pop();
            }
            UseTree::Glob(_) => self.push_leaf(leading_colon, prefix.clone(), None, true),
            UseTree::Group(group) => {
                if group.items.is_empty() {
                    self.error = Some(manifest_structure(
                        self.file.path.as_str(),
                        "empty use group is not inventoried",
                    ));
                    return;
                }
                for tree in &group.items {
                    self.push_use_tree(leading_colon, prefix, tree);
                }
            }
        }
    }

    fn push_leaf(
        &mut self,
        leading_colon: bool,
        path: Vec<String>,
        rename: Option<String>,
        glob: bool,
    ) {
        if path.is_empty() {
            self.error = Some(manifest_structure(
                self.file.path.as_str(),
                "use tree has no structural root",
            ));
            return;
        }
        let Ok(ordinal) = u32::try_from(self.imports.len()) else {
            self.error = Some(AlphaZetaProofErrorV1::SourceManifestCapacity);
            return;
        };
        self.imports.push(StructuralImport {
            file_index: self.file_index,
            ordinal,
            leading_colon,
            path,
            rename,
            glob,
            extern_crate: false,
        });
    }
}

impl<'ast> Visit<'ast> for StructuralImportCollector<'_> {
    fn visit_item_use(&mut self, item: &'ast syn::ItemUse) {
        self.push_use_tree(item.leading_colon.is_some(), &mut Vec::new(), &item.tree);
    }

    fn visit_item_extern_crate(&mut self, item: &'ast syn::ItemExternCrate) {
        let Ok(ordinal) = u32::try_from(self.imports.len()) else {
            self.error = Some(AlphaZetaProofErrorV1::SourceManifestCapacity);
            return;
        };
        self.imports.push(StructuralImport {
            file_index: self.file_index,
            ordinal,
            leading_colon: false,
            path: vec![item.ident.to_string()],
            rename: item.rename.as_ref().map(|(_, name)| name.to_string()),
            glob: false,
            extern_crate: true,
        });
    }

    fn visit_macro(&mut self, mac: &'ast syn::Macro) {
        if opaque_tokens_contain_import(mac.tokens.clone()) {
            self.error = Some(manifest_structure(
                self.file.path.as_str(),
                "import syntax inside an opaque macro is not structurally inventoried",
            ));
        }
    }
}

fn collect_structural_imports(
    file_index: usize,
    file: &AlphaZetaSourceFileIdentityV1,
) -> Result<Vec<StructuralImport>, AlphaZetaProofErrorV1> {
    let source = std::str::from_utf8(file.snapshot_bytes())
        .map_err(|_| manifest_structure(file.path.as_str(), "Rust source is not UTF-8"))?;
    let tokens = TokenStream::from_str(source)
        .map_err(|_| manifest_structure(file.path.as_str(), "Rust tokenization failed"))?;
    let mut collector = StructuralImportCollector {
        file,
        file_index,
        imports: Vec::new(),
        error: None,
    };
    if let Ok(syntax) = syn::parse2::<syn::File>(tokens.clone()) {
        collector.visit_file(&syntax);
    } else if let Ok(syntax) = syn::parse2::<syn::Expr>(tokens.clone()) {
        collector.visit_expr(&syntax);
    } else if let Ok(syntax) = syn::parse2::<syn::Type>(tokens.clone()) {
        collector.visit_type(&syntax);
    } else if let Ok(syntax) = syn::Pat::parse_single.parse2(tokens.clone()) {
        collector.visit_pat(&syntax);
    } else {
        let wrapped = TokenStream::from_str(&format!("{{{source}}}")).map_err(|_| {
            manifest_structure(file.path.as_str(), "Rust fragment did not tokenize")
        })?;
        let syntax = syn::parse2::<syn::Block>(wrapped)
            .map_err(|_| manifest_structure(file.path.as_str(), "Rust fragment did not parse"))?;
        collector.visit_block(&syntax);
    }
    if let Some(error) = collector.error {
        Err(error)
    } else {
        Ok(collector.imports)
    }
}

fn opaque_tokens_contain_import(stream: TokenStream) -> bool {
    let mut tokens = Vec::new();
    flatten_tokens(stream, &mut tokens);
    tokens.iter().enumerate().any(|(index, token)| {
        matches!(token, TokenTree::Ident(identifier) if identifier == "use")
            || matches!(
                (token, tokens.get(index + 1)),
                (TokenTree::Ident(first), Some(TokenTree::Ident(second)))
                    if first == "extern" && second == "crate"
            )
    })
}

fn opaque_tokens_contain_include(stream: TokenStream) -> bool {
    let mut tokens = Vec::new();
    flatten_tokens(stream, &mut tokens);
    tokens.windows(2).any(|tokens| {
        matches!(
            tokens,
            [TokenTree::Ident(identifier), TokenTree::Punct(punctuation)]
                if identifier == "include" && punctuation.as_char() == '!'
        )
    })
}

fn inventory_structural_imports(
    files: &[AlphaZetaSourceFileIdentityV1],
    imports: &[StructuralImport],
    constructs: &mut Vec<AlphaZetaTrustedConstructV1>,
    unmeasured_imports: &mut BTreeSet<Text>,
) -> Result<(), AlphaZetaProofErrorV1> {
    let mut external_roots = BTreeSet::from([
        "vstd".to_owned(),
        "builtin".to_owned(),
        "builtin_macros".to_owned(),
    ]);
    loop {
        let before = external_roots.len();
        for import in imports {
            let Some(root) = import.path.first() else {
                return Err(manifest_structure("use", "use tree has no root"));
            };
            if !external_roots.contains(root) {
                continue;
            }
            if let Some(local_name) = import_local_name(import) {
                external_roots.insert(local_name.to_owned());
            }
        }
        if external_roots.len() == before {
            break;
        }
    }

    for import in imports {
        let Some(root) = import.path.first() else {
            return Err(manifest_structure("use", "use tree has no root"));
        };
        if !external_roots.contains(root) {
            continue;
        }
        let file = files
            .get(import.file_index)
            .ok_or(AlphaZetaProofErrorV1::SourceManifestMutation)?;
        unmeasured_imports.insert(
            Text::new(
                "unmeasured Verus import",
                canonical_import(file.path.as_str(), import),
            )
            .map_err(AlphaZetaProofErrorV1::Model)?,
        );
        if import
            .path
            .last()
            .is_some_and(|name| trusted_import_name(name))
        {
            constructs.push(trusted_construct(
                file,
                AlphaZetaTrustedConstructKindV1::TrustedImport,
                import.ordinal,
            ));
        }
    }
    Ok(())
}

fn import_local_name(import: &StructuralImport) -> Option<&str> {
    if let Some(rename) = &import.rename {
        return Some(rename);
    }
    if import.glob {
        return None;
    }
    import
        .path
        .last()
        .map(String::as_str)
        .filter(|name| *name != "self")
}

fn trusted_import_name(name: &str) -> bool {
    matches!(
        name,
        "external_body"
            | "assume"
            | "admit"
            | "trusted"
            | "axiom"
            | "external"
            | "assume_specification"
            | "external_fn_specification"
            | "external_type_specification"
            | "external_trait_specification"
    )
}

fn canonical_import(source_path: &str, import: &StructuralImport) -> String {
    let mut value = format!("{source_path}:");
    if import.extern_crate {
        value.push_str("extern crate ");
    } else {
        value.push_str("use ");
        if import.leading_colon {
            value.push_str("::");
        }
    }
    value.push_str(&import.path.join("::"));
    if import.glob {
        if !import.path.is_empty() {
            value.push_str("::");
        }
        value.push('*');
    }
    if let Some(rename) = &import.rename {
        value.push_str(" as ");
        value.push_str(rename);
    }
    value
}

fn flatten_tokens(stream: TokenStream, tokens: &mut Vec<TokenTree>) {
    for token in stream {
        match token {
            TokenTree::Group(group) => flatten_tokens(group.stream(), tokens),
            token => tokens.push(token),
        }
    }
}

fn is_punctuation(token: &TokenTree, expected: char) -> bool {
    matches!(token, TokenTree::Punct(value) if value.as_char() == expected)
}

fn trusted_construct(
    file: &AlphaZetaSourceFileIdentityV1,
    kind: AlphaZetaTrustedConstructKindV1,
    token_index: u32,
) -> AlphaZetaTrustedConstructV1 {
    let mut bytes = Vec::with_capacity(96);
    bytes.extend_from_slice(TRUSTED_INVENTORY_DOMAIN);
    bytes.extend_from_slice(&MANIFEST_VERSION.to_le_bytes());
    bytes.push(kind.tag());
    put_text(&mut bytes, file.path.as_str());
    bytes.extend_from_slice(&token_index.to_le_bytes());
    put_digest(&mut bytes, file.digest);
    AlphaZetaTrustedConstructV1 {
        source_path: file.path.clone(),
        kind,
        token_index,
        identity: sha256(&bytes),
    }
}

fn trusted_inventory_identity(
    constructs: &[AlphaZetaTrustedConstructV1],
    unmeasured_imports: &[Text],
) -> Digest {
    let mut bytes = Vec::with_capacity(128 + constructs.len() * 96);
    bytes.extend_from_slice(TRUSTED_INVENTORY_DOMAIN);
    bytes.extend_from_slice(&MANIFEST_VERSION.to_le_bytes());
    bytes.extend_from_slice(&(constructs.len() as u16).to_le_bytes());
    for construct in constructs {
        bytes.push(construct.kind.tag());
        put_text(&mut bytes, construct.source_path.as_str());
        bytes.extend_from_slice(&construct.token_index.to_le_bytes());
        put_digest(&mut bytes, construct.identity);
    }
    bytes.extend_from_slice(&(unmeasured_imports.len() as u16).to_le_bytes());
    for import in unmeasured_imports {
        put_text(&mut bytes, import.as_str());
    }
    sha256(&bytes)
}

fn parse_toml(path: &str, bytes: &[u8]) -> Result<toml::Table, AlphaZetaProofErrorV1> {
    let source = std::str::from_utf8(bytes)
        .map_err(|_| manifest_structure(path, "Cargo input is not UTF-8"))?;
    source
        .parse::<toml::Table>()
        .map_err(|error| manifest_structure(path, error.to_string()))
}

fn normalize_relative(path: impl AsRef<Path>) -> Result<String, AlphaZetaProofErrorV1> {
    let mut normalized = Vec::new();
    for component in path.as_ref().components() {
        match component {
            Component::CurDir => {}
            Component::Normal(value) => normalized.push(value.to_owned()),
            Component::ParentDir if normalized.pop().is_some() => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(manifest_structure("path", "path escapes workspace"));
            }
        }
    }
    if normalized.is_empty() {
        return Err(manifest_structure("path", "path is empty"));
    }
    Ok(normalized
        .iter()
        .map(|component| component.to_string_lossy())
        .collect::<Vec<_>>()
        .join("/"))
}

fn source_tree_identity(files: &[AlphaZetaSourceFileIdentityV1]) -> Digest {
    let mut bytes = Vec::with_capacity(files.len() * 128);
    bytes.extend_from_slice(SOURCE_TREE_DOMAIN);
    bytes.extend_from_slice(&MANIFEST_VERSION.to_le_bytes());
    bytes.extend_from_slice(&(files.len() as u16).to_le_bytes());
    for file in files {
        bytes.push(file.role.tag());
        put_text(&mut bytes, file.path.as_str());
        bytes.extend_from_slice(&file.byte_len.to_le_bytes());
        put_digest(&mut bytes, file.digest);
    }
    sha256(&bytes)
}

fn dependency_tree_identity(
    files: &[AlphaZetaSourceFileIdentityV1],
    edges: &[AlphaZetaDependencyEdgeV1],
) -> Digest {
    sha256(&canonical_manifest_bytes(files, edges))
}

fn canonical_manifest_bytes(
    files: &[AlphaZetaSourceFileIdentityV1],
    edges: &[AlphaZetaDependencyEdgeV1],
) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(files.len() * 128 + edges.len() * 128);
    bytes.extend_from_slice(DEPENDENCY_TREE_DOMAIN);
    bytes.extend_from_slice(&MANIFEST_VERSION.to_le_bytes());
    bytes.extend_from_slice(&(files.len() as u16).to_le_bytes());
    for file in files {
        bytes.push(file.role.tag());
        put_text(&mut bytes, file.path.as_str());
        bytes.extend_from_slice(&file.byte_len.to_le_bytes());
        put_digest(&mut bytes, file.digest);
    }
    bytes.extend_from_slice(&(edges.len() as u16).to_le_bytes());
    for edge in edges {
        bytes.push(edge.kind.tag());
        put_text(&mut bytes, edge.parent.as_str());
        put_text(&mut bytes, edge.child.as_str());
    }
    bytes
}

fn put_text(bytes: &mut Vec<u8>, value: &str) {
    bytes.extend_from_slice(&(value.len() as u16).to_le_bytes());
    bytes.extend_from_slice(value.as_bytes());
}

fn put_digest(bytes: &mut Vec<u8>, digest: Digest) {
    bytes.extend_from_slice(digest.as_bytes());
}

fn sha256(bytes: &[u8]) -> Digest {
    let digest = DigestAlgorithm::Sha256.calculate(bytes);
    Digest::from_bytes(*digest.bytes().as_bytes())
}

fn manifest_io(operation: &'static str, path: String) -> AlphaZetaProofErrorV1 {
    AlphaZetaProofErrorV1::SourceManifestIo { operation, path }
}

fn manifest_structure(path: &str, reason: impl Into<String>) -> AlphaZetaProofErrorV1 {
    AlphaZetaProofErrorV1::SourceManifestStructure {
        path: path.to_owned(),
        reason: reason.into(),
    }
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::{Arc, Barrier, Mutex};

    static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(1);
    static SNAPSHOT_RACE_TEST: Mutex<()> = Mutex::new(());

    struct SourceFixture {
        path: PathBuf,
    }

    impl SourceFixture {
        fn copy_from_workspace() -> Self {
            let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
            let sources = AlphaZetaProofSourcesV1::discover_workspace(&workspace).unwrap();
            let path = std::env::temp_dir().join(format!(
                "fe2o3-alpha-zeta-snapshot-{}-{}",
                std::process::id(),
                NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).unwrap();
            for file in sources.files() {
                let relative = Path::new(file.path().as_str());
                let destination = path.join(relative);
                fs::create_dir_all(destination.parent().unwrap()).unwrap();
                fs::copy(workspace.join(relative), destination).unwrap();
            }
            Self { path }
        }
    }

    impl Drop for SourceFixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    #[test]
    fn coherent_snapshot_rejects_concurrent_file_replacement() {
        let _serial = SNAPSHOT_RACE_TEST.lock().unwrap();
        let fixture = SourceFixture::copy_from_workspace();
        let target = fixture.path.join(ALPHA_ZETA_PROOF_HARNESS_PATH_V1);
        let replacement = target.with_extension("replacement");
        let mut bytes = fs::read(&target).unwrap();
        bytes.extend_from_slice(b"\n// concurrent replacement\n");
        fs::write(&replacement, bytes).unwrap();

        let reached = Arc::new(Barrier::new(2));
        let resume = Arc::new(Barrier::new(2));
        snapshot_linux::install_finish_pause(Arc::clone(&reached), Arc::clone(&resume));
        let worker = std::thread::spawn(move || {
            reached.wait();
            fs::rename(replacement, target).unwrap();
            resume.wait();
        });

        let result = AlphaZetaProofSourcesV1::discover_workspace(&fixture.path);
        snapshot_linux::clear_finish_pause();
        worker.join().unwrap();
        assert!(matches!(
            result,
            Err(AlphaZetaProofErrorV1::SourceSnapshotGenerationChanged)
        ));
    }

    #[test]
    fn coherent_snapshot_rejects_concurrent_parent_replacement() {
        let _serial = SNAPSHOT_RACE_TEST.lock().unwrap();
        let fixture = SourceFixture::copy_from_workspace();
        let parent = fixture.path.join("examples/verus_vecadd");
        let target = parent.join("verus");
        let retired = parent.join("verus-retired");
        let replacement = parent.join("verus-replacement");
        fs::create_dir(&replacement).unwrap();

        let reached = Arc::new(Barrier::new(2));
        let resume = Arc::new(Barrier::new(2));
        snapshot_linux::install_finish_pause(Arc::clone(&reached), Arc::clone(&resume));
        let worker = std::thread::spawn(move || {
            reached.wait();
            fs::rename(target, retired).unwrap();
            fs::rename(replacement, parent.join("verus")).unwrap();
            resume.wait();
        });

        let result = AlphaZetaProofSourcesV1::discover_workspace(&fixture.path);
        snapshot_linux::clear_finish_pause();
        worker.join().unwrap();
        assert!(matches!(
            result,
            Err(AlphaZetaProofErrorV1::SourceSnapshotGenerationChanged)
        ));
    }

    #[test]
    fn coherent_snapshot_rejects_concurrent_workspace_dirent_replacement() {
        let _serial = SNAPSHOT_RACE_TEST.lock().unwrap();
        let fixture = SourceFixture::copy_from_workspace();
        let target = fixture.path.clone();
        let retired = target.with_extension("retired");
        let replacement = target.with_extension("replacement");
        fs::create_dir(&replacement).unwrap();

        let reached = Arc::new(Barrier::new(2));
        let resume = Arc::new(Barrier::new(2));
        snapshot_linux::install_finish_pause(Arc::clone(&reached), Arc::clone(&resume));
        let retired_for_worker = retired.clone();
        let worker = std::thread::spawn(move || {
            reached.wait();
            fs::rename(&target, &retired_for_worker).unwrap();
            fs::rename(&replacement, &target).unwrap();
            resume.wait();
        });

        let result = AlphaZetaProofSourcesV1::discover_workspace(&fixture.path);
        snapshot_linux::clear_finish_pause();
        worker.join().unwrap();
        assert!(matches!(
            result,
            Err(AlphaZetaProofErrorV1::SourceSnapshotGenerationChanged)
        ));
        fs::remove_dir_all(retired).unwrap();
    }
}
