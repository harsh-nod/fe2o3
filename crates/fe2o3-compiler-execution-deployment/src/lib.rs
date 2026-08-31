//! Pinned compiler-execution deployment manifest and sealed source custody.

#![cfg(target_os = "linux")]
#![deny(missing_docs, unsafe_code)]

use std::error::Error;
use std::ffi::{OsStr, OsString};
use std::fmt;
use std::fmt::Write as _;
use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::os::fd::AsFd;
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::path::Path;

use rustix::fs::{
    FileType, MemfdFlags, Mode, OFlags, ResolveFlags, SealFlags, fcntl_add_seals, fcntl_get_seals,
    flistxattr, fstat, memfd_create, openat, openat2,
};
use sha2::{Digest, Sha256};

mod host;
mod install;
mod mount;
mod qualification;
mod run;
mod staging;
mod supervisor;

pub use host::{
    CompilerExecutionQualificationHostProbeV1, probe_compiler_execution_qualification_host_v1,
};
pub use install::{
    CompilerExecutionInstallRecoveryV1, CompilerExecutionInstalledRootPublicationV1,
    InstalledCompilerExecutionDeploymentV1, compiler_execution_install_root_name_v1,
    install_compiler_execution_deployment_v1, recover_compiler_execution_install_parent_v1,
};
pub use mount::{
    MountedCompilerExecutionQualificationV1, PrivateQualificationMountNamespaceV1,
    QualificationMountFaultPointV1, attach_compiler_execution_qualification_mounts_v1,
    enter_private_qualification_mount_namespace_v1,
};
pub use qualification::{
    PreparedCompilerExecutionQualificationV1, prepare_compiler_execution_qualification_v1,
};
pub use run::{
    CompilerExecutionMountCampaignReportV1, CompilerExecutionMountFaultReportV1,
    CompilerExecutionMountQualificationReportV1, CompilerExecutionMountQualificationRequestV1,
    run_compiler_execution_mount_campaign_v1, run_compiler_execution_mount_fault_v1,
    run_compiler_execution_mount_qualification_request_v1,
    run_compiler_execution_mount_qualification_v1,
};
pub use staging::{
    CompilerExecutionQualificationRecoveryV1, StagedCompilerExecutionQualificationV1,
    recover_compiler_execution_qualification_parent_v1, stage_compiler_execution_qualification_v1,
};
pub use supervisor::{QualificationWorkerTerminationV1, wait_for_qualification_worker_v1};

/// Canonical deployment target admitted by this V1 profile.
pub const COMPILER_EXECUTION_DEPLOYMENT_TARGET_V1: &str = "x86_64-unknown-linux-musl";
/// Canonical install-manifest source name inside a deployment bundle.
pub const COMPILER_EXECUTION_INSTALL_MANIFEST_NAME_V1: &str = "INSTALL-MANIFEST-V1";
/// Number of content files bound by the V1 manifest, excluding the manifest itself.
pub const COMPILER_EXECUTION_INSTALL_FILE_COUNT_V1: usize = 13;

const MANIFEST_HEADER_V1: &str = "fe2o3-compiler-execution-install-manifest-v1";
const MANIFEST_MAX_BYTES_V1: usize = 32 * 1024;
const CONFIG_MAX_BYTES_V1: u64 = 1024 * 1024;
const EXECUTABLE_MAX_BYTES_V1: u64 = 128 * 1024 * 1024;
const GIT_COMMIT_BYTES_V1: usize = 20;
const DIRECTORY_MODE_V1: u32 = 0o700;
const MANIFEST_MODE_V1: u32 = 0o444;

#[derive(Clone, Copy, Debug)]
struct FileSpecV1 {
    source: &'static str,
    install: &'static str,
    mode: u32,
    max_bytes: u64,
}

const MANIFEST_FILE_SPEC_V1: FileSpecV1 = FileSpecV1 {
    source: COMPILER_EXECUTION_INSTALL_MANIFEST_NAME_V1,
    install: "/usr/share/fe2o3/compiler-execution/INSTALL-MANIFEST-V1",
    mode: MANIFEST_MODE_V1,
    max_bytes: MANIFEST_MAX_BYTES_V1 as u64,
};

const FILE_SPECS_V1: [FileSpecV1; COMPILER_EXECUTION_INSTALL_FILE_COUNT_V1] = [
    FileSpecV1 {
        source: "BUILD-INFO",
        install: "/usr/share/fe2o3/compiler-execution/BUILD-INFO",
        mode: 0o444,
        max_bytes: CONFIG_MAX_BYTES_V1,
    },
    FileSpecV1 {
        source: "SHA256SUMS",
        install: "/usr/share/fe2o3/compiler-execution/SHA256SUMS",
        mode: 0o444,
        max_bytes: CONFIG_MAX_BYTES_V1,
    },
    FileSpecV1 {
        source: "systemd/fe2o3-compiler-execution.service",
        install: "/usr/lib/systemd/system/fe2o3-compiler-execution.service",
        mode: 0o444,
        max_bytes: CONFIG_MAX_BYTES_V1,
    },
    FileSpecV1 {
        source: "systemd/fe2o3-compiler-execution.socket",
        install: "/usr/lib/systemd/system/fe2o3-compiler-execution.socket",
        mode: 0o444,
        max_bytes: CONFIG_MAX_BYTES_V1,
    },
    FileSpecV1 {
        source: "sysusers.d/fe2o3-compiler-execution.conf",
        install: "/usr/lib/sysusers.d/fe2o3-compiler-execution.conf",
        mode: 0o444,
        max_bytes: CONFIG_MAX_BYTES_V1,
    },
    FileSpecV1 {
        source: "tmpfiles.d/fe2o3-compiler-execution.conf",
        install: "/usr/lib/tmpfiles.d/fe2o3-compiler-execution.conf",
        mode: 0o444,
        max_bytes: CONFIG_MAX_BYTES_V1,
    },
    FileSpecV1 {
        source: "usr/libexec/fe2o3/fe2o3-compiler-execution-coordinator",
        install: "/usr/libexec/fe2o3/fe2o3-compiler-execution-coordinator",
        mode: 0o555,
        max_bytes: EXECUTABLE_MAX_BYTES_V1,
    },
    FileSpecV1 {
        source: "usr/libexec/fe2o3/fe2o3-compiler-execution-issuer",
        install: "/usr/libexec/fe2o3/fe2o3-compiler-execution-issuer",
        mode: 0o555,
        max_bytes: EXECUTABLE_MAX_BYTES_V1,
    },
    FileSpecV1 {
        source: "usr/libexec/fe2o3/fe2o3-compiler-execution-provision",
        install: "/usr/libexec/fe2o3/fe2o3-compiler-execution-provision",
        mode: 0o555,
        max_bytes: EXECUTABLE_MAX_BYTES_V1,
    },
    FileSpecV1 {
        source: "usr/libexec/fe2o3/fe2o3-compiler-execution-supervisor",
        install: "/usr/libexec/fe2o3/fe2o3-compiler-execution-supervisor",
        mode: 0o555,
        max_bytes: EXECUTABLE_MAX_BYTES_V1,
    },
    FileSpecV1 {
        source: "usr/libexec/fe2o3/fe2o3-external-anchor-provisioning-helper",
        install: "/usr/libexec/fe2o3/fe2o3-external-anchor-provisioning-helper",
        mode: 0o555,
        max_bytes: EXECUTABLE_MAX_BYTES_V1,
    },
    FileSpecV1 {
        source: "usr/libexec/fe2o3/fe2o3-external-anchor-service",
        install: "/usr/libexec/fe2o3/fe2o3-external-anchor-service",
        mode: 0o555,
        max_bytes: EXECUTABLE_MAX_BYTES_V1,
    },
    FileSpecV1 {
        source: "usr/libexec/fe2o3/fe2o3-static-preexec-launcher",
        install: "/usr/libexec/fe2o3/fe2o3-static-preexec-launcher",
        mode: 0o555,
        max_bytes: EXECUTABLE_MAX_BYTES_V1,
    },
];

const ROOT_CHILDREN_WITH_MANIFEST_V1: &[&str] = &[
    "BUILD-INFO",
    "INSTALL-MANIFEST-V1",
    "SHA256SUMS",
    "systemd",
    "sysusers.d",
    "tmpfiles.d",
    "usr",
];
const ROOT_CHILDREN_WITHOUT_MANIFEST_V1: &[&str] = &[
    "BUILD-INFO",
    "SHA256SUMS",
    "systemd",
    "sysusers.d",
    "tmpfiles.d",
    "usr",
];
const SYSTEMD_CHILDREN_V1: &[&str] = &[
    "fe2o3-compiler-execution.service",
    "fe2o3-compiler-execution.socket",
];
const SYSUSERS_CHILDREN_V1: &[&str] = &["fe2o3-compiler-execution.conf"];
const TMPFILES_CHILDREN_V1: &[&str] = &["fe2o3-compiler-execution.conf"];
const USR_CHILDREN_V1: &[&str] = &["libexec"];
const LIBEXEC_CHILDREN_V1: &[&str] = &["fe2o3"];
const IMAGE_CHILDREN_V1: &[&str] = &[
    "fe2o3-compiler-execution-coordinator",
    "fe2o3-compiler-execution-issuer",
    "fe2o3-compiler-execution-provision",
    "fe2o3-compiler-execution-supervisor",
    "fe2o3-external-anchor-provisioning-helper",
    "fe2o3-external-anchor-service",
    "fe2o3-static-preexec-launcher",
];

const DIRECTORY_SPECS_V1: &[(&str, &[&str])] = &[
    ("systemd", SYSTEMD_CHILDREN_V1),
    ("sysusers.d", SYSUSERS_CHILDREN_V1),
    ("tmpfiles.d", TMPFILES_CHILDREN_V1),
    ("usr", USR_CHILDREN_V1),
    ("usr/libexec", LIBEXEC_CHILDREN_V1),
    ("usr/libexec/fe2o3", IMAGE_CHILDREN_V1),
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ObjectSnapshotV1 {
    device: u64,
    inode: u64,
    mode: u32,
    uid: u32,
    gid: u32,
    links: u64,
    byte_len: u64,
    modified_seconds: i64,
    modified_nanoseconds: u64,
    changed_seconds: i64,
    changed_nanoseconds: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ManifestEntryV1 {
    spec: FileSpecV1,
    byte_len: u64,
    sha256: [u8; 32],
}

impl PartialEq for FileSpecV1 {
    fn eq(&self, other: &Self) -> bool {
        self.source == other.source
            && self.install == other.install
            && self.mode == other.mode
            && self.max_bytes == other.max_bytes
    }
}

impl Eq for FileSpecV1 {}

struct SealedDeploymentFileV1 {
    entry: ManifestEntryV1,
    file: File,
}

/// Move-only, authority-free custody of one fully verified deployment bundle.
///
/// The source files have been copied into sealed anonymous descriptors. The original bundle may
/// be changed or removed after this value is returned without changing the admitted bytes.
///
/// ```compile_fail
/// use fe2o3_compiler_execution_deployment::VerifiedCompilerExecutionDeploymentV1;
/// fn require_clone<T: Clone>() {}
/// require_clone::<VerifiedCompilerExecutionDeploymentV1>();
/// ```
///
/// ```compile_fail
/// use std::os::fd::AsFd;
/// use fe2o3_compiler_execution_deployment::VerifiedCompilerExecutionDeploymentV1;
/// fn require_as_fd<T: AsFd>() {}
/// require_as_fd::<VerifiedCompilerExecutionDeploymentV1>();
/// ```
pub struct VerifiedCompilerExecutionDeploymentV1 {
    git_commit: String,
    target: String,
    manifest_sha256: [u8; 32],
    manifest: SealedDeploymentFileV1,
    files: Vec<SealedDeploymentFileV1>,
}

impl fmt::Debug for VerifiedCompilerExecutionDeploymentV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VerifiedCompilerExecutionDeploymentV1")
            .field("git_commit", &self.git_commit)
            .field("target", &self.target)
            .field("file_count", &self.files.len())
            .field("sealed_source_file_count", &self.sealed_source_file_count())
            .field("authority", &"verified-source-custody-only")
            .finish_non_exhaustive()
    }
}

impl VerifiedCompilerExecutionDeploymentV1 {
    /// Returns the exact source commit bound by both the caller and manifest.
    pub fn git_commit(&self) -> &str {
        &self.git_commit
    }

    /// Returns the exact static target bound by the manifest.
    pub fn target(&self) -> &str {
        &self.target
    }

    /// Returns the out-of-band-pinned manifest digest.
    pub const fn manifest_sha256(&self) -> [u8; 32] {
        self.manifest_sha256
    }

    /// Returns the exact number of sealed deployment content files.
    pub fn file_count(&self) -> usize {
        self.files.len()
    }

    /// Returns the manifest plus the exact number of sealed deployment content files.
    pub fn sealed_source_file_count(&self) -> usize {
        std::iter::once(&self.manifest).chain(&self.files).count()
    }
}

/// Result of writing one canonical install manifest into a clean bundle root.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompilerExecutionManifestGenerationV1 {
    sha256: [u8; 32],
    byte_len: u64,
}

impl CompilerExecutionManifestGenerationV1 {
    /// Returns the SHA-256 that must be distributed outside the bundle.
    pub const fn sha256(self) -> [u8; 32] {
        self.sha256
    }

    /// Returns the exact canonical manifest length.
    pub const fn byte_len(self) -> u64 {
        self.byte_len
    }
}

/// Generates and durably writes one canonical manifest into an otherwise complete clean bundle.
pub fn generate_compiler_execution_install_manifest_v1(
    bundle_root: &Path,
    git_commit: &str,
    target: &str,
) -> Result<CompilerExecutionManifestGenerationV1, DeploymentVerificationErrorV1> {
    parse_lower_hex_exact(git_commit, GIT_COMMIT_BYTES_V1, "git commit")?;
    if target != COMPILER_EXECUTION_DEPLOYMENT_TARGET_V1 {
        return Err(invalid(
            DeploymentVerificationErrorKindV1::InvalidManifest,
            "deployment target is not the sole V1 target",
        ));
    }
    let root = open_bundle_root(bundle_root)?;
    let root_snapshot = validate_directory(&root, None, "bundle root")?;
    verify_inventory(&root, root_snapshot, false)?;

    let mut entries = Vec::with_capacity(FILE_SPECS_V1.len());
    let mut contents = Vec::with_capacity(FILE_SPECS_V1.len());
    for spec in FILE_SPECS_V1 {
        let admitted = admit_source_file(&root, root_snapshot, spec, None)?;
        entries.push(ManifestEntryV1 {
            spec,
            byte_len: admitted.bytes.len() as u64,
            sha256: admitted.sha256,
        });
        contents.push(admitted.bytes);
    }
    validate_build_info(&contents[0], git_commit, target)?;
    validate_sha256sums(&contents[1], &entries)?;
    let manifest = serialize_manifest(git_commit, target, &entries);
    if manifest.len() > MANIFEST_MAX_BYTES_V1 {
        return Err(invalid(
            DeploymentVerificationErrorKindV1::InvalidManifest,
            "canonical deployment manifest exceeds its bound",
        ));
    }
    let digest: [u8; 32] = Sha256::digest(&manifest).into();
    verify_inventory(&root, root_snapshot, false)?;
    write_manifest(&root, &manifest)?;
    let published_root_snapshot = validate_directory(
        &root,
        Some((root_snapshot.uid, root_snapshot.gid)),
        "published bundle root",
    )?;
    if !same_directory_custody(root_snapshot, published_root_snapshot) {
        return Err(changed(
            "bundle root custody changed while publishing the install manifest",
        ));
    }
    verify_inventory(&root, published_root_snapshot, true)?;
    Ok(CompilerExecutionManifestGenerationV1 {
        sha256: digest,
        byte_len: manifest.len() as u64,
    })
}

/// Verifies a bundle against caller-supplied pins and retains sealed immutable source copies.
pub fn verify_compiler_execution_deployment_v1(
    bundle_root: &Path,
    expected_manifest_sha256: &str,
    expected_git_commit: &str,
) -> Result<VerifiedCompilerExecutionDeploymentV1, DeploymentVerificationErrorV1> {
    let expected_manifest =
        parse_lower_hex_exact(expected_manifest_sha256, 32, "expected manifest SHA-256")?;
    parse_lower_hex_exact(
        expected_git_commit,
        GIT_COMMIT_BYTES_V1,
        "expected git commit",
    )?;
    let root = open_bundle_root(bundle_root)?;
    let root_snapshot = validate_directory(&root, None, "bundle root")?;
    verify_inventory(&root, root_snapshot, true)?;

    let manifest_spec = MANIFEST_FILE_SPEC_V1;
    let manifest_file = admit_source_file(&root, root_snapshot, manifest_spec, None)?;
    if manifest_file.sha256 != expected_manifest.as_slice() {
        return Err(invalid(
            DeploymentVerificationErrorKindV1::ManifestMismatch,
            "install manifest does not match the out-of-band SHA-256",
        ));
    }
    let manifest = parse_manifest(&manifest_file.bytes, expected_git_commit)?;
    let sealed_manifest = seal_source(
        ManifestEntryV1 {
            spec: manifest_spec,
            byte_len: manifest_file.bytes.len() as u64,
            sha256: manifest_file.sha256,
        },
        &manifest_file.bytes,
    )?;

    let mut files = Vec::with_capacity(manifest.entries.len());
    let mut contents = Vec::with_capacity(manifest.entries.len());
    for entry in &manifest.entries {
        let admitted = admit_source_file(&root, root_snapshot, entry.spec, Some(entry))?;
        let sealed = seal_source(entry.clone(), &admitted.bytes)?;
        contents.push(admitted.bytes);
        files.push(sealed);
    }
    validate_build_info(&contents[0], expected_git_commit, &manifest.target)?;
    validate_sha256sums(&contents[1], &manifest.entries)?;
    verify_inventory(&root, root_snapshot, true)?;
    validate_sealed_file(&sealed_manifest)?;
    validate_sealed_files(&files)?;
    Ok(VerifiedCompilerExecutionDeploymentV1 {
        git_commit: manifest.git_commit,
        target: manifest.target,
        manifest_sha256: expected_manifest
            .try_into()
            .expect("32-byte manifest digest was checked"),
        manifest: sealed_manifest,
        files,
    })
}

/// Encodes one SHA-256 value as canonical lowercase hexadecimal.
pub fn encode_sha256_lower_hex_v1(digest: [u8; 32]) -> String {
    lower_hex(&digest)
}

struct ParsedManifestV1 {
    git_commit: String,
    target: String,
    entries: Vec<ManifestEntryV1>,
}

struct AdmittedSourceV1 {
    bytes: Vec<u8>,
    sha256: [u8; 32],
}

fn open_bundle_root(path: &Path) -> Result<File, DeploymentVerificationErrorV1> {
    openat2(
        rustix::fs::CWD,
        path,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
        Mode::empty(),
        ResolveFlags::NO_SYMLINKS | ResolveFlags::NO_MAGICLINKS,
    )
    .map(File::from)
    .map_err(|source| io_error("open deployment bundle root", source))
}

fn open_beneath(
    root: &File,
    path: &str,
    directory: bool,
) -> Result<File, DeploymentVerificationErrorV1> {
    let mut flags = OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW;
    if directory {
        flags |= OFlags::DIRECTORY;
    } else {
        flags |= OFlags::NONBLOCK;
    }
    openat2(
        root,
        path,
        flags,
        Mode::empty(),
        ResolveFlags::BENEATH
            | ResolveFlags::NO_SYMLINKS
            | ResolveFlags::NO_MAGICLINKS
            | ResolveFlags::NO_XDEV,
    )
    .map(File::from)
    .map_err(|source| io_error("open deployment object beneath retained root", source))
}

fn validate_directory(
    directory: &File,
    expected_owner: Option<(u32, u32)>,
    role: &'static str,
) -> Result<ObjectSnapshotV1, DeploymentVerificationErrorV1> {
    validate_directory_mode(directory, expected_owner, DIRECTORY_MODE_V1, role)
}

fn validate_directory_mode(
    directory: &File,
    expected_owner: Option<(u32, u32)>,
    expected_mode: u32,
    role: &'static str,
) -> Result<ObjectSnapshotV1, DeploymentVerificationErrorV1> {
    let descriptor_flags = rustix::io::fcntl_getfd(directory)
        .map_err(|source| io_error("inspect deployment directory descriptor flags", source))?;
    let status = rustix::fs::fcntl_getfl(directory)
        .map_err(|source| io_error("inspect deployment directory status flags", source))?;
    let stat = fstat(directory)
        .map_err(|source| io_error("inspect deployment directory metadata", source))?;
    let snapshot = snapshot(&stat);
    if descriptor_flags != rustix::io::FdFlags::CLOEXEC
        || status & OFlags::ACCMODE != OFlags::RDONLY
        || status.contains(OFlags::PATH)
        || FileType::from_raw_mode(snapshot.mode) != FileType::Directory
        || snapshot.mode & 0o7777 != expected_mode
        || snapshot.links == 0
        || expected_owner.is_some_and(|owner| owner != (snapshot.uid, snapshot.gid))
    {
        return Err(invalid(
            DeploymentVerificationErrorKindV1::InvalidMetadata,
            format!(
                "{role} has invalid metadata: mode={:04o} owner={}:{} links={} fd_flags={descriptor_flags:?} status_flags={status:?}",
                snapshot.mode & 0o7777,
                snapshot.uid,
                snapshot.gid,
                snapshot.links,
            ),
        ));
    }
    require_no_xattrs(directory, role)?;
    Ok(snapshot)
}

fn verify_inventory(
    root: &File,
    root_snapshot: ObjectSnapshotV1,
    manifest_present: bool,
) -> Result<(), DeploymentVerificationErrorV1> {
    if snapshot(&fstat(root).map_err(|source| io_error("reinspect bundle root", source))?)
        != root_snapshot
    {
        return Err(changed("bundle root changed while verifying inventory"));
    }
    let root_children = if manifest_present {
        ROOT_CHILDREN_WITH_MANIFEST_V1
    } else {
        ROOT_CHILDREN_WITHOUT_MANIFEST_V1
    };
    verify_directory_children(root, root_children, "bundle root")?;
    let owner = (root_snapshot.uid, root_snapshot.gid);
    for &(path, expected_children) in DIRECTORY_SPECS_V1 {
        let directory = open_beneath(root, path, true)?;
        let initial = validate_directory(&directory, Some(owner), "bundle subdirectory")?;
        verify_directory_children(&directory, expected_children, "bundle subdirectory")?;
        if snapshot(
            &fstat(&directory)
                .map_err(|source| io_error("reinspect bundle subdirectory", source))?,
        ) != initial
        {
            return Err(changed("bundle subdirectory changed during enumeration"));
        }
        let reopened = open_beneath(root, path, true)?;
        if snapshot(
            &fstat(&reopened)
                .map_err(|source| io_error("reinspect canonical bundle subdirectory", source))?,
        ) != initial
        {
            return Err(changed("bundle subdirectory pathname changed"));
        }
    }
    verify_directory_children(root, root_children, "bundle root")?;
    if snapshot(&fstat(root).map_err(|source| io_error("reinspect bundle root", source))?)
        != root_snapshot
    {
        return Err(changed("bundle root changed after verifying inventory"));
    }
    Ok(())
}

fn same_directory_custody(left: ObjectSnapshotV1, right: ObjectSnapshotV1) -> bool {
    left.device == right.device
        && left.inode == right.inode
        && left.mode == right.mode
        && left.uid == right.uid
        && left.gid == right.gid
        && left.links == right.links
}

fn verify_directory_children(
    directory: &File,
    expected: &[&str],
    role: &'static str,
) -> Result<(), DeploymentVerificationErrorV1> {
    let observed = canonical_directory_children(directory, role)?;
    let expected: Vec<&OsStr> = expected.iter().map(OsStr::new).collect();
    if observed.len() != expected.len()
        || observed
            .iter()
            .zip(expected)
            .any(|(observed, expected)| observed != expected)
    {
        return Err(invalid(
            DeploymentVerificationErrorKindV1::InvalidInventory,
            format!("{role} has an extra, missing, or substituted entry"),
        ));
    }
    Ok(())
}

fn canonical_directory_children(
    directory: &File,
    role: &'static str,
) -> Result<Vec<OsString>, DeploymentVerificationErrorV1> {
    let scan = openat(
        directory,
        ".",
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC | OFlags::NOFOLLOW,
        Mode::empty(),
    )
    .map_err(|source| io_error("retain deployment directory for enumeration", source))?;
    let mut entries = rustix::fs::Dir::read_from(&scan)
        .map_err(|source| io_error("enumerate deployment directory", source))?;
    let mut observed = Vec::<OsString>::new();
    for entry in &mut entries {
        let entry = entry.map_err(|source| io_error("read deployment directory entry", source))?;
        let bytes = entry.file_name().to_bytes();
        if matches!(bytes, b"." | b"..") {
            continue;
        }
        if bytes.is_empty() || bytes.contains(&0) || std::str::from_utf8(bytes).is_err() {
            return Err(invalid(
                DeploymentVerificationErrorKindV1::InvalidInventory,
                format!("{role} contains a noncanonical name"),
            ));
        }
        observed.push(OsString::from_vec(bytes.to_vec()));
    }
    observed.sort_by(|left, right| left.as_bytes().cmp(right.as_bytes()));
    Ok(observed)
}

fn admit_source_file(
    root: &File,
    root_snapshot: ObjectSnapshotV1,
    spec: FileSpecV1,
    expected: Option<&ManifestEntryV1>,
) -> Result<AdmittedSourceV1, DeploymentVerificationErrorV1> {
    let mut file = open_beneath(root, spec.source, false)?;
    let initial = validate_source_metadata(&file, root_snapshot, spec, expected)?;
    let first = read_bounded(&mut file, spec.max_bytes)?;
    if snapshot(&fstat(&file).map_err(|source| io_error("reinspect deployment file", source))?)
        != initial
    {
        return Err(changed("deployment file changed after first read"));
    }
    let first_hash: [u8; 32] = Sha256::digest(&first).into();
    let second = read_bounded(&mut file, spec.max_bytes)?;
    let second_hash: [u8; 32] = Sha256::digest(&second).into();
    if first != second || first_hash != second_hash {
        return Err(changed(
            "deployment file bytes changed between independent reads",
        ));
    }
    if snapshot(&fstat(&file).map_err(|source| io_error("reinspect deployment file", source))?)
        != initial
    {
        return Err(changed("deployment file changed after second read"));
    }
    if let Some(expected) = expected
        && (first.len() as u64 != expected.byte_len || first_hash != expected.sha256)
    {
        return Err(invalid(
            DeploymentVerificationErrorKindV1::ContentMismatch,
            format!("deployment file {} differs from its manifest", spec.source),
        ));
    }
    let reopened = open_beneath(root, spec.source, false)?;
    if snapshot(
        &fstat(&reopened)
            .map_err(|source| io_error("reinspect canonical deployment file", source))?,
    ) != initial
    {
        return Err(changed("deployment file pathname changed during admission"));
    }
    Ok(AdmittedSourceV1 {
        bytes: first,
        sha256: first_hash,
    })
}

fn validate_source_metadata(
    file: &File,
    root_snapshot: ObjectSnapshotV1,
    spec: FileSpecV1,
    expected: Option<&ManifestEntryV1>,
) -> Result<ObjectSnapshotV1, DeploymentVerificationErrorV1> {
    let descriptor_flags = rustix::io::fcntl_getfd(file)
        .map_err(|source| io_error("inspect deployment file descriptor flags", source))?;
    let status = rustix::fs::fcntl_getfl(file)
        .map_err(|source| io_error("inspect deployment file status flags", source))?;
    let stat =
        fstat(file).map_err(|source| io_error("inspect deployment file metadata", source))?;
    let snapshot = snapshot(&stat);
    let forbidden = OFlags::APPEND | OFlags::ASYNC | OFlags::DIRECT | OFlags::PATH;
    if descriptor_flags != rustix::io::FdFlags::CLOEXEC
        || status & OFlags::ACCMODE != OFlags::RDONLY
        || status.intersects(forbidden)
        || FileType::from_raw_mode(snapshot.mode) != FileType::RegularFile
        || snapshot.mode & 0o7777 != spec.mode
        || (snapshot.uid, snapshot.gid) != (root_snapshot.uid, root_snapshot.gid)
        || snapshot.links != 1
        || snapshot.byte_len > spec.max_bytes
        || expected.is_some_and(|entry| entry.byte_len != snapshot.byte_len)
    {
        return Err(invalid(
            DeploymentVerificationErrorKindV1::InvalidMetadata,
            format!("deployment file {} has invalid metadata", spec.source),
        ));
    }
    require_no_xattrs(file, "deployment file")?;
    Ok(snapshot)
}

fn require_no_xattrs(
    file: &impl AsFd,
    role: &'static str,
) -> Result<(), DeploymentVerificationErrorV1> {
    let mut attributes = [0_u8; 1];
    match flistxattr(file, &mut attributes) {
        Ok(0) => Ok(()),
        Ok(_) | Err(rustix::io::Errno::RANGE) => Err(invalid(
            DeploymentVerificationErrorKindV1::ForbiddenAttributes,
            format!("{role} carries an extended attribute"),
        )),
        Err(source) => Err(io_error("inspect deployment extended attributes", source)),
    }
}

fn read_bounded(file: &mut File, max_bytes: u64) -> Result<Vec<u8>, DeploymentVerificationErrorV1> {
    file.seek(SeekFrom::Start(0))
        .map_err(|source| std_io_error("rewind deployment file", source))?;
    let mut bytes = Vec::new();
    file.take(max_bytes + 1)
        .read_to_end(&mut bytes)
        .map_err(|source| std_io_error("read deployment file", source))?;
    if bytes.len() as u64 > max_bytes {
        return Err(invalid(
            DeploymentVerificationErrorKindV1::InvalidMetadata,
            "deployment file exceeds its byte bound",
        ));
    }
    Ok(bytes)
}

fn write_manifest(root: &File, bytes: &[u8]) -> Result<(), DeploymentVerificationErrorV1> {
    let descriptor = openat(
        root,
        COMPILER_EXECUTION_INSTALL_MANIFEST_NAME_V1,
        OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::CLOEXEC | OFlags::NOFOLLOW,
        Mode::RUSR | Mode::WUSR,
    )
    .map_err(|source| io_error("create deployment install manifest", source))?;
    let mut file = File::from(descriptor);
    file.write_all(bytes)
        .map_err(|source| std_io_error("write deployment install manifest", source))?;
    file.sync_all()
        .map_err(|source| std_io_error("sync deployment install manifest", source))?;
    rustix::fs::fchmod(&file, Mode::from_raw_mode(MANIFEST_MODE_V1))
        .map_err(|source| io_error("set deployment install manifest mode", source))?;
    file.sync_all()
        .map_err(|source| std_io_error("resync deployment install manifest", source))?;
    root.sync_all()
        .map_err(|source| std_io_error("sync deployment bundle root", source))
}

fn serialize_manifest(git_commit: &str, target: &str, entries: &[ManifestEntryV1]) -> Vec<u8> {
    let mut manifest = String::new();
    writeln!(&mut manifest, "{MANIFEST_HEADER_V1}").expect("string writes cannot fail");
    writeln!(&mut manifest, "git_commit\t{git_commit}").expect("string writes cannot fail");
    writeln!(&mut manifest, "target\t{target}").expect("string writes cannot fail");
    writeln!(&mut manifest, "entry_count\t{}", entries.len()).expect("string writes cannot fail");
    for entry in entries {
        writeln!(
            &mut manifest,
            "file\t{}\t{}\t{:04o}\t{}\t{}",
            entry.spec.source,
            entry.spec.install,
            entry.spec.mode,
            entry.byte_len,
            lower_hex(&entry.sha256),
        )
        .expect("string writes cannot fail");
    }
    manifest.into_bytes()
}

fn parse_manifest(
    bytes: &[u8],
    expected_git_commit: &str,
) -> Result<ParsedManifestV1, DeploymentVerificationErrorV1> {
    if bytes.is_empty()
        || bytes.len() > MANIFEST_MAX_BYTES_V1
        || !bytes.ends_with(b"\n")
        || bytes.contains(&b'\r')
        || bytes.contains(&0)
    {
        return Err(invalid(
            DeploymentVerificationErrorKindV1::InvalidManifest,
            "install manifest has a noncanonical envelope",
        ));
    }
    let text = std::str::from_utf8(bytes).map_err(|_| {
        invalid(
            DeploymentVerificationErrorKindV1::InvalidManifest,
            "install manifest is not UTF-8",
        )
    })?;
    let mut lines = text
        .strip_suffix('\n')
        .expect("final newline checked")
        .split('\n');
    if lines.next() != Some(MANIFEST_HEADER_V1) {
        return Err(invalid(
            DeploymentVerificationErrorKindV1::InvalidManifest,
            "install manifest header is invalid",
        ));
    }
    let git_commit = parse_single_field(lines.next(), "git_commit")?;
    parse_lower_hex_exact(git_commit, GIT_COMMIT_BYTES_V1, "manifest git commit")?;
    if git_commit != expected_git_commit {
        return Err(invalid(
            DeploymentVerificationErrorKindV1::CommitMismatch,
            "install manifest commit differs from the out-of-band commit",
        ));
    }
    let target = parse_single_field(lines.next(), "target")?;
    if target != COMPILER_EXECUTION_DEPLOYMENT_TARGET_V1 {
        return Err(invalid(
            DeploymentVerificationErrorKindV1::InvalidManifest,
            "install manifest target is invalid",
        ));
    }
    let count = parse_single_field(lines.next(), "entry_count")?;
    if parse_decimal(count, "manifest entry count")? != FILE_SPECS_V1.len() as u64 {
        return Err(invalid(
            DeploymentVerificationErrorKindV1::InvalidManifest,
            "install manifest entry count is invalid",
        ));
    }
    let mut entries = Vec::with_capacity(FILE_SPECS_V1.len());
    for spec in FILE_SPECS_V1 {
        let line = lines.next().ok_or_else(|| {
            invalid(
                DeploymentVerificationErrorKindV1::InvalidManifest,
                "install manifest is missing a file entry",
            )
        })?;
        let fields: Vec<&str> = line.split('\t').collect();
        if fields.len() != 6
            || fields[0] != "file"
            || fields[1] != spec.source
            || fields[2] != spec.install
            || fields[3] != format!("{:04o}", spec.mode)
        {
            return Err(invalid(
                DeploymentVerificationErrorKindV1::InvalidManifest,
                "install manifest file inventory is not canonical",
            ));
        }
        let byte_len = parse_decimal(fields[4], "manifest file length")?;
        if byte_len > spec.max_bytes {
            return Err(invalid(
                DeploymentVerificationErrorKindV1::InvalidManifest,
                "install manifest file length exceeds its bound",
            ));
        }
        let sha256: [u8; 32] = parse_lower_hex_exact(fields[5], 32, "manifest file SHA-256")?
            .try_into()
            .expect("32-byte file digest was checked");
        entries.push(ManifestEntryV1 {
            spec,
            byte_len,
            sha256,
        });
    }
    if lines.next().is_some() {
        return Err(invalid(
            DeploymentVerificationErrorKindV1::InvalidManifest,
            "install manifest has trailing records",
        ));
    }
    Ok(ParsedManifestV1 {
        git_commit: git_commit.to_owned(),
        target: target.to_owned(),
        entries,
    })
}

fn parse_single_field<'a>(
    line: Option<&'a str>,
    expected_name: &str,
) -> Result<&'a str, DeploymentVerificationErrorV1> {
    let line = line.ok_or_else(|| {
        invalid(
            DeploymentVerificationErrorKindV1::InvalidManifest,
            "install manifest is truncated",
        )
    })?;
    let mut fields = line.split('\t');
    let name = fields.next();
    let value = fields.next();
    if name != Some(expected_name) || value.is_none() || fields.next().is_some() {
        return Err(invalid(
            DeploymentVerificationErrorKindV1::InvalidManifest,
            "install manifest field is noncanonical",
        ));
    }
    Ok(value.expect("manifest value presence checked"))
}

fn parse_decimal(value: &str, role: &'static str) -> Result<u64, DeploymentVerificationErrorV1> {
    if value.is_empty()
        || !value.bytes().all(|byte| byte.is_ascii_digit())
        || (value.len() > 1 && value.starts_with('0'))
    {
        return Err(invalid(
            DeploymentVerificationErrorKindV1::InvalidManifest,
            format!("{role} is not canonical decimal"),
        ));
    }
    value.parse().map_err(|_| {
        invalid(
            DeploymentVerificationErrorKindV1::InvalidManifest,
            format!("{role} overflows"),
        )
    })
}

fn parse_lower_hex_exact(
    value: &str,
    byte_len: usize,
    role: &'static str,
) -> Result<Vec<u8>, DeploymentVerificationErrorV1> {
    if value.len() != byte_len * 2
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(invalid(
            DeploymentVerificationErrorKindV1::InvalidDigest,
            format!("{role} is not canonical lowercase hexadecimal"),
        ));
    }
    let mut bytes = Vec::with_capacity(byte_len);
    for pair in value.as_bytes().chunks_exact(2) {
        bytes.push((hex_nibble(pair[0]) << 4) | hex_nibble(pair[1]));
    }
    Ok(bytes)
}

fn hex_nibble(byte: u8) -> u8 {
    match byte {
        b'0'..=b'9' => byte - b'0',
        b'a'..=b'f' => byte - b'a' + 10,
        _ => unreachable!("hex input was validated"),
    }
}

fn lower_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(char::from(HEX[usize::from(byte >> 4)]));
        encoded.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    encoded
}

fn random_staging_name(
    prefix: &str,
    operation: &'static str,
) -> Result<String, DeploymentVerificationErrorV1> {
    let mut random = [0_u8; 16];
    let mut filled = 0;
    while filled < random.len() {
        let count =
            rustix::rand::getrandom(&mut random[filled..], rustix::rand::GetRandomFlags::empty())
                .map_err(|source| io_error(operation, source))?;
        if count == 0 {
            return Err(invalid(
                DeploymentVerificationErrorKindV1::Io,
                "Linux getrandom returned no staging-name bytes",
            ));
        }
        filled += count;
    }
    Ok(format!("{prefix}{}", lower_hex(&random)))
}

fn validate_build_info(
    bytes: &[u8],
    git_commit: &str,
    target: &str,
) -> Result<(), DeploymentVerificationErrorV1> {
    let text = std::str::from_utf8(bytes).map_err(|_| {
        invalid(
            DeploymentVerificationErrorKindV1::ContentMismatch,
            "BUILD-INFO is not UTF-8",
        )
    })?;
    let lines: Vec<&str> = text.strip_suffix('\n').unwrap_or("").split('\n').collect();
    if lines.len() != 4
        || lines[0] != "schema_version=1"
        || lines[1] != format!("git_commit={git_commit}")
        || !lines[2].starts_with("source_date_epoch=")
        || lines[3] != format!("target={target}")
    {
        return Err(invalid(
            DeploymentVerificationErrorKindV1::ContentMismatch,
            "BUILD-INFO does not match the manifest identity",
        ));
    }
    let epoch = lines[2]
        .strip_prefix("source_date_epoch=")
        .expect("prefix checked");
    if parse_decimal(epoch, "source date epoch")? == 0 {
        return Err(invalid(
            DeploymentVerificationErrorKindV1::ContentMismatch,
            "BUILD-INFO source date epoch is zero",
        ));
    }
    Ok(())
}

fn validate_sha256sums(
    bytes: &[u8],
    entries: &[ManifestEntryV1],
) -> Result<(), DeploymentVerificationErrorV1> {
    let mut expected = String::new();
    for entry in entries {
        if entry.spec.source == "SHA256SUMS" {
            continue;
        }
        writeln!(
            &mut expected,
            "{}  ./{}",
            lower_hex(&entry.sha256),
            entry.spec.source
        )
        .expect("string writes cannot fail");
    }
    if bytes != expected.as_bytes() {
        return Err(invalid(
            DeploymentVerificationErrorKindV1::ContentMismatch,
            "SHA256SUMS disagrees with the canonical install manifest",
        ));
    }
    Ok(())
}

fn seal_source(
    entry: ManifestEntryV1,
    bytes: &[u8],
) -> Result<SealedDeploymentFileV1, DeploymentVerificationErrorV1> {
    let descriptor = memfd_create(
        c"fe2o3-deployment-source-v1",
        MemfdFlags::CLOEXEC | MemfdFlags::ALLOW_SEALING,
    )
    .map_err(|source| io_error("create sealed deployment source", source))?;
    let mut file = File::from(descriptor);
    file.write_all(bytes)
        .map_err(|source| std_io_error("copy sealed deployment source", source))?;
    file.flush()
        .map_err(|source| std_io_error("flush sealed deployment source", source))?;
    rustix::fs::fchmod(&file, Mode::from_raw_mode(entry.spec.mode))
        .map_err(|source| io_error("set sealed deployment source mode", source))?;
    fcntl_add_seals(
        &file,
        SealFlags::WRITE | SealFlags::GROW | SealFlags::SHRINK | SealFlags::SEAL,
    )
    .map_err(|source| io_error("seal deployment source", source))?;
    Ok(SealedDeploymentFileV1 { entry, file })
}

fn validate_sealed_files(
    files: &[SealedDeploymentFileV1],
) -> Result<(), DeploymentVerificationErrorV1> {
    for retained in files {
        validate_sealed_file(retained)?;
    }
    Ok(())
}

fn validate_sealed_file(
    retained: &SealedDeploymentFileV1,
) -> Result<(), DeploymentVerificationErrorV1> {
    let expected_seals = SealFlags::WRITE | SealFlags::GROW | SealFlags::SHRINK | SealFlags::SEAL;
    let stat = fstat(&retained.file)
        .map_err(|source| io_error("inspect sealed deployment source", source))?;
    if FileType::from_raw_mode(stat.st_mode) != FileType::RegularFile
        || stat.st_mode & 0o7777 != retained.entry.spec.mode
        || u64::try_from(stat.st_size).unwrap_or(u64::MAX) != retained.entry.byte_len
        || fcntl_get_seals(&retained.file)
            .map_err(|source| io_error("inspect deployment source seals", source))?
            != expected_seals
    {
        return Err(changed("sealed deployment source changed after admission"));
    }
    Ok(())
}

fn snapshot(stat: &rustix::fs::Stat) -> ObjectSnapshotV1 {
    ObjectSnapshotV1 {
        device: stat.st_dev,
        inode: stat.st_ino,
        mode: stat.st_mode,
        uid: stat.st_uid,
        gid: stat.st_gid,
        links: stat.st_nlink,
        byte_len: u64::try_from(stat.st_size).unwrap_or(u64::MAX),
        modified_seconds: stat.st_mtime,
        modified_nanoseconds: stat.st_mtime_nsec,
        changed_seconds: stat.st_ctime,
        changed_nanoseconds: stat.st_ctime_nsec,
    }
}

/// Stable category for a deployment generation or verification failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum DeploymentVerificationErrorKindV1 {
    /// A caller-supplied digest or commit is not canonical lowercase hexadecimal.
    InvalidDigest,
    /// The manifest digest differs from the out-of-band pin.
    ManifestMismatch,
    /// The canonical manifest grammar, target, order, or bounds are invalid.
    InvalidManifest,
    /// The manifest commit differs from the caller-supplied commit.
    CommitMismatch,
    /// The bundle contains an extra, missing, or substituted path.
    InvalidInventory,
    /// A retained object has the wrong type, mode, owner, links, or length.
    InvalidMetadata,
    /// A retained object has one or more extended attributes.
    ForbiddenAttributes,
    /// File bytes disagree with the manifest or cross-file identity.
    ContentMismatch,
    /// A retained object or pathname changed during admission.
    InputChanged,
    /// A bounded operating-system operation failed.
    Io,
    /// A pinned qualification base image is outside the sole supported V1 profile.
    InvalidQualificationBase,
    /// The qualification process could not establish or retain its private mount namespace.
    InvalidQualificationIsolation,
    /// A read-only base or disposable overlay mount failed admission or cleanup.
    InvalidQualificationMount,
    /// A production root operation did not run with effective UID 0.
    InsufficientPrivilege,
    /// Atomic publication happened, but its durability result is ambiguous.
    PublicationAmbiguous,
    /// Bounded descriptor-relative staging cleanup failed.
    CleanupFailed,
    /// A deterministic test interrupted one installation transaction boundary.
    InjectedFailure,
}

/// Stable deployment generation, verification, or installation error.
#[derive(Debug)]
pub struct DeploymentVerificationErrorV1 {
    kind: DeploymentVerificationErrorKindV1,
    message: String,
    source: Option<io::Error>,
}

impl DeploymentVerificationErrorV1 {
    /// Returns the stable error category.
    pub const fn kind(&self) -> DeploymentVerificationErrorKindV1 {
        self.kind
    }
}

impl fmt::Display for DeploymentVerificationErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(source) = &self.source {
            write!(formatter, "{}: {source}", self.message)
        } else {
            formatter.write_str(&self.message)
        }
    }
}

impl Error for DeploymentVerificationErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.source
            .as_ref()
            .map(|source| source as &(dyn Error + 'static))
    }
}

fn invalid(
    kind: DeploymentVerificationErrorKindV1,
    message: impl Into<String>,
) -> DeploymentVerificationErrorV1 {
    DeploymentVerificationErrorV1 {
        kind,
        message: message.into(),
        source: None,
    }
}

fn changed(message: impl Into<String>) -> DeploymentVerificationErrorV1 {
    invalid(DeploymentVerificationErrorKindV1::InputChanged, message)
}

fn io_error(operation: &'static str, source: rustix::io::Errno) -> DeploymentVerificationErrorV1 {
    std_io_error(operation, source.into())
}

fn std_io_error(operation: &'static str, source: io::Error) -> DeploymentVerificationErrorV1 {
    DeploymentVerificationErrorV1 {
        kind: DeploymentVerificationErrorKindV1::Io,
        message: operation.to_owned(),
        source: Some(source),
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::os::unix::ffi::OsStringExt;
    use std::os::unix::fs::{PermissionsExt, symlink};
    use std::path::PathBuf;

    use rustix::fs::XattrFlags;

    use super::*;

    const COMMIT: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const OTHER_COMMIT: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

    struct Fixture {
        root: tempfile::TempDir,
    }

    impl Fixture {
        fn new() -> Self {
            let root = tempfile::tempdir().unwrap();
            for (path, _) in DIRECTORY_SPECS_V1 {
                fs::create_dir_all(root.path().join(path)).unwrap();
            }
            fs::set_permissions(root.path(), fs::Permissions::from_mode(DIRECTORY_MODE_V1))
                .unwrap();
            for (path, _) in DIRECTORY_SPECS_V1 {
                fs::set_permissions(
                    root.path().join(path),
                    fs::Permissions::from_mode(DIRECTORY_MODE_V1),
                )
                .unwrap();
            }
            for spec in FILE_SPECS_V1 {
                if spec.source == "SHA256SUMS" {
                    continue;
                }
                let bytes = if spec.source == "BUILD-INFO" {
                    format!(
                        "schema_version=1\ngit_commit={COMMIT}\nsource_date_epoch=1788120406\ntarget={}\n",
                        COMPILER_EXECUTION_DEPLOYMENT_TARGET_V1
                    )
                    .into_bytes()
                } else {
                    format!("exact fixture bytes for {}\n", spec.source).into_bytes()
                };
                fs::write(root.path().join(spec.source), bytes).unwrap();
                fs::set_permissions(
                    root.path().join(spec.source),
                    fs::Permissions::from_mode(spec.mode),
                )
                .unwrap();
            }
            let entries = fixture_entries(root.path());
            let mut sums = String::new();
            for entry in &entries {
                if entry.spec.source != "SHA256SUMS" {
                    writeln!(
                        &mut sums,
                        "{}  ./{}",
                        lower_hex(&entry.sha256),
                        entry.spec.source
                    )
                    .unwrap();
                }
            }
            fs::write(root.path().join("SHA256SUMS"), sums).unwrap();
            fs::set_permissions(
                root.path().join("SHA256SUMS"),
                fs::Permissions::from_mode(0o444),
            )
            .unwrap();
            Self { root }
        }

        fn generate(&self) -> CompilerExecutionManifestGenerationV1 {
            generate_compiler_execution_install_manifest_v1(
                self.root.path(),
                COMMIT,
                COMPILER_EXECUTION_DEPLOYMENT_TARGET_V1,
            )
            .unwrap()
        }

        fn verify(
            &self,
            digest: [u8; 32],
        ) -> Result<VerifiedCompilerExecutionDeploymentV1, DeploymentVerificationErrorV1> {
            verify_compiler_execution_deployment_v1(self.root.path(), &lower_hex(&digest), COMMIT)
        }

        fn path(&self, relative: &str) -> std::path::PathBuf {
            self.root.path().join(relative)
        }
    }

    fn fixture_entries(root: &Path) -> Vec<ManifestEntryV1> {
        FILE_SPECS_V1
            .into_iter()
            .map(|spec| {
                if spec.source == "SHA256SUMS" && !root.join(spec.source).exists() {
                    return ManifestEntryV1 {
                        spec,
                        byte_len: 0,
                        sha256: Sha256::digest([]).into(),
                    };
                }
                let bytes = fs::read(root.join(spec.source)).unwrap();
                ManifestEntryV1 {
                    spec,
                    byte_len: bytes.len() as u64,
                    sha256: Sha256::digest(bytes).into(),
                }
            })
            .collect()
    }

    #[test]
    fn canonical_bundle_verifies_into_move_only_sealed_custody() {
        let fixture = Fixture::new();
        let generation = fixture.generate();
        let mut verified = fixture.verify(generation.sha256()).unwrap();
        assert_eq!(verified.git_commit(), COMMIT);
        assert_eq!(verified.target(), COMPILER_EXECUTION_DEPLOYMENT_TARGET_V1);
        assert_eq!(
            verified.file_count(),
            COMPILER_EXECUTION_INSTALL_FILE_COUNT_V1
        );
        assert_eq!(verified.sealed_source_file_count(), 14);
        assert_eq!(verified.manifest_sha256(), generation.sha256());
        assert_eq!(
            verified.manifest.entry.spec.source,
            COMPILER_EXECUTION_INSTALL_MANIFEST_NAME_V1
        );

        let expected_build_info = fs::read(fixture.path("BUILD-INFO")).unwrap();
        drop(fixture);
        let retained = &mut verified.files[0].file;
        retained.seek(SeekFrom::Start(0)).unwrap();
        let mut bytes = Vec::new();
        retained.read_to_end(&mut bytes).unwrap();
        assert_eq!(bytes, expected_build_info);
        validate_sealed_file(&verified.manifest).unwrap();
        validate_sealed_files(&verified.files).unwrap();
    }

    #[test]
    fn out_of_band_manifest_and_commit_pins_are_independent() {
        let fixture = Fixture::new();
        let generation = fixture.generate();
        let mut wrong_digest = generation.sha256();
        wrong_digest[0] ^= 1;
        assert_eq!(
            fixture.verify(wrong_digest).unwrap_err().kind(),
            DeploymentVerificationErrorKindV1::ManifestMismatch
        );
        assert_eq!(
            verify_compiler_execution_deployment_v1(
                fixture.root.path(),
                &lower_hex(&generation.sha256()),
                OTHER_COMMIT,
            )
            .unwrap_err()
            .kind(),
            DeploymentVerificationErrorKindV1::CommitMismatch
        );
    }

    #[test]
    fn malformed_reordered_and_trailing_manifest_records_are_rejected_under_a_fresh_pin() {
        for mutation in [
            |bytes: &mut Vec<u8>| bytes[0] ^= 1,
            |bytes: &mut Vec<u8>| {
                bytes.extend_from_slice(b"trailing\trecord\n");
            },
        ] {
            let fixture = Fixture::new();
            fixture.generate();
            let path = fixture.path(COMPILER_EXECUTION_INSTALL_MANIFEST_NAME_V1);
            fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
            let mut bytes = fs::read(&path).unwrap();
            mutation(&mut bytes);
            fs::write(&path, &bytes).unwrap();
            fs::set_permissions(&path, fs::Permissions::from_mode(MANIFEST_MODE_V1)).unwrap();
            let digest: [u8; 32] = Sha256::digest(bytes).into();
            assert_eq!(
                fixture.verify(digest).unwrap_err().kind(),
                DeploymentVerificationErrorKindV1::InvalidManifest
            );
        }
    }

    #[test]
    fn extra_and_non_utf8_inventory_entries_are_rejected() {
        let fixture = Fixture::new();
        let generation = fixture.generate();
        fs::write(fixture.path("extra"), b"hostile").unwrap();
        assert_eq!(
            fixture.verify(generation.sha256()).unwrap_err().kind(),
            DeploymentVerificationErrorKindV1::InvalidInventory
        );

        let fixture = Fixture::new();
        let generation = fixture.generate();
        let hostile = OsString::from_vec(vec![0xff]);
        fs::write(fixture.root.path().join(hostile), b"hostile").unwrap();
        assert_eq!(
            fixture.verify(generation.sha256()).unwrap_err().kind(),
            DeploymentVerificationErrorKindV1::InvalidInventory
        );
    }

    #[test]
    fn symlink_hardlink_and_mode_substitution_are_rejected() {
        let fixture = Fixture::new();
        let generation = fixture.generate();
        let service = fixture.path("systemd/fe2o3-compiler-execution.service");
        fs::set_permissions(&service, fs::Permissions::from_mode(0o600)).unwrap();
        fs::write(&service, b"hostile").unwrap();
        fs::remove_file(&service).unwrap();
        symlink("fe2o3-compiler-execution.socket", &service).unwrap();
        assert!(fixture.verify(generation.sha256()).is_err());

        let fixture = Fixture::new();
        let generation = fixture.generate();
        let external = tempfile::tempdir().unwrap();
        fs::hard_link(fixture.path("BUILD-INFO"), external.path().join("alias")).unwrap();
        assert_eq!(
            fixture.verify(generation.sha256()).unwrap_err().kind(),
            DeploymentVerificationErrorKindV1::InvalidMetadata
        );

        let fixture = Fixture::new();
        let generation = fixture.generate();
        fs::set_permissions(
            fixture.path("BUILD-INFO"),
            fs::Permissions::from_mode(0o440),
        )
        .unwrap();
        assert_eq!(
            fixture.verify(generation.sha256()).unwrap_err().kind(),
            DeploymentVerificationErrorKindV1::InvalidMetadata
        );
    }

    #[test]
    fn same_length_content_and_extended_attribute_substitution_are_rejected() {
        let fixture = Fixture::new();
        let generation = fixture.generate();
        let path = fixture.path("systemd/fe2o3-compiler-execution.socket");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        let mut bytes = fs::read(&path).unwrap();
        bytes[0] ^= 1;
        fs::write(&path, bytes).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o444)).unwrap();
        assert_eq!(
            fixture.verify(generation.sha256()).unwrap_err().kind(),
            DeploymentVerificationErrorKindV1::ContentMismatch
        );

        let fixture = Fixture::new();
        let generation = fixture.generate();
        let build_info = fixture.path("BUILD-INFO");
        fs::set_permissions(&build_info, fs::Permissions::from_mode(0o600)).unwrap();
        let file = File::options()
            .read(true)
            .write(true)
            .open(&build_info)
            .unwrap();
        rustix::fs::fsetxattr(&file, "user.fe2o3-test", b"1", XattrFlags::empty()).unwrap();
        fs::set_permissions(&build_info, fs::Permissions::from_mode(0o444)).unwrap();
        assert_eq!(
            fixture.verify(generation.sha256()).unwrap_err().kind(),
            DeploymentVerificationErrorKindV1::ForbiddenAttributes
        );
    }

    #[test]
    fn generation_rejects_build_info_and_sha256sums_that_disagree() {
        let fixture = Fixture::new();
        let build_info = fixture.path("BUILD-INFO");
        fs::set_permissions(&build_info, fs::Permissions::from_mode(0o600)).unwrap();
        let bytes = fs::read(&build_info).unwrap();
        fs::write(
            &build_info,
            String::from_utf8(bytes)
                .unwrap()
                .replace(COMMIT, OTHER_COMMIT),
        )
        .unwrap();
        fs::set_permissions(&build_info, fs::Permissions::from_mode(0o444)).unwrap();
        assert_eq!(
            generate_compiler_execution_install_manifest_v1(
                fixture.root.path(),
                COMMIT,
                COMPILER_EXECUTION_DEPLOYMENT_TARGET_V1,
            )
            .unwrap_err()
            .kind(),
            DeploymentVerificationErrorKindV1::ContentMismatch
        );

        let fixture = Fixture::new();
        let sums = fixture.path("SHA256SUMS");
        fs::set_permissions(&sums, fs::Permissions::from_mode(0o600)).unwrap();
        let mut bytes = fs::read(&sums).unwrap();
        bytes[0] ^= 1;
        fs::write(&sums, bytes).unwrap();
        fs::set_permissions(&sums, fs::Permissions::from_mode(0o444)).unwrap();
        assert_eq!(
            generate_compiler_execution_install_manifest_v1(
                fixture.root.path(),
                COMMIT,
                COMPILER_EXECUTION_DEPLOYMENT_TARGET_V1,
            )
            .unwrap_err()
            .kind(),
            DeploymentVerificationErrorKindV1::ContentMismatch
        );
    }

    #[test]
    fn noncanonical_pins_and_root_symlinks_are_rejected() {
        let fixture = Fixture::new();
        let generation = fixture.generate();
        for commit in [
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        ] {
            assert_eq!(
                verify_compiler_execution_deployment_v1(
                    fixture.root.path(),
                    &lower_hex(&generation.sha256()),
                    commit,
                )
                .unwrap_err()
                .kind(),
                DeploymentVerificationErrorKindV1::InvalidDigest
            );
        }
        assert_eq!(
            verify_compiler_execution_deployment_v1(
                fixture.root.path(),
                &lower_hex(&generation.sha256()).to_uppercase(),
                COMMIT,
            )
            .unwrap_err()
            .kind(),
            DeploymentVerificationErrorKindV1::InvalidDigest
        );
        let locator = tempfile::tempdir().unwrap();
        let link = locator.path().join("bundle");
        symlink(fixture.root.path(), &link).unwrap();
        assert!(
            verify_compiler_execution_deployment_v1(
                &link,
                &lower_hex(&generation.sha256()),
                COMMIT,
            )
            .is_err()
        );
    }

    fn current_owner() -> (u32, u32) {
        (
            rustix::process::geteuid().as_raw(),
            rustix::process::getegid().as_raw(),
        )
    }

    fn tree_counts(root: &Path) -> (usize, usize) {
        let mut directories = 1;
        let mut files = 0;
        for entry in fs::read_dir(root).unwrap() {
            let entry = entry.unwrap();
            let metadata = entry.metadata().unwrap();
            if metadata.is_dir() {
                let nested = tree_counts(&entry.path());
                directories += nested.0;
                files += nested.1;
            } else if metadata.is_file() {
                files += 1;
            } else {
                panic!("installed root contains a non-file, non-directory object");
            }
        }
        (directories, files)
    }

    fn private_install_parent() -> tempfile::TempDir {
        let parent = tempfile::tempdir().unwrap();
        fs::set_permissions(parent.path(), fs::Permissions::from_mode(0o700)).unwrap();
        parent
    }

    fn canonical_qualification_base_bytes() -> Vec<u8> {
        fn put_u16(bytes: &mut [u8], offset: usize, value: u16) {
            bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
        }
        fn put_u32(bytes: &mut [u8], offset: usize, value: u32) {
            bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
        }
        fn put_u64(bytes: &mut [u8], offset: usize, value: u64) {
            bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
        }

        let mut bytes = vec![0_u8; 4096];
        put_u32(&mut bytes, 0, 0x7371_7368);
        put_u32(&mut bytes, 4, 1);
        put_u32(&mut bytes, 8, 1_700_000_000);
        put_u32(&mut bytes, 12, 128 * 1024);
        put_u16(&mut bytes, 20, 6);
        put_u16(&mut bytes, 22, 17);
        put_u16(&mut bytes, 24, 0x0100);
        put_u16(&mut bytes, 26, 1);
        put_u16(&mut bytes, 28, 4);
        put_u16(&mut bytes, 30, 0);
        put_u64(&mut bytes, 40, 128);
        put_u64(&mut bytes, 48, 96);
        put_u64(&mut bytes, 56, u64::MAX);
        put_u64(&mut bytes, 64, 104);
        put_u64(&mut bytes, 72, 112);
        for (index, byte) in bytes[96..128].iter_mut().enumerate() {
            *byte = index as u8;
        }
        bytes
    }

    fn qualification_base_fixture(bytes: &[u8]) -> (tempfile::TempDir, PathBuf, String) {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("qualification-base-v1.squashfs");
        fs::write(&path, bytes).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o444)).unwrap();
        let digest: [u8; 32] = Sha256::digest(bytes).into();
        let digest = lower_hex(&digest);
        (root, path, digest)
    }

    fn installed_for_qualification() -> (
        install::InstalledCompilerExecutionDeploymentV1,
        tempfile::TempDir,
    ) {
        let fixture = Fixture::new();
        let generation = fixture.generate();
        let parent = private_install_parent();
        let installed = install::install_compiler_execution_deployment_for_test_v1(
            fixture.verify(generation.sha256()).unwrap(),
            parent.path(),
            current_owner(),
        )
        .unwrap();
        (installed, parent)
    }

    fn prepared_for_staging() -> (
        qualification::PreparedCompilerExecutionQualificationV1,
        tempfile::TempDir,
        tempfile::TempDir,
        tempfile::TempDir,
    ) {
        let base_bytes = canonical_qualification_base_bytes();
        let (base_root, base_path, base_sha256) = qualification_base_fixture(&base_bytes);
        let qualification_parent = private_install_parent();
        let (installed, install_parent) = installed_for_qualification();
        let prepared = qualification::prepare_compiler_execution_qualification_for_test_v1(
            installed,
            &base_path,
            &base_sha256,
            qualification_parent.path(),
            current_owner(),
        )
        .unwrap();
        (prepared, qualification_parent, base_root, install_parent)
    }

    fn assert_installed_root_mutation_rejected(
        expected_kind: DeploymentVerificationErrorKindV1,
        mutate: impl FnOnce(&Path),
    ) {
        let fixture = Fixture::new();
        let generation = fixture.generate();
        let parent = private_install_parent();
        let root_name = install::compiler_execution_install_root_name_v1(generation.sha256());
        let installed = install::install_compiler_execution_deployment_for_test_v1(
            fixture.verify(generation.sha256()).unwrap(),
            parent.path(),
            current_owner(),
        )
        .unwrap();
        let root = parent.path().join(root_name);
        mutate(&root);
        assert_eq!(
            install::revalidate_installed_deployment_for_test_v1(&installed, current_owner(),)
                .unwrap_err()
                .kind(),
            expected_kind
        );

        let second_fixture = Fixture::new();
        let second_generation = second_fixture.generate();
        assert_eq!(second_generation.sha256(), generation.sha256());
        assert_eq!(
            install::install_compiler_execution_deployment_for_test_v1(
                second_fixture.verify(second_generation.sha256()).unwrap(),
                parent.path(),
                current_owner(),
            )
            .unwrap_err()
            .kind(),
            expected_kind
        );
    }

    #[test]
    fn sealed_sources_publish_and_reacquire_one_exact_offline_root() {
        let fixture = Fixture::new();
        let generation = fixture.generate();
        let expected_manifest =
            fs::read(fixture.path(COMPILER_EXECUTION_INSTALL_MANIFEST_NAME_V1)).unwrap();
        let verified = fixture.verify(generation.sha256()).unwrap();
        let install_parent = private_install_parent();
        let root_name = install::compiler_execution_install_root_name_v1(generation.sha256());
        drop(fixture);

        let installed = install::install_compiler_execution_deployment_for_test_v1(
            verified,
            install_parent.path(),
            current_owner(),
        )
        .unwrap();
        assert_eq!(
            installed.publication(),
            install::CompilerExecutionInstalledRootPublicationV1::Created
        );
        install::verify_installed_projection(
            installed.retained_root(),
            &installed,
            current_owner(),
        )
        .unwrap();
        assert_eq!(installed.root_name(), root_name);
        assert_eq!(installed.file_count(), 14);
        let root = install_parent.path().join(&root_name);
        assert_eq!(tree_counts(&root), (12, 14));
        assert_eq!(
            fs::read(root.join("usr/share/fe2o3/compiler-execution/INSTALL-MANIFEST-V1")).unwrap(),
            expected_manifest
        );
        for path in [
            root.clone(),
            root.join("usr"),
            root.join("usr/lib"),
            root.join("usr/lib/systemd"),
            root.join("usr/lib/systemd/system"),
            root.join("usr/lib/sysusers.d"),
            root.join("usr/lib/tmpfiles.d"),
            root.join("usr/libexec"),
            root.join("usr/libexec/fe2o3"),
            root.join("usr/share"),
            root.join("usr/share/fe2o3"),
            root.join("usr/share/fe2o3/compiler-execution"),
        ] {
            assert_eq!(
                fs::metadata(path).unwrap().permissions().mode() & 0o7777,
                0o755
            );
        }

        let second_fixture = Fixture::new();
        let second_generation = second_fixture.generate();
        assert_eq!(second_generation.sha256(), generation.sha256());
        let reacquired = install::install_compiler_execution_deployment_for_test_v1(
            second_fixture.verify(second_generation.sha256()).unwrap(),
            install_parent.path(),
            current_owner(),
        )
        .unwrap();
        assert_eq!(
            reacquired.publication(),
            install::CompilerExecutionInstalledRootPublicationV1::Reacquired
        );
        assert_eq!(tree_counts(&root), (12, 14));
    }

    #[test]
    fn installer_rejects_parent_policy_and_conflicting_final_root() {
        let fixture = Fixture::new();
        let generation = fixture.generate();
        let parent = private_install_parent();
        fs::set_permissions(parent.path(), fs::Permissions::from_mode(0o755)).unwrap();
        assert_eq!(
            install::install_compiler_execution_deployment_for_test_v1(
                fixture.verify(generation.sha256()).unwrap(),
                parent.path(),
                current_owner(),
            )
            .unwrap_err()
            .kind(),
            DeploymentVerificationErrorKindV1::InvalidMetadata
        );

        let fixture = Fixture::new();
        let generation = fixture.generate();
        let parent = private_install_parent();
        let final_root = parent
            .path()
            .join(install::compiler_execution_install_root_name_v1(
                generation.sha256(),
            ));
        fs::create_dir(&final_root).unwrap();
        fs::set_permissions(&final_root, fs::Permissions::from_mode(0o755)).unwrap();
        fs::write(final_root.join("hostile"), b"do not replace").unwrap();
        assert_eq!(
            install::install_compiler_execution_deployment_for_test_v1(
                fixture.verify(generation.sha256()).unwrap(),
                parent.path(),
                current_owner(),
            )
            .unwrap_err()
            .kind(),
            DeploymentVerificationErrorKindV1::InvalidInventory
        );
        assert_eq!(
            fs::read(final_root.join("hostile")).unwrap(),
            b"do not replace"
        );

        let fixture = Fixture::new();
        let generation = fixture.generate();
        let parent = private_install_parent();
        let target = tempfile::tempdir().unwrap();
        let final_root = parent
            .path()
            .join(install::compiler_execution_install_root_name_v1(
                generation.sha256(),
            ));
        symlink(target.path(), &final_root).unwrap();
        assert_eq!(
            install::install_compiler_execution_deployment_for_test_v1(
                fixture.verify(generation.sha256()).unwrap(),
                parent.path(),
                current_owner(),
            )
            .unwrap_err()
            .kind(),
            DeploymentVerificationErrorKindV1::Io
        );
        assert_eq!(fs::read_link(final_root).unwrap(), target.path());
    }

    #[test]
    fn installer_rejects_wrong_parent_owner_xattrs_and_symlink_path() {
        let fixture = Fixture::new();
        let generation = fixture.generate();
        let parent = private_install_parent();
        let owner = current_owner();
        let wrong_owner = (owner.0.wrapping_add(1), owner.1);
        assert_eq!(
            install::install_compiler_execution_deployment_for_test_v1(
                fixture.verify(generation.sha256()).unwrap(),
                parent.path(),
                wrong_owner,
            )
            .unwrap_err()
            .kind(),
            DeploymentVerificationErrorKindV1::InvalidMetadata
        );

        let fixture = Fixture::new();
        let generation = fixture.generate();
        let parent = private_install_parent();
        let parent_file = File::open(parent.path()).unwrap();
        rustix::fs::fsetxattr(&parent_file, "user.fe2o3-test", b"1", XattrFlags::empty()).unwrap();
        assert_eq!(
            install::install_compiler_execution_deployment_for_test_v1(
                fixture.verify(generation.sha256()).unwrap(),
                parent.path(),
                current_owner(),
            )
            .unwrap_err()
            .kind(),
            DeploymentVerificationErrorKindV1::ForbiddenAttributes
        );

        let fixture = Fixture::new();
        let generation = fixture.generate();
        let parent = private_install_parent();
        let link_container = tempfile::tempdir().unwrap();
        let parent_link = link_container.path().join("install-parent-link");
        symlink(parent.path(), &parent_link).unwrap();
        assert_eq!(
            install::install_compiler_execution_deployment_for_test_v1(
                fixture.verify(generation.sha256()).unwrap(),
                &parent_link,
                current_owner(),
            )
            .unwrap_err()
            .kind(),
            DeploymentVerificationErrorKindV1::Io
        );
        assert_eq!(fs::read_dir(parent.path()).unwrap().count(), 0);
    }

    #[test]
    fn installer_rejects_same_length_installed_content_substitution() {
        let fixture = Fixture::new();
        let generation = fixture.generate();
        let parent = private_install_parent();
        let root_name = install::compiler_execution_install_root_name_v1(generation.sha256());
        let installed = install::install_compiler_execution_deployment_for_test_v1(
            fixture.verify(generation.sha256()).unwrap(),
            parent.path(),
            current_owner(),
        )
        .unwrap();
        let build_info = parent
            .path()
            .join(root_name)
            .join("usr/share/fe2o3/compiler-execution/BUILD-INFO");
        fs::set_permissions(&build_info, fs::Permissions::from_mode(0o600)).unwrap();
        let mut bytes = fs::read(&build_info).unwrap();
        bytes[0] ^= 1;
        fs::write(&build_info, bytes).unwrap();
        fs::set_permissions(&build_info, fs::Permissions::from_mode(0o444)).unwrap();
        assert_eq!(
            install::revalidate_installed_deployment_for_test_v1(&installed, current_owner(),)
                .unwrap_err()
                .kind(),
            DeploymentVerificationErrorKindV1::ContentMismatch
        );

        let second_fixture = Fixture::new();
        let second_generation = second_fixture.generate();
        assert_eq!(second_generation.sha256(), generation.sha256());
        assert_eq!(
            install::install_compiler_execution_deployment_for_test_v1(
                second_fixture.verify(second_generation.sha256()).unwrap(),
                parent.path(),
                current_owner(),
            )
            .unwrap_err()
            .kind(),
            DeploymentVerificationErrorKindV1::ContentMismatch
        );
    }

    #[test]
    fn installer_rejects_installed_inventory_type_mode_link_xattr_and_manifest_mutations() {
        assert_installed_root_mutation_rejected(
            DeploymentVerificationErrorKindV1::InvalidInventory,
            |root| fs::write(root.join("unexpected"), b"extra").unwrap(),
        );
        assert_installed_root_mutation_rejected(
            DeploymentVerificationErrorKindV1::InvalidInventory,
            |root| {
                fs::remove_file(root.join("usr/share/fe2o3/compiler-execution/BUILD-INFO"))
                    .unwrap();
            },
        );
        assert_installed_root_mutation_rejected(
            DeploymentVerificationErrorKindV1::InvalidMetadata,
            |root| {
                let path = root.join("usr/share/fe2o3/compiler-execution/BUILD-INFO");
                fs::remove_file(&path).unwrap();
                fs::create_dir(path).unwrap();
            },
        );
        assert_installed_root_mutation_rejected(
            DeploymentVerificationErrorKindV1::InvalidMetadata,
            |root| {
                fs::set_permissions(
                    root.join("usr/share/fe2o3/compiler-execution/BUILD-INFO"),
                    fs::Permissions::from_mode(0o644),
                )
                .unwrap();
            },
        );
        assert_installed_root_mutation_rejected(
            DeploymentVerificationErrorKindV1::InvalidMetadata,
            |root| {
                let systemd = root.join("usr/lib/systemd/system");
                let service = systemd.join("fe2o3-compiler-execution.service");
                let socket = systemd.join("fe2o3-compiler-execution.socket");
                fs::remove_file(&service).unwrap();
                fs::set_permissions(&socket, fs::Permissions::from_mode(0o644)).unwrap();
                fs::hard_link(&socket, service).unwrap();
                fs::set_permissions(socket, fs::Permissions::from_mode(0o444)).unwrap();
            },
        );
        assert_installed_root_mutation_rejected(
            DeploymentVerificationErrorKindV1::ForbiddenAttributes,
            |root| {
                let path = root.join("usr/share/fe2o3/compiler-execution/BUILD-INFO");
                fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
                let file = File::options().read(true).write(true).open(&path).unwrap();
                rustix::fs::fsetxattr(&file, "user.fe2o3-test", b"1", XattrFlags::empty()).unwrap();
                fs::set_permissions(path, fs::Permissions::from_mode(0o444)).unwrap();
            },
        );
        assert_installed_root_mutation_rejected(
            DeploymentVerificationErrorKindV1::ContentMismatch,
            |root| {
                let manifest = root.join("usr/share/fe2o3/compiler-execution/INSTALL-MANIFEST-V1");
                fs::set_permissions(&manifest, fs::Permissions::from_mode(0o600)).unwrap();
                let mut bytes = fs::read(&manifest).unwrap();
                bytes[0] ^= 1;
                fs::write(&manifest, bytes).unwrap();
                fs::set_permissions(&manifest, fs::Permissions::from_mode(0o444)).unwrap();
            },
        );
    }

    #[test]
    fn qualification_preparation_retains_exact_installed_and_base_evidence() {
        let (installed, _install_parent) = installed_for_qualification();
        let expected_manifest_sha256 = installed.manifest_sha256();
        let expected_root_name = installed.root_name().to_owned();
        let base_bytes = canonical_qualification_base_bytes();
        let (base_root, base_path, base_sha256) = qualification_base_fixture(&base_bytes);
        let qualification_parent = private_install_parent();
        let prepared = qualification::prepare_compiler_execution_qualification_for_test_v1(
            installed,
            &base_path,
            &base_sha256,
            qualification_parent.path(),
            current_owner(),
        )
        .unwrap();
        assert_eq!(prepared.manifest_sha256(), expected_manifest_sha256);
        assert_eq!(prepared.installed_root_name(), expected_root_name);
        assert_eq!(lower_hex(&prepared.base_image_sha256()), base_sha256);
        assert_eq!(prepared.base_image_byte_len(), 4096);
        assert_eq!(prepared.base_image_created_epoch(), 1_700_000_000);
        assert_eq!(
            fs::read_dir(qualification_parent.path()).unwrap().count(),
            0
        );

        drop(base_root);
        qualification::revalidate_prepared_qualification_for_test_v1(&prepared, current_owner())
            .unwrap();
    }

    #[test]
    fn qualification_preparation_rejects_wrong_pin_profile_and_source_metadata() {
        let base_bytes = canonical_qualification_base_bytes();
        let (_base_root, base_path, _base_sha256) = qualification_base_fixture(&base_bytes);
        let qualification_parent = private_install_parent();
        let (installed, _install_parent) = installed_for_qualification();
        assert_eq!(
            qualification::prepare_compiler_execution_qualification_for_test_v1(
                installed,
                &base_path,
                &"00".repeat(32),
                qualification_parent.path(),
                current_owner(),
            )
            .unwrap_err()
            .kind(),
            DeploymentVerificationErrorKindV1::ContentMismatch
        );

        let (installed, _install_parent) = installed_for_qualification();
        assert_eq!(
            qualification::prepare_compiler_execution_qualification_for_test_v1(
                installed,
                &base_path,
                "AA",
                qualification_parent.path(),
                current_owner(),
            )
            .unwrap_err()
            .kind(),
            DeploymentVerificationErrorKindV1::InvalidDigest
        );

        let mut malformed = canonical_qualification_base_bytes();
        malformed[0] ^= 1;
        let (_base_root, base_path, base_sha256) = qualification_base_fixture(&malformed);
        let (installed, _install_parent) = installed_for_qualification();
        assert_eq!(
            qualification::prepare_compiler_execution_qualification_for_test_v1(
                installed,
                &base_path,
                &base_sha256,
                qualification_parent.path(),
                current_owner(),
            )
            .unwrap_err()
            .kind(),
            DeploymentVerificationErrorKindV1::InvalidQualificationBase
        );

        let base_bytes = canonical_qualification_base_bytes();
        let (base_root, base_path, base_sha256) = qualification_base_fixture(&base_bytes);
        fs::set_permissions(&base_path, fs::Permissions::from_mode(0o644)).unwrap();
        let (installed, _install_parent) = installed_for_qualification();
        assert_eq!(
            qualification::prepare_compiler_execution_qualification_for_test_v1(
                installed,
                &base_path,
                &base_sha256,
                qualification_parent.path(),
                current_owner(),
            )
            .unwrap_err()
            .kind(),
            DeploymentVerificationErrorKindV1::InvalidMetadata
        );

        fs::set_permissions(&base_path, fs::Permissions::from_mode(0o600)).unwrap();
        let file = File::options()
            .read(true)
            .write(true)
            .open(&base_path)
            .unwrap();
        rustix::fs::fsetxattr(&file, "user.fe2o3-test", b"1", XattrFlags::empty()).unwrap();
        fs::set_permissions(&base_path, fs::Permissions::from_mode(0o444)).unwrap();
        let (installed, _install_parent) = installed_for_qualification();
        assert_eq!(
            qualification::prepare_compiler_execution_qualification_for_test_v1(
                installed,
                &base_path,
                &base_sha256,
                qualification_parent.path(),
                current_owner(),
            )
            .unwrap_err()
            .kind(),
            DeploymentVerificationErrorKindV1::ForbiddenAttributes
        );
        drop(file);
        drop(base_root);

        let base_bytes = canonical_qualification_base_bytes();
        let (base_root, base_path, base_sha256) = qualification_base_fixture(&base_bytes);
        fs::hard_link(&base_path, base_root.path().join("base-hardlink")).unwrap();
        let (installed, _install_parent) = installed_for_qualification();
        assert_eq!(
            qualification::prepare_compiler_execution_qualification_for_test_v1(
                installed,
                &base_path,
                &base_sha256,
                qualification_parent.path(),
                current_owner(),
            )
            .unwrap_err()
            .kind(),
            DeploymentVerificationErrorKindV1::InvalidMetadata
        );

        let base_bytes = canonical_qualification_base_bytes();
        let (base_root, base_path, base_sha256) = qualification_base_fixture(&base_bytes);
        let base_link = base_root.path().join("base-link");
        symlink(&base_path, &base_link).unwrap();
        let (installed, _install_parent) = installed_for_qualification();
        assert_eq!(
            qualification::prepare_compiler_execution_qualification_for_test_v1(
                installed,
                &base_link,
                &base_sha256,
                qualification_parent.path(),
                current_owner(),
            )
            .unwrap_err()
            .kind(),
            DeploymentVerificationErrorKindV1::Io
        );

        let nonempty_parent = private_install_parent();
        fs::write(nonempty_parent.path().join("hostile"), b"do not use").unwrap();
        let (installed, _install_parent) = installed_for_qualification();
        assert_eq!(
            qualification::prepare_compiler_execution_qualification_for_test_v1(
                installed,
                &base_path,
                &base_sha256,
                nonempty_parent.path(),
                current_owner(),
            )
            .unwrap_err()
            .kind(),
            DeploymentVerificationErrorKindV1::InvalidInventory
        );

        let (installed, _install_parent) = installed_for_qualification();
        assert_eq!(
            qualification::prepare_compiler_execution_qualification_for_test_v1(
                installed,
                Path::new("relative-base.squashfs"),
                &base_sha256,
                qualification_parent.path(),
                current_owner(),
            )
            .unwrap_err()
            .kind(),
            DeploymentVerificationErrorKindV1::InvalidMetadata
        );
    }

    #[test]
    fn qualification_preparation_rejects_changed_installed_root_parent_and_privilege() {
        let base_bytes = canonical_qualification_base_bytes();
        let (_base_root, base_path, base_sha256) = qualification_base_fixture(&base_bytes);
        let qualification_parent = private_install_parent();
        let (installed, install_parent) = installed_for_qualification();
        let build_info = install_parent
            .path()
            .join(installed.root_name())
            .join("usr/share/fe2o3/compiler-execution/BUILD-INFO");
        fs::set_permissions(&build_info, fs::Permissions::from_mode(0o600)).unwrap();
        let mut bytes = fs::read(&build_info).unwrap();
        bytes[0] ^= 1;
        fs::write(&build_info, bytes).unwrap();
        fs::set_permissions(&build_info, fs::Permissions::from_mode(0o444)).unwrap();
        assert_eq!(
            qualification::prepare_compiler_execution_qualification_for_test_v1(
                installed,
                &base_path,
                &base_sha256,
                qualification_parent.path(),
                current_owner(),
            )
            .unwrap_err()
            .kind(),
            DeploymentVerificationErrorKindV1::ContentMismatch
        );

        let (installed, _install_parent) = installed_for_qualification();
        let prepared = qualification::prepare_compiler_execution_qualification_for_test_v1(
            installed,
            &base_path,
            &base_sha256,
            qualification_parent.path(),
            current_owner(),
        )
        .unwrap();
        let displaced = qualification_parent
            .path()
            .with_extension("qualification-parent-displaced");
        fs::rename(qualification_parent.path(), &displaced).unwrap();
        fs::create_dir(qualification_parent.path()).unwrap();
        fs::set_permissions(
            qualification_parent.path(),
            fs::Permissions::from_mode(0o700),
        )
        .unwrap();
        assert_eq!(
            qualification::revalidate_prepared_qualification_for_test_v1(
                &prepared,
                current_owner(),
            )
            .unwrap_err()
            .kind(),
            DeploymentVerificationErrorKindV1::InputChanged
        );
        drop(prepared);
        fs::remove_dir(displaced).unwrap();

        if rustix::process::geteuid().as_raw() != 0 {
            let (installed, _install_parent) = installed_for_qualification();
            assert_eq!(
                prepare_compiler_execution_qualification_v1(
                    installed,
                    &base_path,
                    &base_sha256,
                    qualification_parent.path(),
                )
                .unwrap_err()
                .kind(),
                DeploymentVerificationErrorKindV1::InsufficientPrivilege
            );
        }
    }

    #[test]
    fn qualification_staging_retains_one_exact_empty_tree_and_cleans_it() {
        let (prepared, qualification_parent, _base_root, _install_parent) = prepared_for_staging();
        let expected_commit = prepared.git_commit().to_owned();
        let expected_manifest = prepared.manifest_sha256();
        let expected_base = prepared.base_image_sha256();
        let staged =
            staging::stage_compiler_execution_qualification_for_test_v1(prepared, current_owner())
                .unwrap();
        assert_eq!(staged.git_commit(), expected_commit);
        assert_eq!(staged.manifest_sha256(), expected_manifest);
        assert_eq!(staged.base_image_sha256(), expected_base);
        assert_eq!(staged.directory_count(), 8);
        assert!(
            staged
                .run_name()
                .strip_prefix(".compiler-execution-qualification-v1-")
                .is_some_and(|suffix| suffix.len() == 32
                    && suffix
                        .bytes()
                        .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase()))
        );
        let root = qualification_parent.path().join(staged.run_name());
        assert_eq!(tree_counts(&root), (8, 0));
        for path in [
            root.clone(),
            root.join("base"),
            root.join("evidence"),
            root.join("root"),
            root.join("run"),
            root.join("state"),
            root.join("upper"),
            root.join("work"),
        ] {
            assert_eq!(
                fs::metadata(path).unwrap().permissions().mode() & 0o7777,
                0o700
            );
        }
        staging::revalidate_staged_qualification_for_test_v1(&staged, current_owner()).unwrap();
        staged.cleanup().unwrap();
        assert_eq!(
            fs::read_dir(qualification_parent.path()).unwrap().count(),
            0
        );

        let (prepared, qualification_parent, _base_root, _install_parent) = prepared_for_staging();
        let staged =
            staging::stage_compiler_execution_qualification_for_test_v1(prepared, current_owner())
                .unwrap();
        drop(staged);
        assert_eq!(
            fs::read_dir(qualification_parent.path()).unwrap().count(),
            0
        );
    }

    #[test]
    fn every_qualification_staging_boundary_cleans_to_an_empty_parent() {
        let points = staging::qualification_staging_fault_points_for_test_v1();
        assert_eq!(points.len(), 21);
        for point in points {
            let (prepared, qualification_parent, _base_root, _install_parent) =
                prepared_for_staging();
            let error = staging::stage_compiler_execution_qualification_at_fault_for_test_v1(
                prepared,
                current_owner(),
                point,
            )
            .unwrap_err();
            assert_eq!(
                error.kind(),
                DeploymentVerificationErrorKindV1::InjectedFailure,
                "unexpected result at {point:?}: {error}"
            );
            assert_eq!(
                fs::read_dir(qualification_parent.path()).unwrap().count(),
                0,
                "staging residue remained at {point:?}"
            );
        }
    }

    #[test]
    fn qualification_staging_rejects_mutation_and_parent_replacement() {
        let (prepared, qualification_parent, _base_root, _install_parent) = prepared_for_staging();
        let staged =
            staging::stage_compiler_execution_qualification_for_test_v1(prepared, current_owner())
                .unwrap();
        let run = qualification_parent.path().join(staged.run_name());
        fs::set_permissions(run.join("upper"), fs::Permissions::from_mode(0o755)).unwrap();
        assert_eq!(
            staging::revalidate_staged_qualification_for_test_v1(&staged, current_owner())
                .unwrap_err()
                .kind(),
            DeploymentVerificationErrorKindV1::InvalidMetadata
        );
        fs::set_permissions(run.join("upper"), fs::Permissions::from_mode(0o700)).unwrap();
        fs::write(run.join("evidence/unexpected"), b"hostile").unwrap();
        assert_eq!(
            staging::revalidate_staged_qualification_for_test_v1(&staged, current_owner())
                .unwrap_err()
                .kind(),
            DeploymentVerificationErrorKindV1::InvalidInventory
        );
        fs::remove_file(run.join("evidence/unexpected")).unwrap();
        staged.cleanup().unwrap();

        let (prepared, qualification_parent, _base_root, _install_parent) = prepared_for_staging();
        let staged =
            staging::stage_compiler_execution_qualification_for_test_v1(prepared, current_owner())
                .unwrap();
        let displaced = qualification_parent
            .path()
            .with_extension("qualification-staging-displaced");
        fs::rename(qualification_parent.path(), &displaced).unwrap();
        fs::create_dir(qualification_parent.path()).unwrap();
        fs::set_permissions(
            qualification_parent.path(),
            fs::Permissions::from_mode(0o700),
        )
        .unwrap();
        assert_eq!(
            staging::revalidate_staged_qualification_for_test_v1(&staged, current_owner())
                .unwrap_err()
                .kind(),
            DeploymentVerificationErrorKindV1::InputChanged
        );
        staged.cleanup().unwrap();
        assert_eq!(fs::read_dir(&displaced).unwrap().count(), 0);
        fs::remove_dir(qualification_parent.path()).unwrap();
        fs::rename(&displaced, qualification_parent.path()).unwrap();
    }

    #[test]
    fn production_qualification_staging_requires_effective_root() {
        if rustix::process::geteuid().as_raw() == 0 {
            return;
        }
        let (prepared, qualification_parent, _base_root, _install_parent) = prepared_for_staging();
        assert_eq!(
            stage_compiler_execution_qualification_v1(prepared)
                .unwrap_err()
                .kind(),
            DeploymentVerificationErrorKindV1::InsufficientPrivilege
        );
        assert_eq!(
            fs::read_dir(qualification_parent.path()).unwrap().count(),
            0
        );
    }

    #[test]
    fn install_parent_replacement_fails_closed_before_and_after_publication() {
        let fixture = Fixture::new();
        let generation = fixture.generate();
        let parent = private_install_parent();
        let displaced = parent.path().with_extension("displaced-before-publication");
        let error =
            install::install_compiler_execution_deployment_with_parent_replacement_for_test_v1(
                fixture.verify(generation.sha256()).unwrap(),
                parent.path(),
                &displaced,
                current_owner(),
                install::InstallParentReplacementPointV1::DuringCopy,
            )
            .unwrap_err();
        assert_eq!(
            error.kind(),
            DeploymentVerificationErrorKindV1::InputChanged
        );
        assert_eq!(fs::read_dir(parent.path()).unwrap().count(), 0);
        assert_eq!(fs::read_dir(&displaced).unwrap().count(), 0);
        fs::remove_dir(&displaced).unwrap();

        let fixture = Fixture::new();
        let generation = fixture.generate();
        let root_name = install::compiler_execution_install_root_name_v1(generation.sha256());
        let displaced = parent.path().with_extension("displaced-after-publication");
        let error =
            install::install_compiler_execution_deployment_with_parent_replacement_for_test_v1(
                fixture.verify(generation.sha256()).unwrap(),
                parent.path(),
                &displaced,
                current_owner(),
                install::InstallParentReplacementPointV1::AfterPublication,
            )
            .unwrap_err();
        assert_eq!(
            error.kind(),
            DeploymentVerificationErrorKindV1::PublicationAmbiguous
        );
        assert_eq!(fs::read_dir(parent.path()).unwrap().count(), 0);
        assert_eq!(tree_counts(&displaced.join(&root_name)), (12, 14));

        fs::remove_dir(parent.path()).unwrap();
        fs::rename(&displaced, parent.path()).unwrap();
        let recovery_fixture = Fixture::new();
        let recovery_generation = recovery_fixture.generate();
        let recovered = install::install_compiler_execution_deployment_for_test_v1(
            recovery_fixture
                .verify(recovery_generation.sha256())
                .unwrap(),
            parent.path(),
            current_owner(),
        )
        .unwrap();
        assert_eq!(
            recovered.publication(),
            install::CompilerExecutionInstalledRootPublicationV1::Reacquired
        );
    }

    #[test]
    fn production_installer_requires_effective_root() {
        if rustix::process::geteuid().as_raw() == 0 {
            return;
        }
        let fixture = Fixture::new();
        let generation = fixture.generate();
        let parent = private_install_parent();
        assert_eq!(
            install::install_compiler_execution_deployment_v1(
                fixture.verify(generation.sha256()).unwrap(),
                parent.path(),
            )
            .unwrap_err()
            .kind(),
            DeploymentVerificationErrorKindV1::InsufficientPrivilege
        );
        assert_eq!(fs::read_dir(parent.path()).unwrap().count(), 0);
    }

    #[test]
    fn every_installation_boundary_is_absent_or_completely_reacquirable() {
        let points = install::installation_fault_points_for_test_v1();
        assert_eq!(points.len(), 99);
        for point in points {
            let fixture = Fixture::new();
            let generation = fixture.generate();
            let parent = private_install_parent();
            let root_name = install::compiler_execution_install_root_name_v1(generation.sha256());
            let error = install::install_compiler_execution_deployment_at_fault_for_test_v1(
                fixture.verify(generation.sha256()).unwrap(),
                parent.path(),
                current_owner(),
                point,
            )
            .unwrap_err();
            let after_publication =
                install::installation_fault_is_after_publication_for_test_v1(point);
            assert_eq!(
                error.kind(),
                if after_publication {
                    DeploymentVerificationErrorKindV1::PublicationAmbiguous
                } else {
                    DeploymentVerificationErrorKindV1::InjectedFailure
                },
                "unexpected failure category at {point:?}: {error}"
            );
            let entries: Vec<_> = fs::read_dir(parent.path())
                .unwrap()
                .map(|entry| entry.unwrap().file_name())
                .collect();
            if after_publication {
                assert_eq!(entries, [OsString::from(&root_name)]);
                let recovery_fixture = Fixture::new();
                let recovery_generation = recovery_fixture.generate();
                assert_eq!(recovery_generation.sha256(), generation.sha256());
                let recovered = install::install_compiler_execution_deployment_for_test_v1(
                    recovery_fixture
                        .verify(recovery_generation.sha256())
                        .unwrap(),
                    parent.path(),
                    current_owner(),
                )
                .unwrap();
                assert_eq!(
                    recovered.publication(),
                    install::CompilerExecutionInstalledRootPublicationV1::Reacquired
                );
                assert_eq!(
                    tree_counts(&parent.path().join(&root_name)),
                    (12, 14),
                    "published fault point left a partial root: {point:?}"
                );
            } else {
                assert!(
                    entries.is_empty(),
                    "pre-publication fault left staging or final state at {point:?}: {entries:?}"
                );
            }
        }
    }
}
