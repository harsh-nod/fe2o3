use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Component, Path, PathBuf};

use fe2o3_artifacts::DigestAlgorithm;
use syn::{Attribute, Expr, Item, Lit, LitStr, Meta};

use crate::{AlphaZetaProofErrorV1, Digest, Text};

pub const MAX_GFX942_ALPHA_ZETA_SOURCE_FILES_V1: usize = 64;
pub const MAX_GFX942_ALPHA_ZETA_DEPENDENCY_EDGES_V1: usize = 128;
pub const MAX_GFX942_ALPHA_ZETA_SOURCE_BYTES_V1: u64 = 1024 * 1024;
pub const MAX_GFX942_ALPHA_ZETA_SOURCE_TREE_BYTES_V1: u64 = 8 * 1024 * 1024;

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
const MANIFEST_VERSION: u16 = 2;

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

    pub fn matches(&self, path: &str, bytes: &[u8]) -> bool {
        self.path.as_str() == path
            && self.byte_len == bytes.len() as u64
            && self.digest == sha256(bytes)
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AlphaZetaProofSourcesV1 {
    files: Vec<AlphaZetaSourceFileIdentityV1>,
    edges: Vec<AlphaZetaDependencyEdgeV1>,
    source_tree_identity: Digest,
    dependency_tree_identity: Digest,
}

impl AlphaZetaProofSourcesV1 {
    /// Discovers the complete bounded project source closure from structural
    /// Rust and Cargo inputs rooted at `workspace_root`.
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

    pub fn validate_workspace(
        &self,
        workspace_root: impl AsRef<Path>,
    ) -> Result<(), AlphaZetaProofErrorV1> {
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

struct Discovery {
    root: PathBuf,
    files: BTreeMap<String, AlphaZetaSourceFileIdentityV1>,
    edges: BTreeSet<AlphaZetaDependencyEdgeV1>,
    rust_visited: BTreeSet<String>,
    package_visited: BTreeSet<String>,
    total_bytes: u64,
    workspace_manifest: toml::Table,
}

impl Discovery {
    fn new(root: &Path) -> Result<Self, AlphaZetaProofErrorV1> {
        let root = fs::canonicalize(root)
            .map_err(|_| manifest_io("canonicalize", root.to_string_lossy().into_owned()))?;
        if !root.is_dir() {
            return Err(manifest_io("open workspace", root.display().to_string()));
        }
        let workspace_bytes = read_bounded_file(&root, ALPHA_ZETA_WORKSPACE_MANIFEST_PATH_V1)?;
        let workspace_manifest =
            parse_toml(ALPHA_ZETA_WORKSPACE_MANIFEST_PATH_V1, &workspace_bytes)?;
        Ok(Self {
            root,
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
            let exists = self
                .root
                .join(path)
                .try_exists()
                .map_err(|_| manifest_io("inspect", path.to_owned()))?;
            if exists {
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
        self.discover_rust(ALPHA_ZETA_PROOF_HARNESS_PATH_V1)?;
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
        Ok(AlphaZetaProofSourcesV1 {
            files,
            edges,
            source_tree_identity,
            dependency_tree_identity,
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
        self.discover_rust(&lib_path)?;
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

    fn discover_rust(&mut self, path: &str) -> Result<(), AlphaZetaProofErrorV1> {
        if !self.rust_visited.insert(path.to_owned()) {
            return Ok(());
        }
        let bytes = self.add_file(path)?;
        let source = std::str::from_utf8(&bytes)
            .map_err(|_| manifest_structure(path, "Rust source is not UTF-8"))?;
        let syntax = syn::parse_file(source)
            .map_err(|_| manifest_structure(path, "Rust source did not parse"))?;
        let parent = Path::new(path).parent().unwrap_or(Path::new(""));
        self.discover_items(path, parent, &syntax.items)
    }

    fn discover_items(
        &mut self,
        owner: &str,
        parent: &Path,
        items: &[Item],
    ) -> Result<(), AlphaZetaProofErrorV1> {
        for item in items {
            match item {
                Item::Macro(item_macro) if item_macro.mac.path.is_ident("include") => {
                    let included = syn::parse2::<LitStr>(item_macro.mac.tokens.clone())
                        .map_err(|_| manifest_structure(owner, "include! path is not literal"))?;
                    let child = normalize_relative(parent.join(included.value()))?;
                    self.add_edge(owner, &child, AlphaZetaDependencyKindV1::RustInclude)?;
                    self.discover_rust(&child)?;
                }
                Item::Mod(item_mod) if !is_cfg_test(&item_mod.attrs) => {
                    if let Some((_, nested)) = &item_mod.content {
                        self.discover_items(owner, parent, nested)?;
                        continue;
                    }
                    let explicit = path_attribute(&item_mod.attrs)?;
                    let (child, kind) = if let Some(explicit) = explicit {
                        (
                            normalize_relative(parent.join(explicit))?,
                            AlphaZetaDependencyKindV1::RustPathModule,
                        )
                    } else {
                        let direct = parent.join(format!("{}.rs", item_mod.ident));
                        let nested = parent.join(item_mod.ident.to_string()).join("mod.rs");
                        match (
                            self.root.join(&direct).is_file(),
                            self.root.join(&nested).is_file(),
                        ) {
                            (true, false) => (
                                normalize_relative(direct)?,
                                AlphaZetaDependencyKindV1::RustModule,
                            ),
                            (false, true) => (
                                normalize_relative(nested)?,
                                AlphaZetaDependencyKindV1::RustModule,
                            ),
                            _ => {
                                return Err(manifest_structure(
                                    owner,
                                    "module path is missing or ambiguous",
                                ));
                            }
                        }
                    };
                    self.add_edge(owner, &child, kind)?;
                    self.discover_rust(&child)?;
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn add_file(&mut self, path: &str) -> Result<Vec<u8>, AlphaZetaProofErrorV1> {
        let bytes = read_bounded_file(&self.root, path)?;
        let role = role_for_path(path)?;
        let measured = AlphaZetaSourceFileIdentityV1::measure(role, path, &bytes)?;
        if let Some(existing) = self.files.get(path) {
            if existing != &measured {
                return Err(AlphaZetaProofErrorV1::SourceManifestMutation);
            }
            return Ok(bytes);
        }
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

fn path_attribute(attributes: &[Attribute]) -> Result<Option<String>, AlphaZetaProofErrorV1> {
    for attribute in attributes {
        if !attribute.path().is_ident("path") {
            continue;
        }
        let Meta::NameValue(name_value) = &attribute.meta else {
            return Err(manifest_structure("#[path]", "path attribute is malformed"));
        };
        let Expr::Lit(expression) = &name_value.value else {
            return Err(manifest_structure(
                "#[path]",
                "path attribute is not literal",
            ));
        };
        let Lit::Str(path) = &expression.lit else {
            return Err(manifest_structure(
                "#[path]",
                "path attribute is not a string",
            ));
        };
        return Ok(Some(path.value()));
    }
    Ok(None)
}

fn is_cfg_test(attributes: &[Attribute]) -> bool {
    attributes.iter().any(|attribute| {
        if !attribute.path().is_ident("cfg") {
            return false;
        }
        let mut is_test = false;
        let _ = attribute.parse_nested_meta(|meta| {
            if meta.path.is_ident("test") {
                is_test = true;
            }
            Ok(())
        });
        is_test
    })
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

fn read_bounded_file(root: &Path, path: &str) -> Result<Vec<u8>, AlphaZetaProofErrorV1> {
    let relative = normalize_relative(path)?;
    let full = root.join(&relative);
    let metadata =
        fs::symlink_metadata(&full).map_err(|_| manifest_io("open", relative.clone()))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(manifest_io("open regular file", relative));
    }
    if metadata.len() == 0 || metadata.len() > MAX_GFX942_ALPHA_ZETA_SOURCE_BYTES_V1 {
        return Err(AlphaZetaProofErrorV1::SourceLengthOutOfRange {
            max: MAX_GFX942_ALPHA_ZETA_SOURCE_BYTES_V1,
        });
    }
    let bytes = fs::read(&full).map_err(|_| manifest_io("read", relative))?;
    if bytes.len() as u64 != metadata.len() {
        return Err(AlphaZetaProofErrorV1::SourceManifestMutation);
    }
    Ok(bytes)
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
