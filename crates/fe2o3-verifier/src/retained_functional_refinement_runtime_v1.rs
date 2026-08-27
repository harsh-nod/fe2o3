//! Retained filesystem admission for the workload-neutral functional-refinement runtime.
//!
//! This private backend retains the exact pinned verifier runtime and executes only sealed,
//! compiler-generated proof sources. Admission does not establish a proof or grant compiler,
//! artifact, publication, or launch authority.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::{Component, Path, PathBuf};
use std::time::Instant;

use sha2::{Digest, Sha256};

use crate::CanonicalGeneratedVerusProofInputV3;

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[path = "retained_functional_refinement_runtime_v1_linux.rs"]
mod linux;

pub(crate) const FUNCTIONAL_REFINEMENT_RUNTIME_V1_MANIFEST_NAME: &str =
    "FUNCTIONAL_REFINEMENT_RUNTIME_V1.manifest";
const FUNCTIONAL_REFINEMENT_MANIFEST_BYTES: &[u8] =
    include_bytes!("../verus/pins/FUNCTIONAL_REFINEMENT_RUNTIME_V1.manifest");
const RUST_TARGET_PINS: &[u8] = include_bytes!("../verus/pins/rust_target_1_97_1.sha256");
const FUNCTIONAL_REFINEMENT_CLOSURE_IDENTITY_DOMAIN_V1: &[u8] =
    b"FE2O3/FUNCTIONAL-REFINEMENT/RETAINED-RUNTIME/V1\0";
const TARGET_PREFIX: &str = "toolchain/lib/rustlib/x86_64-unknown-linux-gnu/lib";
const MAX_MANIFEST_BYTES: usize = 64 * 1024;
const MAX_RUNTIME_FILES: usize = 128;
const MAX_RUNTIME_DIRECTORIES: usize = 32;
const MAX_RELATIVE_PATH_BYTES: usize = 512;
const MAX_TARGET_FILE_BYTES: u64 = 70 * 1024 * 1024;
/// Stable category for a runtime-closure admission failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RetainedFunctionalRefinementRuntimeErrorKindV1 {
    /// The retained closure is available only on Linux.
    #[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
    UnsupportedPlatform,
    /// A path or canonical pin record is malformed.
    InvalidManifest,
    /// A path contains a link or resolves outside its retained parent.
    SymlinkOrTraversal,
    /// A required object is absent or has the wrong filesystem type.
    ObjectType,
    /// Ownership, permissions, or hard-link count differs from policy.
    Protection,
    /// Directory membership differs from the exact manifest.
    InventoryMismatch,
    /// File length or SHA-256 differs from the reviewed pin.
    ContentMismatch,
    /// A retained object, path edge, or mutation journal changed.
    ClosureChanged,
    /// The lease was used by a process other than its admitting process.
    OwnerProcessChanged,
    /// An operating-system operation failed.
    Io,
    /// A supervised proof child exceeded its one global deadline.
    TimedOut,
    /// A supervised proof child exceeded a bounded output stream.
    OutputTooLarge,
    /// A supervised proof child could not be spawned, observed, or contained.
    Process,
}

/// Failure to admit or revalidate the retained runtime closure.
#[derive(Debug)]
pub struct RetainedFunctionalRefinementRuntimeErrorV1 {
    kind: RetainedFunctionalRefinementRuntimeErrorKindV1,
    detail: String,
}

impl RetainedFunctionalRefinementRuntimeErrorV1 {
    pub(crate) fn new(
        kind: RetainedFunctionalRefinementRuntimeErrorKindV1,
        detail: impl Into<String>,
    ) -> Self {
        Self {
            kind,
            detail: detail.into(),
        }
    }

    /// Returns the stable failure category.
    pub const fn kind(&self) -> RetainedFunctionalRefinementRuntimeErrorKindV1 {
        self.kind
    }
}

impl fmt::Display for RetainedFunctionalRefinementRuntimeErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "functional-refinement retained runtime: {}",
            self.detail
        )
    }
}

impl std::error::Error for RetainedFunctionalRefinementRuntimeErrorV1 {}
/// Bounded output from one directly executed retained `rust_verify` process.
pub(crate) struct RetainedFunctionalRefinementRuntimeOutputV1 {
    pub(crate) exit_code: Option<i32>,
    pub(crate) signal: Option<i32>,
    pub(crate) stdout: Vec<u8>,
    pub(crate) stderr: Vec<u8>,
}
fn put_blob(digest: &mut Sha256, value: &[u8]) {
    digest.update((value.len() as u64).to_le_bytes());
    digest.update(value);
}
pub(crate) struct RetainedGeneratedVerusRuntimeBackendV1 {
    root: PathBuf,
    identity: [u8; 32],
    owner_process: u32,
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    retained: linux::RetainedRuntimeClosureV2,
}

pub(crate) fn open_retained_generated_verus_runtime_v1(
    root: &Path,
) -> Result<RetainedGeneratedVerusRuntimeBackendV1, RetainedFunctionalRefinementRuntimeErrorV1> {
    validate_absolute_path(root)?;
    validate_runtime_root_path(root)?;
    let manifest = ManifestV2::parse_functional_refinement_runtime_v1()?;
    let identity = functional_refinement_closure_identity_v1();
    let owner_process = std::process::id();
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    {
        let retained = linux::RetainedRuntimeClosureV2::open_protected(root, &manifest)?;
        Ok(RetainedGeneratedVerusRuntimeBackendV1 {
            root: root.to_path_buf(),
            identity,
            owner_process,
            retained,
        })
    }
    #[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
    {
        let _ = (manifest, identity, owner_process);
        Err(RetainedFunctionalRefinementRuntimeErrorV1::new(
            RetainedFunctionalRefinementRuntimeErrorKindV1::UnsupportedPlatform,
            "retained no-follow runtime admission requires Linux",
        ))
    }
}

impl RetainedGeneratedVerusRuntimeBackendV1 {
    pub(crate) fn root(&self) -> &Path {
        &self.root
    }

    pub(crate) const fn identity(&self) -> [u8; 32] {
        self.identity
    }

    pub(crate) fn revalidate(&self) -> Result<(), RetainedFunctionalRefinementRuntimeErrorV1> {
        if std::process::id() != self.owner_process {
            return Err(RetainedFunctionalRefinementRuntimeErrorV1::new(
                RetainedFunctionalRefinementRuntimeErrorKindV1::OwnerProcessChanged,
                "runtime closure lease crossed a process boundary",
            ));
        }
        #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
        {
            self.retained.revalidate()
        }
        #[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
        {
            Err(RetainedFunctionalRefinementRuntimeErrorV1::new(
                RetainedFunctionalRefinementRuntimeErrorKindV1::UnsupportedPlatform,
                "retained no-follow runtime admission requires Linux",
            ))
        }
    }

    pub(crate) fn execute_generated_rust_verify(
        &self,
        source: &CanonicalGeneratedVerusProofInputV3,
        deadline: Instant,
        output_limit: usize,
    ) -> Result<
        RetainedFunctionalRefinementRuntimeOutputV1,
        RetainedFunctionalRefinementRuntimeErrorV1,
    > {
        self.revalidate()?;
        #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
        let result = linux::execute_functional_refinement_generated_rust_verify(
            &self.retained,
            source,
            deadline,
            output_limit,
        );
        #[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
        let result = Err(RetainedFunctionalRefinementRuntimeErrorV1::new(
            RetainedFunctionalRefinementRuntimeErrorKindV1::UnsupportedPlatform,
            "sealed generated rust_verify execution requires Linux x86-64",
        ));
        self.revalidate()?;
        result
    }
}

fn functional_refinement_closure_identity_v1() -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(FUNCTIONAL_REFINEMENT_CLOSURE_IDENTITY_DOMAIN_V1);
    put_blob(&mut digest, FUNCTIONAL_REFINEMENT_MANIFEST_BYTES);
    put_blob(&mut digest, RUST_TARGET_PINS);
    digest.finalize().into()
}

fn validate_absolute_path(path: &Path) -> Result<(), RetainedFunctionalRefinementRuntimeErrorV1> {
    if !path.is_absolute()
        || path.as_os_str().is_empty()
        || path
            .components()
            .any(|component| !matches!(component, Component::RootDir | Component::Normal(_)))
        || path == Path::new("/")
    {
        return Err(RetainedFunctionalRefinementRuntimeErrorV1::new(
            RetainedFunctionalRefinementRuntimeErrorKindV1::InvalidManifest,
            "runtime root must be a normalized absolute non-root path",
        ));
    }
    Ok(())
}

fn validate_runtime_root_path(
    path: &Path,
) -> Result<(), RetainedFunctionalRefinementRuntimeErrorV1> {
    const PARENT: &str = "/opt/fe2o3/verus-runtime-v2";
    let relative = path.strip_prefix(PARENT).map_err(|_| {
        RetainedFunctionalRefinementRuntimeErrorV1::new(
            RetainedFunctionalRefinementRuntimeErrorKindV1::InvalidManifest,
            "runtime root must be one canonical child of /opt/fe2o3/verus-runtime-v2",
        )
    })?;
    let mut components = relative.components();
    let Some(Component::Normal(name)) = components.next() else {
        return Err(RetainedFunctionalRefinementRuntimeErrorV1::new(
            RetainedFunctionalRefinementRuntimeErrorKindV1::InvalidManifest,
            "runtime root must name one versioned child directory",
        ));
    };
    let Some(name) = name.to_str() else {
        return Err(RetainedFunctionalRefinementRuntimeErrorV1::new(
            RetainedFunctionalRefinementRuntimeErrorKindV1::InvalidManifest,
            "runtime-root version is not UTF-8",
        ));
    };
    if components.next().is_some()
        || name.is_empty()
        || !name.as_bytes()[0].is_ascii_alphanumeric()
        || !name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
    {
        return Err(RetainedFunctionalRefinementRuntimeErrorV1::new(
            RetainedFunctionalRefinementRuntimeErrorKindV1::InvalidManifest,
            "runtime root has a noncanonical version component",
        ));
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum EntryKindV2 {
    Directory,
    File,
}

#[derive(Debug)]
pub(super) struct DirectorySpecV2 {
    pub(super) path: PathBuf,
    pub(super) mode: u32,
}

#[derive(Debug)]
pub(super) struct FileSpecV2 {
    pub(super) path: PathBuf,
    pub(super) mode: u32,
    pub(super) size: Option<u64>,
    pub(super) sha256: [u8; 32],
}

#[derive(Debug)]
pub(super) struct InterpreterSpecV2 {
    pub(super) requested: PathBuf,
    pub(super) canonical: PathBuf,
    pub(super) size: u64,
    pub(super) sha256: [u8; 32],
    pub(super) links: Vec<(PathBuf, PathBuf)>,
}

#[derive(Debug)]
pub(super) struct ManifestV2 {
    pub(super) manifest_name: &'static str,
    pub(super) root_mode: u32,
    pub(super) manifest_mode: u32,
    pub(super) manifest_bytes: &'static [u8],
    pub(super) directories: Vec<DirectorySpecV2>,
    pub(super) files: Vec<FileSpecV2>,
    pub(super) children: BTreeMap<PathBuf, BTreeMap<PathBuf, EntryKindV2>>,
    pub(super) interpreter: Option<InterpreterSpecV2>,
}

impl ManifestV2 {
    fn parse_functional_refinement_runtime_v1()
    -> Result<Self, RetainedFunctionalRefinementRuntimeErrorV1> {
        const MANIFEST_SHA256: [u8; 32] = [
            0xff, 0xef, 0x09, 0xbd, 0x24, 0x0c, 0x90, 0xe7, 0x2c, 0xbf, 0xf3, 0x1a, 0x82, 0xbc,
            0x51, 0x73, 0xc7, 0x96, 0xba, 0x7a, 0xb9, 0xaf, 0x23, 0x92, 0x45, 0xe7, 0xad, 0x89,
            0x2c, 0x25, 0x64, 0x1c,
        ];
        if FUNCTIONAL_REFINEMENT_MANIFEST_BYTES.len() > MAX_MANIFEST_BYTES
            || Sha256::digest(FUNCTIONAL_REFINEMENT_MANIFEST_BYTES).as_slice() != MANIFEST_SHA256
            || !FUNCTIONAL_REFINEMENT_MANIFEST_BYTES.ends_with(b"\n")
            || FUNCTIONAL_REFINEMENT_MANIFEST_BYTES.ends_with(b"\n\n")
        {
            return Err(invalid_manifest(
                "functional-refinement runtime manifest identity or framing differs",
            ));
        }
        let source = std::str::from_utf8(FUNCTIONAL_REFINEMENT_MANIFEST_BYTES)
            .map_err(|_| invalid_manifest("functional-refinement runtime manifest is not UTF-8"))?;
        let mut lines = source.lines();
        parse_reviewed_header(&mut lines, "format|fe2o3-functional-refinement-runtime-v1")?;
        validate_rust_target_pins()?;
        let interpreter = parse_interpreter(&mut lines)?;
        let manifest = parse_remaining_manifest(
            lines,
            interpreter,
            FUNCTIONAL_REFINEMENT_MANIFEST_BYTES,
            FUNCTIONAL_REFINEMENT_RUNTIME_V1_MANIFEST_NAME,
        )?;
        if manifest
            .directories
            .iter()
            .any(|entry| entry.path == Path::new("proof") || entry.path.starts_with("proof"))
            || manifest
                .files
                .iter()
                .any(|entry| entry.path == Path::new("proof") || entry.path.starts_with("proof"))
        {
            return Err(invalid_manifest(
                "functional-refinement runtime contains workload proof inventory",
            ));
        }
        Ok(manifest)
    }

    #[cfg(test)]
    pub(super) fn synthetic(
        manifest_bytes: &'static [u8],
        directories: Vec<DirectorySpecV2>,
        files: Vec<FileSpecV2>,
    ) -> Self {
        let children = expected_children(
            FUNCTIONAL_REFINEMENT_RUNTIME_V1_MANIFEST_NAME,
            &directories,
            &files,
        )
        .expect("valid synthetic manifest");
        Self {
            manifest_name: FUNCTIONAL_REFINEMENT_RUNTIME_V1_MANIFEST_NAME,
            root_mode: 0o555,
            manifest_mode: 0o444,
            manifest_bytes,
            directories,
            files,
            children,
            interpreter: None,
        }
    }
}

fn parse_reviewed_header(
    lines: &mut std::str::Lines<'_>,
    format: &str,
) -> Result<(), RetainedFunctionalRefinementRuntimeErrorV1> {
    for expected in [
        format,
        "platform|linux-x86_64",
        "root-mode|0555",
        "manifest-mode|0444",
        "verus-version|0.2026.08.02.b677dd5",
        "rust-toolchain|1.97.1-x86_64-unknown-linux-gnu",
        "rustc-commit|8bab26f4f68e0e26f0bb7960be334d5b520ea452",
        "llvm-version|22.1.6",
        "launcher-excluded|4713704|ad2669f579d898ede53f2bf84e80a1daf4e3578739b0f5807ef209a0c9f382dd",
        "rustup-excluded|20838840|4acc9acc76d5079515b46346a485974457b5a79893cfb01112423c89aeb5aa10",
        "rust-target-pins|62|6303|f32b5f5de52152a9a9706759532fdbdac4d3a6ed63a1efb3c56a0ec9025faffd",
    ] {
        if lines.next() != Some(expected) {
            return Err(invalid_manifest("reviewed manifest header differs"));
        }
    }
    Ok(())
}

fn validate_rust_target_pins() -> Result<(), RetainedFunctionalRefinementRuntimeErrorV1> {
    if RUST_TARGET_PINS.len() != 6303
        || RUST_TARGET_PINS.lines().count() != 62
        || Sha256::digest(RUST_TARGET_PINS).as_slice()
            != decode_sha256("f32b5f5de52152a9a9706759532fdbdac4d3a6ed63a1efb3c56a0ec9025faffd")?
    {
        return Err(invalid_manifest("Rust target pin identity differs"));
    }
    Ok(())
}

fn parse_interpreter(
    lines: &mut std::str::Lines<'_>,
) -> Result<InterpreterSpecV2, RetainedFunctionalRefinementRuntimeErrorV1> {
    let interpreter_line = lines
        .next()
        .ok_or_else(|| invalid_manifest("interpreter record is missing"))?;
    let fields = interpreter_line.split('|').collect::<Vec<_>>();
    if fields.len() != 5 || fields[0] != "interpreter" {
        return Err(invalid_manifest("interpreter record is malformed"));
    }
    Ok(InterpreterSpecV2 {
        requested: absolute_manifest_path(fields[1])?,
        canonical: absolute_manifest_path(fields[2])?,
        size: parse_decimal(fields[3])?,
        sha256: decode_sha256(fields[4])?,
        links: Vec::new(),
    })
}

fn parse_remaining_manifest(
    mut lines: std::str::Lines<'_>,
    mut interpreter: InterpreterSpecV2,
    manifest_bytes: &'static [u8],
    manifest_name: &'static str,
) -> Result<ManifestV2, RetainedFunctionalRefinementRuntimeErrorV1> {
    for _ in 0..2 {
        let line = lines
            .next()
            .ok_or_else(|| invalid_manifest("interpreter link record is missing"))?;
        let fields = line.split('|').collect::<Vec<_>>();
        if fields.len() != 3 || fields[0] != "interpreter-link" {
            return Err(invalid_manifest("interpreter link record is malformed"));
        }
        interpreter.links.push((
            absolute_manifest_path(fields[1])?,
            normalized_link_target(fields[2])?,
        ));
    }

    let mut directories = Vec::new();
    let mut files = Vec::new();
    let mut file_phase = false;
    for line in lines {
        let fields = line.split('|').collect::<Vec<_>>();
        match fields.first().copied() {
            Some("directory") if !file_phase && fields.len() == 3 => {
                directories.push(DirectorySpecV2 {
                    mode: parse_mode(fields[1])?,
                    path: relative_manifest_path(fields[2])?,
                });
            }
            Some("file") if fields.len() == 5 => {
                file_phase = true;
                files.push(FileSpecV2 {
                    mode: parse_mode(fields[1])?,
                    size: Some(parse_decimal(fields[2])?),
                    sha256: decode_sha256(fields[3])?,
                    path: relative_manifest_path(fields[4])?,
                });
            }
            _ => return Err(invalid_manifest("manifest entry is malformed or reordered")),
        }
    }
    if directories.is_empty()
        || directories.len() > MAX_RUNTIME_DIRECTORIES
        || files.is_empty()
        || files.len() > MAX_RUNTIME_FILES
        || !directories
            .windows(2)
            .all(|pair| pair[0].path < pair[1].path)
        || !files.windows(2).all(|pair| pair[0].path < pair[1].path)
    {
        return Err(invalid_manifest(
            "manifest entry bounds or ordering differs",
        ));
    }
    append_target_pins(&mut files)?;
    files.sort_by(|left, right| left.path.cmp(&right.path));
    if files.len() > MAX_RUNTIME_FILES || !files.windows(2).all(|pair| pair[0].path < pair[1].path)
    {
        return Err(invalid_manifest(
            "runtime file inventory contains duplicates",
        ));
    }
    let children = expected_children(manifest_name, &directories, &files)?;
    Ok(ManifestV2 {
        manifest_name,
        root_mode: 0o555,
        manifest_mode: 0o444,
        manifest_bytes,
        directories,
        files,
        children,
        interpreter: Some(interpreter),
    })
}

fn append_target_pins(
    files: &mut Vec<FileSpecV2>,
) -> Result<(), RetainedFunctionalRefinementRuntimeErrorV1> {
    let source = std::str::from_utf8(RUST_TARGET_PINS)
        .map_err(|_| invalid_manifest("target pins are not UTF-8"))?;
    let mut prior = None;
    for line in source.lines() {
        let (digest, name) = line
            .split_once("  ")
            .ok_or_else(|| invalid_manifest("target pin is malformed"))?;
        if name.contains(' ') || name.contains('/') || name.is_empty() {
            return Err(invalid_manifest(
                "target pin name is not one path component",
            ));
        }
        if prior.is_some_and(|prior: &str| prior >= name) {
            return Err(invalid_manifest("target pins are not strictly sorted"));
        }
        prior = Some(name);
        files.push(FileSpecV2 {
            path: Path::new(TARGET_PREFIX).join(name),
            mode: 0o444,
            size: None,
            sha256: decode_sha256(digest)?,
        });
    }
    Ok(())
}

fn expected_children(
    manifest_name: &'static str,
    directories: &[DirectorySpecV2],
    files: &[FileSpecV2],
) -> Result<
    BTreeMap<PathBuf, BTreeMap<PathBuf, EntryKindV2>>,
    RetainedFunctionalRefinementRuntimeErrorV1,
> {
    let declared = directories
        .iter()
        .map(|directory| directory.path.clone())
        .collect::<BTreeSet<_>>();
    let mut children = BTreeMap::<PathBuf, BTreeMap<PathBuf, EntryKindV2>>::new();
    children
        .entry(PathBuf::new())
        .or_default()
        .insert(PathBuf::from(manifest_name), EntryKindV2::File);
    for directory in directories {
        let parent = directory.path.parent().unwrap_or_else(|| Path::new(""));
        if !parent.as_os_str().is_empty() && !declared.contains(parent) {
            return Err(invalid_manifest("directory parent is not declared"));
        }
        let name = directory
            .path
            .file_name()
            .ok_or_else(|| invalid_manifest("directory name is empty"))?;
        if children
            .entry(parent.to_path_buf())
            .or_default()
            .insert(PathBuf::from(name), EntryKindV2::Directory)
            .is_some()
        {
            return Err(invalid_manifest("directory entry is duplicated"));
        }
        children.entry(directory.path.clone()).or_default();
    }
    for file in files {
        let parent = file.path.parent().unwrap_or_else(|| Path::new(""));
        if !parent.as_os_str().is_empty() && !declared.contains(parent) {
            return Err(invalid_manifest("file parent is not declared"));
        }
        let name = file
            .path
            .file_name()
            .ok_or_else(|| invalid_manifest("file name is empty"))?;
        if children
            .entry(parent.to_path_buf())
            .or_default()
            .insert(PathBuf::from(name), EntryKindV2::File)
            .is_some()
        {
            return Err(invalid_manifest("file entry is duplicated"));
        }
    }
    Ok(children)
}

fn parse_mode(value: &str) -> Result<u32, RetainedFunctionalRefinementRuntimeErrorV1> {
    if value.len() != 4 || !value.starts_with('0') {
        return Err(invalid_manifest("mode is not four-digit octal"));
    }
    u32::from_str_radix(value, 8).map_err(|_| invalid_manifest("mode is not octal"))
}

fn parse_decimal(value: &str) -> Result<u64, RetainedFunctionalRefinementRuntimeErrorV1> {
    if value.is_empty() || (value.len() > 1 && value.starts_with('0')) {
        return Err(invalid_manifest("decimal field is not canonical"));
    }
    value
        .parse::<u64>()
        .map_err(|_| invalid_manifest("decimal field is invalid"))
}

fn decode_sha256(value: &str) -> Result<[u8; 32], RetainedFunctionalRefinementRuntimeErrorV1> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err(invalid_manifest("SHA-256 is not lowercase hexadecimal"));
    }
    let mut result = [0_u8; 32];
    for (index, chunk) in value.as_bytes().chunks_exact(2).enumerate() {
        result[index] = (hex_nibble(chunk[0])? << 4) | hex_nibble(chunk[1])?;
    }
    Ok(result)
}

fn hex_nibble(value: u8) -> Result<u8, RetainedFunctionalRefinementRuntimeErrorV1> {
    match value {
        b'0'..=b'9' => Ok(value - b'0'),
        b'a'..=b'f' => Ok(value - b'a' + 10),
        _ => Err(invalid_manifest("SHA-256 contains a non-hexadecimal byte")),
    }
}

fn relative_manifest_path(
    value: &str,
) -> Result<PathBuf, RetainedFunctionalRefinementRuntimeErrorV1> {
    let path = PathBuf::from(value);
    if value.is_empty()
        || value.len() > MAX_RELATIVE_PATH_BYTES
        || path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(invalid_manifest("relative manifest path is not normalized"));
    }
    Ok(path)
}

fn absolute_manifest_path(
    value: &str,
) -> Result<PathBuf, RetainedFunctionalRefinementRuntimeErrorV1> {
    let path = PathBuf::from(value);
    validate_absolute_path(&path)?;
    Ok(path)
}

fn normalized_link_target(
    value: &str,
) -> Result<PathBuf, RetainedFunctionalRefinementRuntimeErrorV1> {
    if value.is_empty() || value.len() > MAX_RELATIVE_PATH_BYTES || value.as_bytes().contains(&0) {
        return Err(invalid_manifest("interpreter link target is malformed"));
    }
    Ok(PathBuf::from(value))
}

fn invalid_manifest(detail: impl Into<String>) -> RetainedFunctionalRefinementRuntimeErrorV1 {
    RetainedFunctionalRefinementRuntimeErrorV1::new(
        RetainedFunctionalRefinementRuntimeErrorKindV1::InvalidManifest,
        detail,
    )
}

trait ByteLines {
    fn lines(&self) -> std::str::Lines<'_>;
}

impl ByteLines for [u8] {
    fn lines(&self) -> std::str::Lines<'_> {
        std::str::from_utf8(self).unwrap_or("").lines()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_has_no_workload_proof_inventory() {
        let manifest = ManifestV2::parse_functional_refinement_runtime_v1().unwrap();
        assert_eq!(
            manifest.manifest_name,
            FUNCTIONAL_REFINEMENT_RUNTIME_V1_MANIFEST_NAME
        );
        assert!(
            manifest
                .directories
                .iter()
                .all(|entry| !entry.path.starts_with("proof"))
        );
        assert!(
            manifest
                .files
                .iter()
                .all(|entry| !entry.path.starts_with("proof"))
        );
        assert_ne!(functional_refinement_closure_identity_v1(), [0; 32]);
    }

    #[test]
    fn runtime_root_requires_one_canonical_opt_version() {
        assert!(
            validate_runtime_root_path(Path::new(
                "/opt/fe2o3/verus-runtime-v2/0.2026.08.02-b677dd5"
            ))
            .is_ok()
        );
        for path in [
            "/tmp/runtime",
            "/opt/fe2o3/verus-runtime-v2",
            "/opt/fe2o3/verus-runtime-v2/-invalid",
            "/opt/fe2o3/verus-runtime-v2/version/extra",
        ] {
            assert_eq!(
                validate_runtime_root_path(Path::new(path))
                    .unwrap_err()
                    .kind(),
                RetainedFunctionalRefinementRuntimeErrorKindV1::InvalidManifest
            );
        }
    }
}
