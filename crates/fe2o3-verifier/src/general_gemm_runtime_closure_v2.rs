//! Retained filesystem admission for the general-GEMM Verus runtime closure.
//!
//! This module does not execute Verus and cannot construct proof evidence. It
//! only admits the exact reviewed V2 filesystem closure and retains the opened
//! objects so a later supervised runner can use the same generation.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::{Component, Path, PathBuf};
use std::time::Instant;

use sha2::{Digest, Sha256};

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[path = "general_gemm_runtime_closure_v2_linux.rs"]
mod linux;

/// Exact installed manifest filename.
pub const GENERAL_GEMM_RUNTIME_CLOSURE_V2_MANIFEST_NAME: &str =
    "GENERAL_GEMM_RUNTIME_CLOSURE_V2.manifest";

/// SHA-256 of the byte-canonical reviewed runtime manifest.
pub const GENERAL_GEMM_RUNTIME_CLOSURE_V2_MANIFEST_SHA256: [u8; 32] = [
    0xbc, 0x5d, 0x19, 0x49, 0x29, 0xee, 0x4a, 0x1b, 0x57, 0xeb, 0x5b, 0xcc, 0x30, 0x36, 0xb4, 0xe9,
    0x34, 0x14, 0x59, 0x2f, 0x0e, 0xff, 0x5a, 0x75, 0x08, 0x35, 0x58, 0x7e, 0x14, 0xd3, 0x41, 0x98,
];

const MANIFEST_BYTES: &[u8] =
    include_bytes!("../verus/pins/GENERAL_GEMM_RUNTIME_CLOSURE_V2.manifest");
const RUST_TARGET_PINS: &[u8] = include_bytes!("../verus/pins/rust_target_1_97_1.sha256");
const CLOSURE_IDENTITY_DOMAIN: &[u8] = b"fe2o3-general-gemm-runtime-closure-v2\0";
const TARGET_PREFIX: &str = "toolchain/lib/rustlib/x86_64-unknown-linux-gnu/lib";
const MAX_MANIFEST_BYTES: usize = 64 * 1024;
const MAX_RUNTIME_FILES: usize = 128;
const MAX_RUNTIME_DIRECTORIES: usize = 32;
const MAX_RELATIVE_PATH_BYTES: usize = 512;
const MAX_TARGET_FILE_BYTES: u64 = 70 * 1024 * 1024;

/// Stable category for a runtime-closure admission failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GeneralGemmRuntimeClosureErrorKindV2 {
    /// The retained closure is available only on Linux.
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
pub struct GeneralGemmRuntimeClosureErrorV2 {
    kind: GeneralGemmRuntimeClosureErrorKindV2,
    detail: String,
}

impl GeneralGemmRuntimeClosureErrorV2 {
    pub(crate) fn new(
        kind: GeneralGemmRuntimeClosureErrorKindV2,
        detail: impl Into<String>,
    ) -> Self {
        Self {
            kind,
            detail: detail.into(),
        }
    }

    /// Returns the stable failure category.
    pub const fn kind(&self) -> GeneralGemmRuntimeClosureErrorKindV2 {
        self.kind
    }
}

impl fmt::Display for GeneralGemmRuntimeClosureErrorV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "general GEMM runtime closure V2: {}",
            self.detail
        )
    }
}

impl std::error::Error for GeneralGemmRuntimeClosureErrorV2 {}

/// Domain-separated identity of the reviewed runtime manifest and target pins.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct GeneralGemmRuntimeClosureIdentityV2([u8; 32]);

impl GeneralGemmRuntimeClosureIdentityV2 {
    /// Returns the exact identity bytes.
    pub const fn as_bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Non-copyable lease over one retained, exact runtime-closure generation.
///
/// Opening this lease does not run a proof and grants no compiler, artifact,
/// publication, or launch authority.
pub struct GeneralGemmVerusRuntimeClosureLeaseV2 {
    root: PathBuf,
    identity: GeneralGemmRuntimeClosureIdentityV2,
    owner_process: u32,
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    retained: linux::RetainedRuntimeClosureV2,
}

/// One immutable wrapper, model, and proof-body input set.
pub(crate) struct GeneralGemmSealedProofInputV2 {
    identity: [u8; 32],
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    sealed: linux::SealedProofInputV2,
}

/// Bounded output from one directly executed retained `rust_verify` process.
pub(crate) struct GeneralGemmRuntimeProcessOutputV2 {
    pub(crate) exit_code: Option<i32>,
    pub(crate) signal: Option<i32>,
    pub(crate) stdout: Vec<u8>,
    pub(crate) stderr: Vec<u8>,
}

impl fmt::Debug for GeneralGemmVerusRuntimeClosureLeaseV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneralGemmVerusRuntimeClosureLeaseV2")
            .field("root", &self.root)
            .field("identity", &self.identity)
            .field("owner_process", &self.owner_process)
            .finish_non_exhaustive()
    }
}

impl GeneralGemmVerusRuntimeClosureLeaseV2 {
    /// Opens the exact root-owned runtime closure without following links.
    pub fn open(root: impl AsRef<Path>) -> Result<Self, GeneralGemmRuntimeClosureErrorV2> {
        let root = root.as_ref();
        validate_absolute_path(root)?;
        validate_runtime_root_path(root)?;
        let manifest = ManifestV2::parse_reviewed()?;
        let identity = closure_identity();
        let owner_process = std::process::id();
        #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
        {
            let retained = linux::RetainedRuntimeClosureV2::open_protected(root, &manifest)?;
            Ok(Self {
                root: root.to_path_buf(),
                identity,
                owner_process,
                retained,
            })
        }
        #[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
        {
            let _ = manifest;
            Err(GeneralGemmRuntimeClosureErrorV2::new(
                GeneralGemmRuntimeClosureErrorKindV2::UnsupportedPlatform,
                "retained no-follow runtime admission requires Linux",
            ))
        }
    }

    /// Returns the caller-supplied diagnostic root path.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Returns the reviewed manifest-and-target-pin identity.
    pub const fn identity(&self) -> GeneralGemmRuntimeClosureIdentityV2 {
        self.identity
    }

    /// Revalidates every retained path edge, object, and exact directory inventory.
    pub fn revalidate(&self) -> Result<(), GeneralGemmRuntimeClosureErrorV2> {
        if std::process::id() != self.owner_process {
            return Err(GeneralGemmRuntimeClosureErrorV2::new(
                GeneralGemmRuntimeClosureErrorKindV2::OwnerProcessChanged,
                "runtime closure lease crossed a process boundary",
            ));
        }
        #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
        {
            self.retained.revalidate()
        }
        #[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
        {
            Err(GeneralGemmRuntimeClosureErrorV2::new(
                GeneralGemmRuntimeClosureErrorKindV2::UnsupportedPlatform,
                "retained no-follow runtime admission requires Linux",
            ))
        }
    }

    pub(crate) fn seal_proof_input(
        wrapper: &[u8],
        model: &[u8],
        proof: &[u8],
    ) -> Result<GeneralGemmSealedProofInputV2, GeneralGemmRuntimeClosureErrorV2> {
        let identity = proof_input_identity(wrapper, model, proof);
        #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
        {
            let sealed = linux::SealedProofInputV2::new(wrapper, model, proof)?;
            let input = GeneralGemmSealedProofInputV2 { identity, sealed };
            input.revalidate()?;
            Ok(input)
        }
        #[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
        {
            let _ = (wrapper, model, proof, identity);
            Err(GeneralGemmRuntimeClosureErrorV2::new(
                GeneralGemmRuntimeClosureErrorKindV2::UnsupportedPlatform,
                "sealed general GEMM proof inputs require Linux x86-64",
            ))
        }
    }

    pub(crate) fn execute_rust_verify(
        &self,
        input: &GeneralGemmSealedProofInputV2,
        deadline: Instant,
        output_limit: usize,
    ) -> Result<GeneralGemmRuntimeProcessOutputV2, GeneralGemmRuntimeClosureErrorV2> {
        self.revalidate()?;
        input.revalidate()?;
        #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
        let result =
            linux::execute_rust_verify(&self.retained, &input.sealed, deadline, output_limit);
        #[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
        let result = Err(GeneralGemmRuntimeClosureErrorV2::new(
            GeneralGemmRuntimeClosureErrorKindV2::UnsupportedPlatform,
            "direct retained rust_verify execution requires Linux x86-64",
        ));
        self.revalidate()?;
        input.revalidate()?;
        result
    }
}

impl GeneralGemmSealedProofInputV2 {
    pub(crate) const fn identity(&self) -> [u8; 32] {
        self.identity
    }

    fn revalidate(&self) -> Result<(), GeneralGemmRuntimeClosureErrorV2> {
        #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
        {
            self.sealed.revalidate(self.identity)
        }
        #[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
        {
            Err(GeneralGemmRuntimeClosureErrorV2::new(
                GeneralGemmRuntimeClosureErrorKindV2::UnsupportedPlatform,
                "sealed general GEMM proof inputs require Linux x86-64",
            ))
        }
    }
}

fn closure_identity() -> GeneralGemmRuntimeClosureIdentityV2 {
    let mut digest = Sha256::new();
    digest.update(CLOSURE_IDENTITY_DOMAIN);
    put_blob(&mut digest, MANIFEST_BYTES);
    put_blob(&mut digest, RUST_TARGET_PINS);
    GeneralGemmRuntimeClosureIdentityV2(digest.finalize().into())
}

fn proof_input_identity(wrapper: &[u8], model: &[u8], proof: &[u8]) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(b"fe2o3-general-gemm-sealed-proof-input-v2\0");
    put_blob(&mut digest, wrapper);
    put_blob(&mut digest, model);
    put_blob(&mut digest, proof);
    digest.finalize().into()
}

fn put_blob(digest: &mut Sha256, value: &[u8]) {
    digest.update((value.len() as u64).to_le_bytes());
    digest.update(value);
}

fn validate_absolute_path(path: &Path) -> Result<(), GeneralGemmRuntimeClosureErrorV2> {
    if !path.is_absolute()
        || path.as_os_str().is_empty()
        || path
            .components()
            .any(|component| !matches!(component, Component::RootDir | Component::Normal(_)))
        || path == Path::new("/")
    {
        return Err(GeneralGemmRuntimeClosureErrorV2::new(
            GeneralGemmRuntimeClosureErrorKindV2::InvalidManifest,
            "runtime root must be a normalized absolute non-root path",
        ));
    }
    Ok(())
}

fn validate_runtime_root_path(path: &Path) -> Result<(), GeneralGemmRuntimeClosureErrorV2> {
    const PARENT: &str = "/opt/fe2o3/verus-runtime-v2";
    let relative = path.strip_prefix(PARENT).map_err(|_| {
        GeneralGemmRuntimeClosureErrorV2::new(
            GeneralGemmRuntimeClosureErrorKindV2::InvalidManifest,
            "runtime root must be one canonical child of /opt/fe2o3/verus-runtime-v2",
        )
    })?;
    let mut components = relative.components();
    let Some(Component::Normal(name)) = components.next() else {
        return Err(GeneralGemmRuntimeClosureErrorV2::new(
            GeneralGemmRuntimeClosureErrorKindV2::InvalidManifest,
            "runtime root must name one versioned child directory",
        ));
    };
    let Some(name) = name.to_str() else {
        return Err(GeneralGemmRuntimeClosureErrorV2::new(
            GeneralGemmRuntimeClosureErrorKindV2::InvalidManifest,
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
        return Err(GeneralGemmRuntimeClosureErrorV2::new(
            GeneralGemmRuntimeClosureErrorKindV2::InvalidManifest,
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
    pub(super) root_mode: u32,
    pub(super) manifest_mode: u32,
    pub(super) manifest_bytes: &'static [u8],
    pub(super) directories: Vec<DirectorySpecV2>,
    pub(super) files: Vec<FileSpecV2>,
    pub(super) children: BTreeMap<PathBuf, BTreeMap<PathBuf, EntryKindV2>>,
    pub(super) interpreter: Option<InterpreterSpecV2>,
}

impl ManifestV2 {
    fn parse_reviewed() -> Result<Self, GeneralGemmRuntimeClosureErrorV2> {
        if MANIFEST_BYTES.len() > MAX_MANIFEST_BYTES
            || Sha256::digest(MANIFEST_BYTES).as_slice()
                != GENERAL_GEMM_RUNTIME_CLOSURE_V2_MANIFEST_SHA256
            || !MANIFEST_BYTES.ends_with(b"\n")
            || MANIFEST_BYTES.ends_with(b"\n\n")
        {
            return Err(invalid_manifest(
                "reviewed manifest identity or framing differs",
            ));
        }
        let source = std::str::from_utf8(MANIFEST_BYTES)
            .map_err(|_| invalid_manifest("reviewed manifest is not UTF-8"))?;
        let mut lines = source.lines();
        for expected in [
            "format|fe2o3-general-gemm-runtime-closure-v2",
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
        if RUST_TARGET_PINS.len() != 6303
            || RUST_TARGET_PINS.lines().count() != 62
            || Sha256::digest(RUST_TARGET_PINS).as_slice()
                != decode_sha256(
                    "f32b5f5de52152a9a9706759532fdbdac4d3a6ed63a1efb3c56a0ec9025faffd",
                )?
        {
            return Err(invalid_manifest("Rust target pin identity differs"));
        }

        let interpreter_line = lines
            .next()
            .ok_or_else(|| invalid_manifest("interpreter record is missing"))?;
        let interpreter_fields = interpreter_line.split('|').collect::<Vec<_>>();
        if interpreter_fields.len() != 5 || interpreter_fields[0] != "interpreter" {
            return Err(invalid_manifest("interpreter record is malformed"));
        }
        let interpreter = InterpreterSpecV2 {
            requested: absolute_manifest_path(interpreter_fields[1])?,
            canonical: absolute_manifest_path(interpreter_fields[2])?,
            size: parse_decimal(interpreter_fields[3])?,
            sha256: decode_sha256(interpreter_fields[4])?,
            links: Vec::new(),
        };
        parse_remaining_manifest(lines, interpreter)
    }

    #[cfg(test)]
    pub(super) fn synthetic(
        manifest_bytes: &'static [u8],
        directories: Vec<DirectorySpecV2>,
        files: Vec<FileSpecV2>,
    ) -> Self {
        let children = expected_children(&directories, &files).expect("valid synthetic manifest");
        Self {
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

fn parse_remaining_manifest(
    mut lines: std::str::Lines<'_>,
    mut interpreter: InterpreterSpecV2,
) -> Result<ManifestV2, GeneralGemmRuntimeClosureErrorV2> {
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
    let children = expected_children(&directories, &files)?;
    Ok(ManifestV2 {
        root_mode: 0o555,
        manifest_mode: 0o444,
        manifest_bytes: MANIFEST_BYTES,
        directories,
        files,
        children,
        interpreter: Some(interpreter),
    })
}

fn append_target_pins(files: &mut Vec<FileSpecV2>) -> Result<(), GeneralGemmRuntimeClosureErrorV2> {
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
    directories: &[DirectorySpecV2],
    files: &[FileSpecV2],
) -> Result<BTreeMap<PathBuf, BTreeMap<PathBuf, EntryKindV2>>, GeneralGemmRuntimeClosureErrorV2> {
    let declared = directories
        .iter()
        .map(|directory| directory.path.clone())
        .collect::<BTreeSet<_>>();
    let mut children = BTreeMap::<PathBuf, BTreeMap<PathBuf, EntryKindV2>>::new();
    children.entry(PathBuf::new()).or_default().insert(
        PathBuf::from(GENERAL_GEMM_RUNTIME_CLOSURE_V2_MANIFEST_NAME),
        EntryKindV2::File,
    );
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

fn parse_mode(value: &str) -> Result<u32, GeneralGemmRuntimeClosureErrorV2> {
    if value.len() != 4 || !value.starts_with('0') {
        return Err(invalid_manifest("mode is not four-digit octal"));
    }
    u32::from_str_radix(value, 8).map_err(|_| invalid_manifest("mode is not octal"))
}

fn parse_decimal(value: &str) -> Result<u64, GeneralGemmRuntimeClosureErrorV2> {
    if value.is_empty() || (value.len() > 1 && value.starts_with('0')) {
        return Err(invalid_manifest("decimal field is not canonical"));
    }
    value
        .parse::<u64>()
        .map_err(|_| invalid_manifest("decimal field is invalid"))
}

fn decode_sha256(value: &str) -> Result<[u8; 32], GeneralGemmRuntimeClosureErrorV2> {
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

fn hex_nibble(value: u8) -> Result<u8, GeneralGemmRuntimeClosureErrorV2> {
    match value {
        b'0'..=b'9' => Ok(value - b'0'),
        b'a'..=b'f' => Ok(value - b'a' + 10),
        _ => Err(invalid_manifest("SHA-256 contains a non-hexadecimal byte")),
    }
}

fn relative_manifest_path(value: &str) -> Result<PathBuf, GeneralGemmRuntimeClosureErrorV2> {
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

fn absolute_manifest_path(value: &str) -> Result<PathBuf, GeneralGemmRuntimeClosureErrorV2> {
    let path = PathBuf::from(value);
    validate_absolute_path(&path)?;
    Ok(path)
}

fn normalized_link_target(value: &str) -> Result<PathBuf, GeneralGemmRuntimeClosureErrorV2> {
    if value.is_empty() || value.len() > MAX_RELATIVE_PATH_BYTES || value.as_bytes().contains(&0) {
        return Err(invalid_manifest("interpreter link target is malformed"));
    }
    Ok(PathBuf::from(value))
}

fn invalid_manifest(detail: impl Into<String>) -> GeneralGemmRuntimeClosureErrorV2 {
    GeneralGemmRuntimeClosureErrorV2::new(
        GeneralGemmRuntimeClosureErrorKindV2::InvalidManifest,
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
    fn reviewed_manifest_and_target_pins_are_canonical() {
        let manifest = ManifestV2::parse_reviewed().unwrap();
        assert_eq!(manifest.directories.len(), 8);
        assert_eq!(manifest.files.len(), 80);
        assert!(manifest.interpreter.is_some());
        assert_eq!(
            Sha256::digest(MANIFEST_BYTES).as_slice(),
            GENERAL_GEMM_RUNTIME_CLOSURE_V2_MANIFEST_SHA256
        );
        assert_ne!(closure_identity().as_bytes(), [0; 32]);
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
                GeneralGemmRuntimeClosureErrorKindV2::InvalidManifest
            );
        }
    }
}
