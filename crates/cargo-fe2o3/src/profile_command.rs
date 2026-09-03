//! Bounded, explicitly authorized rocprofv3 orchestration.
//!
//! Planning opens and measures every executable input, the fixed collector configuration,
//! inherited environment, and direct-KFD topology records. It does not execute rocprofv3 or the
//! target. Collection requires the exact plan digest printed by that dry run. The presence of a
//! rocprofv3 option, a successful collector exit, or a file with a familiar suffix is not treated
//! as proof that a direct-KFD dispatch or ATT record was observed; Bundle V4 import is the separate
//! validation boundary.

use crate::profile_dispatch_import_v1::{
    DispatchImportBindingV1, DispatchImportProductV1, DispatchImportSourceKindV1,
    DispatchImportTargetBindingV1, ObservedTargetFamilyV1, PROFILE_DISPATCH_BUNDLE_FILE_V1,
    PROFILE_DISPATCH_RECEIPT_FILE_V1, import_dispatch_v1, readmit_dispatch_import_tuple_v1,
};
use fe2o3_artifact_transaction::{NoRetainedDurableDirectoryHooksV1, RetainedDurableDirectoryV1};
use fe2o3_kernel_ir::{
    AMDGPU_EXACT_TARGET_CAPABILITY_NAMESPACE, AMDGPU_GFX942_XNACK_MINUS_TARGET_CAPABILITY_NAME,
    AMDGPU_GFX950_XNACK_MINUS_TARGET_CAPABILITY_NAME, MAX_MODULE_BYTES_V1, Module,
    TargetCapability, VerifiedCanonicalKernelIrV7, WaveWidth,
};
use fe2o3_semantic_import::{
    CaptureIdentityV1, ContentIdentityRecordV1, ContentSchemeV1, ProfilerBundleErrorV4,
    RocprofJsonGpuAgentBindingV4, project_rocprofv3_json_dispatch_agents_v4,
    rocprofv3_csv_source_agent_bindings_v4,
};
use fe2o3_semantic_trace::WaveWidthV1;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::ffi::{OsStr, OsString};
use std::fmt::Write as _;
use std::fs::{self, File, Metadata, OpenOptions};
use std::io::{self, Read, Write};
use std::mem::MaybeUninit;
use std::os::fd::AsRawFd;
use std::os::unix::fs::{MetadataExt as _, OpenOptionsExt as _, PermissionsExt as _};
use std::os::unix::process::CommandExt as _;
use std::path::{Component, Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

const DEFAULT_TIMEOUT_MS: u64 = 120_000;
const MAX_TIMEOUT_MS: u64 = 900_000;
const DEFAULT_OUTPUT_LIMIT: usize = 1024 * 1024;
const MAX_OUTPUT_LIMIT: usize = 16 * 1024 * 1024;
const DEFAULT_STORAGE_LIMIT: u64 = 256 * 1024 * 1024;
const MAX_STORAGE_LIMIT: u64 = 4 * 1024 * 1024 * 1024;
const MAX_TOOL_BYTES: u64 = 4 * 1024 * 1024;
const MAX_INTERPRETER_BYTES: u64 = 64 * 1024 * 1024;
const MAX_COLLECTOR_LIBRARY_BYTES: u64 = 128 * 1024 * 1024;
const MAX_TARGET_BYTES: u64 = 1024 * 1024 * 1024;
const MAX_INSPECTION_PREFIX: usize = 128 * 1024;
const MAX_TOPOLOGY_BYTES: u64 = 64 * 1024;
const MAX_KFD_SCALAR_BACKING_BYTES: u64 = 4 * 1024;
const MAX_KFD_SCALAR_CONTENT_BYTES: u64 = 32;
const MAX_DEVICES: usize = 64;
const MAX_ARTIFACTS: usize = 4096;
const MAX_ARTIFACT_DEPTH: usize = 8;
const MAX_PROFILER_IMPORT_SOURCE_BYTES: u64 = 8 * 1024 * 1024;
const MAX_ARGUMENTS: usize = 256;
const MAX_ARGUMENT_BYTES: usize = 4096;
const MAX_OBSERVED_GPU_TARGET_PROFILE_RECORD_BYTES_V1: usize = 512;
const POLL_INTERVAL: Duration = Duration::from_millis(5);
const CAPTURE_DRAIN_TIMEOUT: Duration = Duration::from_millis(100);
const OWNERSHIP_FILE: &str = ".fe2o3-profile-owned-v1";
const MANIFEST_FILE: &str = "fe2o3-profile-manifest-v1.txt";
const MANIFEST_REDO_FILE: &str = ".fe2o3-profile-manifest-v1.redo";
const PROFILE_DISPATCH_BUNDLE_REDO_FILE_V1: &str = ".fe2o3-semantic-profiler-bundle-v4.redo";
const PROFILE_DISPATCH_RECEIPT_REDO_FILE_V1: &str =
    ".fe2o3-profile-dispatch-import-receipt-v1.redo";
const KFD_TOPOLOGY_ROOT: &str = "/sys/class/kfd/kfd/topology/nodes";
const EXPECTED_AMD_VENDOR_ID: u64 = 0x1002;
const GFX942_TARGET_VERSION: u64 = 90_402;
const GFX950_TARGET_VERSION: u64 = 90_500;
const PRODUCTION_WAVE_WIDTH: u64 = 64;
const SEALED_COLLECTOR_ADAPTER_SCHEMA_V1: &[u8] = b"fe2o3-rocprofv3-sealed-adapter-v1";
const SEALED_TOOL_ENV_V1: &str = "FE2O3_ROCPROF_TOOL_IMAGE_V1";
const SEALED_CORE_ENV_V1: &str = "FE2O3_ROCPROF_CORE_IMAGE_V1";
const LOGICAL_ROCM_ROOT_ENV_V1: &str = "FE2O3_ROCPROF_LOGICAL_ROOT_V1";
const SEALED_SCRIPT_ENV_V1: &str = "FE2O3_ROCPROF_SCRIPT_IMAGE_V1";
const LOGICAL_SCRIPT_ENV_V1: &str = "FE2O3_ROCPROF_SCRIPT_LOGICAL_V1";
const SEALED_COLLECTOR_BOOTSTRAP_V1: &str = r#"#!/usr/bin/env python3
import os
import sys
source_path = os.environ.pop("FE2O3_ROCPROF_SCRIPT_IMAGE_V1")
logical_path = os.environ.pop("FE2O3_ROCPROF_SCRIPT_LOGICAL_V1")
with open(source_path, "rb") as stream:
    source = stream.read(4194305)
if not source or len(source) > 4194304:
    raise RuntimeError("sealed rocprofv3 script is outside its byte bound")
sys.argv[0] = logical_path
scope = {"__name__": "__main__", "__file__": logical_path, "__package__": None, "__cached__": None}
exec(compile(source, logical_path, "exec"), scope, scope)
"#;
const INSTALLED_ROCPROFV3_724_SCRIPT_SHA256_V1: [u8; 32] = [
    0x19, 0x5f, 0xf5, 0xe6, 0xfa, 0xf4, 0x8a, 0x3a, 0xbb, 0xc6, 0xf4, 0xdb, 0x9f, 0x69, 0xdd, 0x59,
    0x8f, 0xe7, 0x1f, 0xa9, 0xff, 0x69, 0x5b, 0xa2, 0x55, 0x6d, 0x65, 0xaf, 0x63, 0x6f, 0xdc, 0x48,
];
const INSTALLED_ROCPROFV3_724_SCRIPT_LENGTH_V1: u64 = 62_506;

const ALLOWED_ENVIRONMENT: &[&str] = &[
    "GPU_DEVICE_ORDINAL",
    "HOME",
    "LANG",
    "LC_ALL",
    "ROCR_VISIBLE_DEVICES",
    "TERM",
    "TMPDIR",
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Action {
    Plan,
    Collect,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ProfileKind {
    DispatchJson,
    DispatchCsv,
    Att,
}

impl ProfileKind {
    const fn name(self) -> &'static str {
        match self {
            Self::DispatchJson => "dispatch-json",
            Self::DispatchCsv => "dispatch-csv",
            Self::Att => "att",
        }
    }

    const fn collector_flag(self) -> &'static str {
        match self {
            Self::DispatchJson | Self::DispatchCsv => "--kernel-trace",
            Self::Att => "--advanced-thread-trace",
        }
    }

    const fn output_format(self) -> &'static str {
        match self {
            Self::DispatchCsv => "csv",
            Self::DispatchJson | Self::Att => "json",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct KirBinding {
    digest: [u8; 32],
    length: u64,
    wave_width: u8,
}

#[derive(Debug)]
struct Options {
    action: Action,
    authorization: Option<[u8; 32]>,
    tool: Option<PathBuf>,
    interpreter: Option<PathBuf>,
    output_directory: PathBuf,
    working_directory: Option<PathBuf>,
    kind: ProfileKind,
    timeout: Duration,
    stdout_limit: usize,
    stderr_limit: usize,
    storage_limit: u64,
    program: String,
    program_arguments: Vec<String>,
    kir_binding: Option<KirBinding>,
    kir_v7_path: Option<PathBuf>,
}

#[derive(Debug)]
pub(crate) struct CommandReport {
    output: String,
    succeeded: bool,
}

impl CommandReport {
    pub(crate) fn output(&self) -> &str {
        &self.output
    }

    pub(crate) const fn succeeded(&self) -> bool {
        self.succeeded
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ObjectIdentity {
    device: u64,
    inode: u64,
    mode: u32,
    size: u64,
    modified_seconds: i64,
    modified_nanoseconds: i64,
    changed_seconds: i64,
    changed_nanoseconds: i64,
}

impl ObjectIdentity {
    fn from_metadata(metadata: &Metadata) -> Self {
        Self {
            device: metadata.dev(),
            inode: metadata.ino(),
            mode: metadata.mode(),
            size: metadata.len(),
            modified_seconds: metadata.mtime(),
            modified_nanoseconds: metadata.mtime_nsec(),
            changed_seconds: metadata.ctime(),
            changed_nanoseconds: metadata.ctime_nsec(),
        }
    }
}

struct FilePin {
    file: File,
    image: SealedImage,
    canonical_path: PathBuf,
    identity: ObjectIdentity,
    digest: [u8; 32],
    prefix: Vec<u8>,
}

struct SealedImage {
    file: File,
    identity: ObjectIdentity,
    seals: rustix::fs::SealFlags,
    digest: [u8; 32],
}

impl SealedImage {
    fn writable(label: &str) -> Result<File, String> {
        let descriptor = rustix::fs::memfd_create(
            "fe2o3-profile-execution-v1",
            rustix::fs::MemfdFlags::CLOEXEC | rustix::fs::MemfdFlags::ALLOW_SEALING,
        )
        .map_err(|error| format!("failed to create sealed {label} image: {error}"))?;
        Ok(File::from(descriptor))
    }

    fn from_bytes(bytes: &[u8], executable: bool, label: &str) -> Result<Self, String> {
        let mut writable = Self::writable(label)?;
        writable
            .write_all(bytes)
            .map_err(|error| format!("failed to write sealed {label} image: {error}"))?;
        Self::finish(
            writable,
            executable,
            Sha256::digest(bytes).into(),
            bytes.len() as u64,
            label,
        )
    }

    fn finish(
        mut writable: File,
        executable: bool,
        digest: [u8; 32],
        length: u64,
        label: &str,
    ) -> Result<Self, String> {
        use std::io::{Seek as _, SeekFrom};
        writable
            .seek(SeekFrom::Start(0))
            .map_err(|error| format!("failed to rewind sealed {label} image: {error}"))?;
        let mut observed_digest = Sha256::new();
        let mut observed_length = 0_u64;
        let mut buffer = [0_u8; 16 * 1024];
        loop {
            let read = writable
                .read(&mut buffer)
                .map_err(|error| format!("failed to verify sealed {label} image: {error}"))?;
            if read == 0 {
                break;
            }
            observed_length = observed_length
                .checked_add(read as u64)
                .ok_or_else(|| format!("sealed {label} image length overflow"))?;
            observed_digest.update(&buffer[..read]);
        }
        if observed_length != length || <[u8; 32]>::from(observed_digest.finalize()) != digest {
            return Err(format!(
                "sealed {label} image content does not match its source"
            ));
        }
        rustix::fs::fchmod(
            &writable,
            rustix::fs::Mode::from_raw_mode(if executable { 0o500 } else { 0o400 }),
        )
        .map_err(|error| format!("failed to set sealed {label} image mode: {error}"))?;
        let data_seals = rustix::fs::SealFlags::WRITE
            | rustix::fs::SealFlags::GROW
            | rustix::fs::SealFlags::SHRINK;
        rustix::fs::fcntl_add_seals(&writable, data_seals)
            .and_then(|()| rustix::fs::fcntl_add_seals(&writable, rustix::fs::SealFlags::SEAL))
            .map_err(|error| format!("failed to seal {label} image: {error}"))?;
        let seals = rustix::fs::fcntl_get_seals(&writable)
            .map_err(|error| format!("failed to inspect sealed {label} image: {error}"))?;
        let required = data_seals | rustix::fs::SealFlags::SEAL;
        if seals != required && seals != required | rustix::fs::SealFlags::FUTURE_WRITE {
            return Err(format!("sealed {label} image has an unexpected seal set"));
        }
        let writable_metadata = writable
            .metadata()
            .map_err(|error| format!("failed to inspect sealed {label} image: {error}"))?;
        if !writable_metadata.is_file() || writable_metadata.len() != length {
            return Err(format!("sealed {label} image has an invalid object shape"));
        }
        let read_path = format!("/proc/self/fd/{}", writable.as_raw_fd());
        let read_only = rustix::fs::open(
            read_path,
            rustix::fs::OFlags::RDONLY | rustix::fs::OFlags::CLOEXEC,
            rustix::fs::Mode::empty(),
        )
        .map(File::from)
        .map_err(|error| format!("failed to reopen sealed {label} image read-only: {error}"))?;
        let metadata = read_only
            .metadata()
            .map_err(|error| format!("failed to inspect read-only {label} image: {error}"))?;
        if metadata.dev() != writable_metadata.dev()
            || metadata.ino() != writable_metadata.ino()
            || rustix::fs::fcntl_getfl(&read_only)
                .map_err(|error| format!("failed to inspect read-only {label} flags: {error}"))?
                & rustix::fs::OFlags::ACCMODE
                != rustix::fs::OFlags::RDONLY
        {
            return Err(format!(
                "sealed {label} image read-only reopen changed identity"
            ));
        }
        drop(writable);
        Ok(Self {
            file: read_only,
            identity: ObjectIdentity::from_metadata(&metadata),
            seals,
            digest,
        })
    }

    fn validate(&self, label: &str) -> Result<(), String> {
        let metadata = self
            .file
            .metadata()
            .map_err(|error| format!("failed to re-inspect sealed {label} image: {error}"))?;
        let seals = rustix::fs::fcntl_get_seals(&self.file)
            .map_err(|error| format!("failed to re-inspect sealed {label} seals: {error}"))?;
        if ObjectIdentity::from_metadata(&metadata) != self.identity
            || seals != self.seals
            || rustix::fs::fcntl_getfl(&self.file)
                .map_err(|error| format!("failed to re-inspect sealed {label} flags: {error}"))?
                & rustix::fs::OFlags::ACCMODE
                != rustix::fs::OFlags::RDONLY
        {
            return Err(format!("sealed {label} image changed after planning"));
        }
        Ok(())
    }

    fn execution_path(&self) -> PathBuf {
        PathBuf::from(format!("/proc/self/fd/{}", self.file.as_raw_fd()))
    }

    fn external_path(&self) -> PathBuf {
        PathBuf::from(format!(
            "/proc/{}/fd/{}",
            std::process::id(),
            self.file.as_raw_fd()
        ))
    }
}

impl FilePin {
    fn open(path: &Path, label: &str, maximum: u64, executable: bool) -> Result<Self, String> {
        let link = fs::symlink_metadata(path)
            .map_err(|error| format!("failed to inspect {label} {}: {error}", path.display()))?;
        if link.file_type().is_symlink() {
            return Err(format!(
                "{label} path must not be a symbolic link: {}",
                path.display()
            ));
        }
        let canonical_path = fs::canonicalize(path).map_err(|error| {
            format!("failed to canonicalize {label} {}: {error}", path.display())
        })?;
        if canonical_path.as_os_str().as_encoded_bytes().len() > MAX_ARGUMENT_BYTES {
            return Err(format!("canonical {label} path exceeds the path bound"));
        }
        let fd = rustix::fs::open(
            &canonical_path,
            rustix::fs::OFlags::RDONLY
                | rustix::fs::OFlags::NOFOLLOW
                | rustix::fs::OFlags::NONBLOCK
                | rustix::fs::OFlags::CLOEXEC,
            rustix::fs::Mode::empty(),
        )
        .map_err(|error| {
            format!(
                "failed to retain {label} {}: {error}",
                canonical_path.display()
            )
        })?;
        let mut file = File::from(fd);
        let before = file
            .metadata()
            .map_err(|error| format!("failed to inspect retained {label}: {error}"))?;
        if !before.is_file() || before.len() == 0 || before.len() > maximum {
            return Err(format!(
                "{label} must be a nonempty regular file of at most {maximum} bytes"
            ));
        }
        if executable && before.permissions().mode() & 0o111 == 0 {
            return Err(format!(
                "{label} is not executable: {}",
                canonical_path.display()
            ));
        }
        let mut hasher = Sha256::new();
        let mut prefix = Vec::new();
        let mut image = SealedImage::writable(label)?;
        let mut read_total = 0_u64;
        let mut buffer = [0_u8; 16 * 1024];
        loop {
            let read = file
                .read(&mut buffer)
                .map_err(|error| format!("failed to read retained {label}: {error}"))?;
            if read == 0 {
                break;
            }
            read_total = read_total
                .checked_add(read as u64)
                .ok_or_else(|| format!("{label} length overflow"))?;
            if read_total > maximum {
                return Err(format!("{label} exceeds the {maximum}-byte bound"));
            }
            hasher.update(&buffer[..read]);
            image
                .write_all(&buffer[..read])
                .map_err(|error| format!("failed to copy sealed {label} image: {error}"))?;
            let remaining = MAX_INSPECTION_PREFIX.saturating_sub(prefix.len());
            prefix.extend_from_slice(&buffer[..read.min(remaining)]);
        }
        if read_total != before.len() {
            return Err(format!("{label} changed while it was measured"));
        }
        let after = file
            .metadata()
            .map_err(|error| format!("failed to re-inspect retained {label}: {error}"))?;
        let identity = ObjectIdentity::from_metadata(&before);
        if ObjectIdentity::from_metadata(&after) != identity {
            return Err(format!("{label} changed while it was measured"));
        }
        let digest = hasher.finalize().into();
        let image = SealedImage::finish(image, executable, digest, read_total, label)?;
        Ok(Self {
            file,
            image,
            canonical_path,
            identity,
            digest,
            prefix,
        })
    }

    fn validate(&self, label: &str) -> Result<(), String> {
        self.image.validate(label)?;
        if self.image.digest != self.digest || self.image.identity.size != self.identity.size {
            return Err(format!(
                "sealed {label} image identity changed after planning"
            ));
        }
        let descriptor = self
            .file
            .metadata()
            .map_err(|error| format!("failed to re-inspect retained {label}: {error}"))?;
        if ObjectIdentity::from_metadata(&descriptor) != self.identity {
            return Err(format!("retained {label} changed after planning"));
        }
        let current = fs::metadata(&self.canonical_path)
            .map_err(|error| format!("{label} path disappeared after planning: {error}"))?;
        if ObjectIdentity::from_metadata(&current) != self.identity {
            return Err(format!("{label} path changed after planning"));
        }
        Ok(())
    }

    fn read_retained(&self, label: &str, maximum: u64) -> Result<Vec<u8>, String> {
        self.validate(label)?;
        if self.identity.size == 0 || self.identity.size > maximum {
            return Err(format!("retained {label} exceeds the {maximum}-byte bound"));
        }
        let capacity = usize::try_from(self.identity.size)
            .map_err(|_| format!("retained {label} size does not fit memory"))?;
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(capacity)
            .map_err(|_| format!("failed to reserve retained {label} bytes"))?;
        let mut clone = self
            .file
            .try_clone()
            .map_err(|error| format!("failed to duplicate retained {label}: {error}"))?;
        use std::io::{Seek as _, SeekFrom};
        clone
            .seek(SeekFrom::Start(0))
            .map_err(|error| format!("failed to rewind retained {label}: {error}"))?;
        clone
            .take(maximum.saturating_add(1))
            .read_to_end(&mut bytes)
            .map_err(|error| format!("failed to read retained {label}: {error}"))?;
        if bytes.len() != capacity || <[u8; 32]>::from(Sha256::digest(&bytes)) != self.digest {
            return Err(format!("retained {label} changed while it was reread"));
        }
        self.validate(label)?;
        Ok(bytes)
    }

    fn execution_path(&self) -> PathBuf {
        self.image.execution_path()
    }

    fn external_path(&self) -> PathBuf {
        self.image.external_path()
    }

    fn content_identity(&self) -> String {
        content_identity(&self.digest, self.identity.size)
    }
}

#[derive(Clone, Debug)]
struct EnvironmentEntry {
    name: &'static str,
    value: OsString,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct DeviceIdentity {
    node: u32,
    hardware: KfdGpuHardwareV1,
    bytes: Vec<u8>,
    digest: [u8; 32],
    target_profile: ObservedGpuTargetProfileRecordV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct KfdGpuHardwareV1 {
    gpu_id: u64,
    simd_count: u64,
    vendor_id: u64,
    device_id: u64,
    location_id: u64,
    domain: u64,
    gfx_target_version: u64,
    wave_front_size: u64,
    num_xcc: u64,
}

struct VerifiedKirInputV1 {
    pin: FilePin,
    owner: VerifiedCanonicalKernelIrV7,
    compatibility: KirTargetCompatibilityV1,
}

impl VerifiedKirInputV1 {
    fn revalidate(&self) -> Result<(), String> {
        self.pin.validate("canonical Kernel IR V7")?;
        let bytes = self
            .pin
            .read_retained("canonical Kernel IR V7", MAX_MODULE_BYTES_V1 as u64)?;
        if bytes != self.owner.canonical_bytes() {
            return Err("retained canonical Kernel IR V7 bytes changed".to_owned());
        }
        self.owner
            .revalidate()
            .map_err(|error| format!("canonical Kernel IR V7 revalidation failed: {error}"))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum KirTargetCompatibilityV1 {
    Ready(ObservedGpuTargetProfileV1),
    Unavailable(KirTargetUnavailableReasonV1),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum KirTargetUnavailableReasonV1 {
    MissingExactTarget,
    ConflictingExactTargets,
    UnknownExactTarget,
    MissingWave64,
    ConflictingWaveWidth,
    KfdProfileUnavailable,
    KfdFamilyMismatch,
}

impl KirTargetUnavailableReasonV1 {
    const fn name(self) -> &'static str {
        match self {
            Self::MissingExactTarget => "missing-exact-target-capability",
            Self::ConflictingExactTargets => "conflicting-exact-target-capabilities",
            Self::UnknownExactTarget => "unknown-exact-target-capability",
            Self::MissingWave64 => "missing-wave64-capability",
            Self::ConflictingWaveWidth => "conflicting-wave-width-capabilities",
            Self::KfdProfileUnavailable => "direct-kfd-target-profile-unavailable",
            Self::KfdFamilyMismatch => "kir-target-family-does-not-match-direct-kfd",
        }
    }

    const fn tag(self) -> u8 {
        match self {
            Self::MissingExactTarget => 1,
            Self::ConflictingExactTargets => 2,
            Self::UnknownExactTarget => 3,
            Self::MissingWave64 => 4,
            Self::ConflictingWaveWidth => 5,
            Self::KfdProfileUnavailable => 6,
            Self::KfdFamilyMismatch => 7,
        }
    }
}

impl DeviceIdentity {
    fn content_identity(&self) -> String {
        content_identity(&self.digest, self.bytes.len() as u64)
    }

    fn target_profile_record(&self) -> String {
        let (availability, profile, reason) = self.target_profile.status.fields();
        let record = format!(
            "schema=fe2o3-observed-gpu-target-profile-v1;origin=direct-kfd-properties;node={};stable-device-identity={};vendor-id={};gfx-target-version={};wave-width={};availability={availability};profile={profile};unavailable-reason={reason}",
            self.node,
            self.content_identity(),
            self.target_profile.vendor_id,
            self.target_profile.gfx_target_version,
            self.target_profile.wave_width,
        );
        debug_assert!(record.len() <= MAX_OBSERVED_GPU_TARGET_PROFILE_RECORD_BYTES_V1);
        record
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ObservedGpuTargetProfileRecordV1 {
    vendor_id: u64,
    gfx_target_version: u64,
    wave_width: u64,
    status: ObservedGpuTargetProfileStatusV1,
}

impl ObservedGpuTargetProfileRecordV1 {
    const fn from_direct_kfd_properties(
        vendor_id: u64,
        gfx_target_version: u64,
        wave_width: u64,
    ) -> Self {
        let candidate = match gfx_target_version {
            GFX942_TARGET_VERSION => Some(ObservedGpuTargetProfileV1::Gfx942),
            GFX950_TARGET_VERSION => Some(ObservedGpuTargetProfileV1::Gfx950),
            _ => None,
        };
        let status = match candidate {
            None => ObservedGpuTargetProfileStatusV1::Unavailable(
                ObservedGpuTargetProfileUnavailableReasonV1::UnknownGfxTargetVersion,
            ),
            Some(_)
                if vendor_id != EXPECTED_AMD_VENDOR_ID && wave_width != PRODUCTION_WAVE_WIDTH =>
            {
                ObservedGpuTargetProfileStatusV1::Unavailable(
                    ObservedGpuTargetProfileUnavailableReasonV1::VendorAndWaveWidthContradiction,
                )
            }
            Some(_) if vendor_id != EXPECTED_AMD_VENDOR_ID => {
                ObservedGpuTargetProfileStatusV1::Unavailable(
                    ObservedGpuTargetProfileUnavailableReasonV1::VendorContradiction,
                )
            }
            Some(_) if wave_width != PRODUCTION_WAVE_WIDTH => {
                ObservedGpuTargetProfileStatusV1::Unavailable(
                    ObservedGpuTargetProfileUnavailableReasonV1::WaveWidthContradiction,
                )
            }
            Some(profile) => ObservedGpuTargetProfileStatusV1::Observed(profile),
        };
        Self {
            vendor_id,
            gfx_target_version,
            wave_width,
            status,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ObservedGpuTargetProfileV1 {
    Gfx942,
    Gfx950,
}

impl ObservedGpuTargetProfileV1 {
    const fn name(self) -> &'static str {
        match self {
            Self::Gfx942 => "gfx942",
            Self::Gfx950 => "gfx950",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ObservedGpuTargetProfileStatusV1 {
    Observed(ObservedGpuTargetProfileV1),
    Unavailable(ObservedGpuTargetProfileUnavailableReasonV1),
}

impl ObservedGpuTargetProfileStatusV1 {
    const fn fields(self) -> (&'static str, &'static str, &'static str) {
        match self {
            Self::Observed(profile) => ("observed", profile.name(), "none"),
            Self::Unavailable(reason) => ("unavailable", "unavailable", reason.name()),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ObservedGpuTargetProfileUnavailableReasonV1 {
    UnknownGfxTargetVersion,
    VendorContradiction,
    WaveWidthContradiction,
    VendorAndWaveWidthContradiction,
}

impl ObservedGpuTargetProfileUnavailableReasonV1 {
    const fn name(self) -> &'static str {
        match self {
            Self::UnknownGfxTargetVersion => "unknown-gfx-target-version",
            Self::VendorContradiction => "vendor-contradicts-amd-target",
            Self::WaveWidthContradiction => "wave-width-contradicts-target",
            Self::VendorAndWaveWidthContradiction => "vendor-and-wave-width-contradict-target",
        }
    }
}

struct Plan {
    options: Options,
    working_directory: PathBuf,
    output_directory: PathBuf,
    tool: FilePin,
    interpreter: FilePin,
    collector_libraries: Option<CollectorLibraries>,
    collector_execution: CollectorExecutionV1,
    collector_tool_bytes: Vec<u8>,
    collector_tool_digest: [u8; 32],
    target: FilePin,
    environment: Vec<EnvironmentEntry>,
    environment_bytes: Vec<u8>,
    environment_digest: [u8; 32],
    devices: Vec<DeviceIdentity>,
    configuration: Vec<u8>,
    configuration_digest: [u8; 32],
    collector_arguments: Vec<String>,
    authorization: [u8; 32],
    verified_kir_v7: Option<VerifiedKirInputV1>,
}

struct CollectorLibraries {
    tool_route: PathBuf,
    tool: FilePin,
    core_route: PathBuf,
    core: FilePin,
}

enum CollectorExecutionV1 {
    ExactScript {
        bootstrap: SealedImage,
    },
    InstalledAdapter {
        bootstrap: SealedImage,
        image: SealedImage,
        digest: [u8; 32],
        length: u64,
    },
}

const ROCPROF_TOOL_ASSIGNMENT_V1: &str =
    "    ROCPROF_TOOL_LIBRARY = f\"{ROCM_DIR}/lib/rocprofiler-sdk/librocprofiler-sdk-tool.so\"";
const ROCPROF_CORE_ASSIGNMENT_V1: &str =
    "    ROCPROF_SDK_LIBRARY = f\"{ROCM_DIR}/lib/librocprofiler-sdk.so\"";
const ROCPROF_TOOL_RESOLUTION_V1: &str =
    "    ROCPROF_TOOL_LIBRARY = resolve_library_path(ROCPROF_TOOL_LIBRARY, args)";
const ROCPROF_CORE_RESOLUTION_V1: &str =
    "    ROCPROF_SDK_LIBRARY = resolve_library_path(ROCPROF_SDK_LIBRARY, args)";
const ROCPROF_PRELOAD_ORDER_V1: &str =
    "    append_preload = [\n        ROCPROF_TOOL_LIBRARY,\n        ROCPROF_SDK_LIBRARY,\n    ]";
const ROCPROF_RUN_ROOT_BLOCK_V1: &str = "    update_env(\"ROCPROFILER_LIBRARY_CTOR\", True)\n\n    ROCPROFV3_DIR = os.path.dirname(os.path.realpath(__file__))\n    ROCM_DIR = os.path.dirname(ROCPROFV3_DIR)\n    if args.rocm_root is not None:\n        ROCM_DIR = os.path.abspath(args.rocm_root)";

fn derive_installed_collector_adapter_v1(source: &str) -> Result<Vec<u8>, String> {
    for (symbol, first, second) in [
        (
            "ROCPROF_TOOL_LIBRARY",
            ROCPROF_TOOL_ASSIGNMENT_V1,
            ROCPROF_TOOL_RESOLUTION_V1,
        ),
        (
            "ROCPROF_SDK_LIBRARY",
            ROCPROF_CORE_ASSIGNMENT_V1,
            ROCPROF_CORE_RESOLUTION_V1,
        ),
    ] {
        let mut count = 0_usize;
        let mut exact = true;
        for line in source.lines().filter(|line| {
            line.trim_start()
                .strip_prefix(symbol)
                .is_some_and(|tail| tail.trim_start().starts_with('='))
        }) {
            count = count
                .checked_add(1)
                .ok_or_else(|| format!("{symbol} assignment count overflow"))?;
            exact &= match count {
                1 => line == first,
                2 => line == second,
                _ => false,
            };
        }
        if count != 2 || !exact {
            return Err(format!(
                "unsupported rocprofv3 script: expected one exact {symbol} source assignment followed by its exact resolver assignment"
            ));
        }
    }
    if source.matches(ROCPROF_PRELOAD_ORDER_V1).count() != 1 {
        return Err(
            "unsupported rocprofv3 script: expected one exact SDK preload order".to_owned(),
        );
    }
    if source.matches(ROCPROF_RUN_ROOT_BLOCK_V1).count() != 1 {
        return Err(
            "unsupported rocprofv3 script: expected one exact run root-discovery block".to_owned(),
        );
    }
    let adapted = source
        .replacen(
            ROCPROF_RUN_ROOT_BLOCK_V1,
            &format!(
                "    update_env(\"ROCPROFILER_LIBRARY_CTOR\", True)\n\n    ROCM_DIR = app_env.pop(\"{LOGICAL_ROCM_ROOT_ENV_V1}\")"
            ),
            1,
        )
        .replacen(
            ROCPROF_TOOL_ASSIGNMENT_V1,
            &format!("    ROCPROF_TOOL_LIBRARY = app_env.pop(\"{SEALED_TOOL_ENV_V1}\")"),
            1,
        )
        .replacen(
            ROCPROF_CORE_ASSIGNMENT_V1,
            &format!("    ROCPROF_SDK_LIBRARY = app_env.pop(\"{SEALED_CORE_ENV_V1}\")"),
            1,
        )
        .replacen(
            ROCPROF_PRELOAD_ORDER_V1,
            "    append_preload = [\n        ROCPROF_SDK_LIBRARY,\n        ROCPROF_TOOL_LIBRARY,\n    ]",
            1,
        )
        .into_bytes();
    if adapted.len() > MAX_TOOL_BYTES as usize {
        return Err("sealed rocprofv3 adapter exceeds the script byte bound".to_owned());
    }
    Ok(adapted)
}

impl CollectorExecutionV1 {
    fn prepare(script: &FilePin, libraries: Option<&CollectorLibraries>) -> Result<Self, String> {
        let Some(libraries) = libraries else {
            return Ok(Self::ExactScript {
                bootstrap: SealedImage::from_bytes(
                    SEALED_COLLECTOR_BOOTSTRAP_V1.as_bytes(),
                    true,
                    "rocprofv3 bootstrap",
                )?,
            });
        };
        if script.digest != INSTALLED_ROCPROFV3_724_SCRIPT_SHA256_V1
            || script.identity.size != INSTALLED_ROCPROFV3_724_SCRIPT_LENGTH_V1
        {
            return Err(
                "unsupported standard-layout rocprofv3 script identity; sealed adaptation is allowlisted only for ROCprofiler SDK 97f5574"
                    .to_owned(),
            );
        }
        let source = script.read_retained("rocprofv3 script", MAX_TOOL_BYTES)?;
        let source = std::str::from_utf8(&source)
            .map_err(|_| "installed rocprofv3 script is not UTF-8".to_owned())?;
        let adapted = derive_installed_collector_adapter_v1(source)?;
        let digest = Sha256::digest(&adapted).into();
        let length = adapted.len() as u64;
        let image = SealedImage::from_bytes(&adapted, true, "rocprofv3 adapter")?;
        let bootstrap = SealedImage::from_bytes(
            SEALED_COLLECTOR_BOOTSTRAP_V1.as_bytes(),
            true,
            "rocprofv3 bootstrap",
        )?;
        libraries.validate()?;
        Ok(Self::InstalledAdapter {
            bootstrap,
            image,
            digest,
            length,
        })
    }

    fn validate(&self) -> Result<(), String> {
        match self {
            Self::ExactScript { bootstrap } => bootstrap.validate("rocprofv3 bootstrap"),
            Self::InstalledAdapter {
                bootstrap,
                image,
                digest,
                length,
            } => {
                bootstrap.validate("rocprofv3 bootstrap")?;
                image.validate("rocprofv3 adapter")?;
                if image.digest != *digest || image.identity.size != *length {
                    return Err("sealed rocprofv3 adapter identity changed".to_owned());
                }
                Ok(())
            }
        }
    }

    fn bootstrap_path(&self) -> PathBuf {
        match self {
            Self::ExactScript { bootstrap } | Self::InstalledAdapter { bootstrap, .. } => {
                bootstrap.external_path()
            }
        }
    }

    fn source_path(&self, original: &FilePin) -> PathBuf {
        match self {
            Self::ExactScript { .. } => original.external_path(),
            Self::InstalledAdapter { image, .. } => image.external_path(),
        }
    }

    const fn mode_name(&self) -> &'static str {
        match self {
            Self::ExactScript { .. } => "sealed-exact-script-v1",
            Self::InstalledAdapter { .. } => "sealed-installed-adapter-v1",
        }
    }
}

impl CollectorLibraries {
    fn maybe_open(script: &FilePin) -> Result<Option<Self>, String> {
        let Some(bin) = script.canonical_path.parent() else {
            return Ok(None);
        };
        if bin.file_name() != Some(OsStr::new("bin")) {
            return Ok(None);
        }
        let root = bin
            .parent()
            .ok_or("rocprofv3 bin directory has no canonical ROCm root")?;
        let tool = root.join("lib/rocprofiler-sdk/librocprofiler-sdk-tool.so");
        let core = root.join("lib/librocprofiler-sdk.so");
        let tool_exists = tool
            .try_exists()
            .map_err(|error| format!("failed to inspect rocprofiler SDK tool route: {error}"))?;
        let core_exists = core
            .try_exists()
            .map_err(|error| format!("failed to inspect rocprofiler SDK core route: {error}"))?;
        match (tool_exists, core_exists) {
            (false, false) => Ok(None),
            (true, true) => Self::open(script).map(Some),
            _ => Err("rocprofv3 installation has an incomplete SDK library pair".to_owned()),
        }
    }

    fn open(script: &FilePin) -> Result<Self, String> {
        let root = script
            .canonical_path
            .parent()
            .and_then(Path::parent)
            .ok_or("rocprofv3 script has no canonical ROCm root")?;
        let tool_route = root.join("lib/rocprofiler-sdk/librocprofiler-sdk-tool.so");
        let core_route = root.join("lib/librocprofiler-sdk.so");
        let tool_path = fs::canonicalize(&tool_route).map_err(|error| {
            format!(
                "failed to resolve rocprofiler SDK tool library {}: {error}",
                tool_route.display()
            )
        })?;
        let core_path = fs::canonicalize(&core_route).map_err(|error| {
            format!(
                "failed to resolve rocprofiler SDK core library {}: {error}",
                core_route.display()
            )
        })?;
        let libraries = Self {
            tool_route,
            tool: FilePin::open(
                &tool_path,
                "rocprofiler SDK tool library",
                MAX_COLLECTOR_LIBRARY_BYTES,
                false,
            )?,
            core_route,
            core: FilePin::open(
                &core_path,
                "rocprofiler SDK core library",
                MAX_COLLECTOR_LIBRARY_BYTES,
                false,
            )?,
        };
        libraries.validate()?;
        Ok(libraries)
    }

    fn validate(&self) -> Result<(), String> {
        self.tool.validate("rocprofiler SDK tool library")?;
        self.core.validate("rocprofiler SDK core library")?;
        for (route, expected, label) in [
            (&self.tool_route, &self.tool.canonical_path, "SDK tool"),
            (&self.core_route, &self.core.canonical_path, "SDK core"),
        ] {
            let current = fs::canonicalize(route)
                .map_err(|error| format!("rocprofiler {label} route changed: {error}"))?;
            if &current != expected {
                return Err(format!("rocprofiler {label} route was substituted"));
            }
        }
        Ok(())
    }
}

pub(crate) fn command(args: &[String]) -> Result<CommandReport, String> {
    if matches!(args, [arg] if arg == "--help" || arg == "-h") {
        return Ok(CommandReport {
            output: usage().to_owned(),
            succeeded: true,
        });
    }
    let options = parse_options(args)?;
    let supplied_authorization = options.authorization;
    let action = options.action;
    let plan = prepare_plan(options)?;
    if action == Action::Plan {
        return Ok(CommandReport {
            output: render_plan(&plan),
            succeeded: true,
        });
    }
    if plan.options.kind == ProfileKind::Att {
        return Err(
            "ATT collection is unavailable under sealed execution: the rocprofiler decoder API requires a mutable directory namespace and no mutation-proof sealed decoder route is implemented"
                .to_owned(),
        );
    }
    if supplied_authorization != Some(plan.authorization) {
        return Err(format!(
            "profile collection authorization does not match this exact plan; rerun without --collect and pass --authorize-collection {}",
            hex(&plan.authorization)
        ));
    }
    collect(plan)
}

fn usage() -> &'static str {
    "usage: cargo fe2o3 profile [--kind dispatch-json|dispatch-csv|att] [--tool /absolute/path/to/rocprofv3] [--python /absolute/path/to/python3] --output-dir /absolute/new/directory [--cwd /absolute/directory] [--timeout-ms N] [--stdout-limit N] [--stderr-limit N] [--storage-limit N] [--kir-v7 /absolute/path/to/canonical.kir] [--collect --authorize-collection HEX] -- <program> [arguments...]"
}

fn parse_options(args: &[String]) -> Result<Options, String> {
    if args.len() > MAX_ARGUMENTS
        || args
            .iter()
            .any(|argument| argument.len() > MAX_ARGUMENT_BYTES || argument.contains('\0'))
    {
        return Err("profile arguments exceed the bounded UTF-8 argument policy".to_owned());
    }
    let separator = args
        .iter()
        .position(|argument| argument == "--")
        .ok_or_else(|| {
            format!(
                "profile requires `--` before the target program\n{}",
                usage()
            )
        })?;
    let mut action = Action::Plan;
    let mut action_explicit = false;
    let mut authorization = None;
    let mut tool = None;
    let mut interpreter = None;
    let mut output_directory = None;
    let mut working_directory = None;
    let mut kind = ProfileKind::DispatchJson;
    let mut kind_explicit = false;
    let mut timeout = Duration::from_millis(DEFAULT_TIMEOUT_MS);
    let mut stdout_limit = DEFAULT_OUTPUT_LIMIT;
    let mut stderr_limit = DEFAULT_OUTPUT_LIMIT;
    let mut storage_limit = DEFAULT_STORAGE_LIMIT;
    let mut kir_digest = None;
    let mut kir_length = None;
    let mut wave_width = None;
    let mut kir_v7_path = None;
    let mut scalar_options = BTreeSet::new();

    let mut index = 0;
    while index < separator {
        let argument = &args[index];
        if argument == "--collect" || argument == "--print-plan" {
            let requested = if argument == "--collect" {
                Action::Collect
            } else {
                Action::Plan
            };
            if action_explicit {
                return Err("profile action was specified more than once".to_owned());
            }
            action = requested;
            action_explicit = true;
        } else if let Some(value) =
            option_value(args, &mut index, separator, "--authorize-collection")?
        {
            set_once(
                &mut authorization,
                parse_hex(value, "--authorize-collection")?,
                "--authorize-collection",
            )?;
        } else if let Some(value) = option_value(args, &mut index, separator, "--tool")? {
            set_once(&mut tool, PathBuf::from(value), "--tool")?;
        } else if let Some(value) = option_value(args, &mut index, separator, "--python")? {
            set_once(&mut interpreter, PathBuf::from(value), "--python")?;
        } else if let Some(value) = option_value(args, &mut index, separator, "--output-dir")? {
            set_once(&mut output_directory, PathBuf::from(value), "--output-dir")?;
        } else if let Some(value) = option_value(args, &mut index, separator, "--cwd")? {
            set_once(&mut working_directory, PathBuf::from(value), "--cwd")?;
        } else if let Some(value) = option_value(args, &mut index, separator, "--kind")? {
            if kind_explicit {
                return Err("--kind was specified more than once".to_owned());
            }
            kind = match value {
                "dispatch-json" => ProfileKind::DispatchJson,
                "dispatch-csv" => ProfileKind::DispatchCsv,
                "att" => ProfileKind::Att,
                _ => return Err("--kind must be dispatch-json, dispatch-csv, or att".to_owned()),
            };
            kind_explicit = true;
        } else if let Some(value) = option_value(args, &mut index, separator, "--timeout-ms")? {
            require_first(&mut scalar_options, "--timeout-ms")?;
            timeout = Duration::from_millis(parse_u64(value, "--timeout-ms", 1, MAX_TIMEOUT_MS)?);
        } else if let Some(value) = option_value(args, &mut index, separator, "--stdout-limit")? {
            require_first(&mut scalar_options, "--stdout-limit")?;
            stdout_limit = parse_u64(value, "--stdout-limit", 1, MAX_OUTPUT_LIMIT as u64)? as usize;
        } else if let Some(value) = option_value(args, &mut index, separator, "--stderr-limit")? {
            require_first(&mut scalar_options, "--stderr-limit")?;
            stderr_limit = parse_u64(value, "--stderr-limit", 1, MAX_OUTPUT_LIMIT as u64)? as usize;
        } else if let Some(value) = option_value(args, &mut index, separator, "--storage-limit")? {
            require_first(&mut scalar_options, "--storage-limit")?;
            storage_limit = parse_u64(value, "--storage-limit", 1, MAX_STORAGE_LIMIT)?;
        } else if let Some(value) = option_value(args, &mut index, separator, "--kir-sha256")? {
            set_once(
                &mut kir_digest,
                parse_hex(value, "--kir-sha256")?,
                "--kir-sha256",
            )?;
        } else if let Some(value) = option_value(args, &mut index, separator, "--kir-len")? {
            set_once(
                &mut kir_length,
                parse_u64(value, "--kir-len", 1, u64::MAX)?,
                "--kir-len",
            )?;
        } else if let Some(value) = option_value(args, &mut index, separator, "--wave-width")? {
            let parsed = match value {
                "32" => 32,
                "64" => 64,
                _ => return Err("--wave-width must be 32 or 64".to_owned()),
            };
            set_once(&mut wave_width, parsed, "--wave-width")?;
        } else if let Some(value) = option_value(args, &mut index, separator, "--kir-v7")? {
            set_once(&mut kir_v7_path, PathBuf::from(value), "--kir-v7")?;
        } else {
            return Err(format!("unknown profile option `{argument}`\n{}", usage()));
        }
        index += 1;
    }
    if action == Action::Plan && authorization.is_some() {
        return Err("--authorize-collection is valid only with --collect".to_owned());
    }
    if action == Action::Collect && authorization.is_none() {
        return Err(
            "--collect requires --authorize-collection from the exact dry-run plan".to_owned(),
        );
    }
    let output_directory = output_directory.ok_or("profile requires --output-dir")?;
    let program = args
        .get(separator + 1)
        .filter(|program| !program.is_empty())
        .cloned()
        .ok_or_else(|| format!("profile requires a nonempty target program\n{}", usage()))?;
    let kir_binding = match (kir_digest, kir_length, wave_width) {
        (None, None, None) => None,
        (Some(digest), Some(length), Some(wave_width)) => Some(KirBinding {
            digest,
            length,
            wave_width,
        }),
        _ => {
            return Err(
                "--kir-sha256, --kir-len, and --wave-width must be specified together".to_owned(),
            );
        }
    };
    if kind == ProfileKind::Att && kir_binding.is_some() {
        return Err("KIR dispatch binding options are not valid for ATT collection".to_owned());
    }
    if kir_v7_path.is_some() && kir_binding.is_some() {
        return Err("--kir-v7 cannot be combined with legacy KIR declaration options".to_owned());
    }
    if kind == ProfileKind::Att && kir_v7_path.is_some() {
        return Err("--kir-v7 is not valid for ATT collection".to_owned());
    }
    Ok(Options {
        action,
        authorization,
        tool,
        interpreter,
        output_directory,
        working_directory,
        kind,
        timeout,
        stdout_limit,
        stderr_limit,
        storage_limit,
        program,
        program_arguments: args[separator + 2..].to_vec(),
        kir_binding,
        kir_v7_path,
    })
}

fn option_value<'a>(
    args: &'a [String],
    index: &mut usize,
    separator: usize,
    name: &str,
) -> Result<Option<&'a str>, String> {
    let argument = &args[*index];
    if argument == name {
        *index += 1;
        if *index >= separator {
            return Err(format!("{name} requires a value before `--`"));
        }
        return Ok(Some(args[*index].as_str()));
    }
    Ok(argument
        .strip_prefix(name)
        .and_then(|rest| rest.strip_prefix('=')))
}

fn set_once<T>(slot: &mut Option<T>, value: T, name: &str) -> Result<(), String> {
    if slot.replace(value).is_some() {
        return Err(format!("{name} was specified more than once"));
    }
    Ok(())
}

fn require_first(seen: &mut BTreeSet<&'static str>, name: &'static str) -> Result<(), String> {
    if !seen.insert(name) {
        return Err(format!("{name} was specified more than once"));
    }
    Ok(())
}

fn parse_u64(value: &str, name: &str, minimum: u64, maximum: u64) -> Result<u64, String> {
    if value.is_empty() || (value.len() > 1 && value.starts_with('0')) {
        return Err(format!("{name} must use canonical decimal encoding"));
    }
    let parsed = value
        .parse::<u64>()
        .map_err(|_| format!("{name} must be an integer"))?;
    if !(minimum..=maximum).contains(&parsed) {
        return Err(format!("{name} must be between {minimum} and {maximum}"));
    }
    Ok(parsed)
}

fn parse_hex(value: &str, name: &str) -> Result<[u8; 32], String> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(format!(
            "{name} must be exactly 64 lowercase hexadecimal digits"
        ));
    }
    let mut output = [0_u8; 32];
    for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
        let high = hex_nibble(pair[0]);
        let low = hex_nibble(pair[1]);
        output[index] = (high << 4) | low;
    }
    Ok(output)
}

fn hex_nibble(byte: u8) -> u8 {
    match byte {
        b'0'..=b'9' => byte - b'0',
        b'a'..=b'f' => byte - b'a' + 10,
        _ => 0,
    }
}

fn prepare_plan(options: Options) -> Result<Plan, String> {
    let working_directory = resolve_directory(options.working_directory.as_deref(), "--cwd")?;
    let output_directory = resolve_new_output(&options.output_directory)?;
    let selected_tool = options.tool.clone().unwrap_or_else(default_tool_path);
    require_absolute_named(&selected_tool, "--tool", "rocprofv3")?;
    let tool = FilePin::open(&selected_tool, "rocprofv3 script", MAX_TOOL_BYTES, true)?;
    if !tool.prefix.starts_with(b"#!")
        || !tool
            .prefix
            .windows(b"--kernel-trace".len())
            .any(|window| window == b"--kernel-trace")
        || !tool
            .prefix
            .windows(b"--advanced-thread-trace".len())
            .any(|window| window == b"--advanced-thread-trace")
    {
        return Err(
            "rocprofv3 script does not expose the reviewed kernel-trace and ATT option surface"
                .to_owned(),
        );
    }
    let selected_interpreter = options
        .interpreter
        .clone()
        .or_else(discover_python)
        .ok_or("a native python3.12 or python3.13 interpreter was not found")?;
    require_absolute_python(&selected_interpreter)?;
    let interpreter = FilePin::open(
        &selected_interpreter,
        "rocprofv3 Python interpreter",
        MAX_INTERPRETER_BYTES,
        true,
    )?;
    require_elf(&interpreter, "rocprofv3 Python interpreter")?;
    let collector_libraries = CollectorLibraries::maybe_open(&tool)?;
    let collector_execution = CollectorExecutionV1::prepare(&tool, collector_libraries.as_ref())?;
    let collector_tool_bytes = canonical_collector_tool(
        &tool,
        &interpreter,
        collector_libraries.as_ref(),
        &collector_execution,
    );
    let collector_tool_digest = Sha256::digest(&collector_tool_bytes).into();

    let requested_target = PathBuf::from(&options.program);
    let target_path = if requested_target.is_absolute() {
        requested_target
    } else {
        working_directory.join(requested_target)
    };
    let target = FilePin::open(&target_path, "profile target", MAX_TARGET_BYTES, true)?;
    require_elf(&target, "profile target")?;
    let environment = capture_environment();
    let environment_bytes = canonical_environment(&environment);
    let environment_digest = Sha256::digest(&environment_bytes).into();
    let devices = discover_visible_devices()?;
    let verified_kir_v7 = match options.kir_v7_path.as_deref() {
        Some(path) => Some(admit_kir_v7(path, &devices)?),
        None => None,
    };
    let collector_arguments =
        collector_arguments(options.kind, &output_directory, &target, &options)?;
    let configuration = canonical_configuration(options.kind, &devices);
    let configuration_digest = Sha256::digest(&configuration).into();
    let authorization = authorization_digest(AuthorizationInputs {
        options: &options,
        working_directory: &working_directory,
        output_directory: &output_directory,
        tool: &tool,
        interpreter: &interpreter,
        target: &target,
        environment: &environment_digest,
        devices: &devices,
        configuration: &configuration_digest,
        collector_tool: &collector_tool_digest,
        verified_kir_v7: verified_kir_v7.as_ref(),
    });
    Ok(Plan {
        options,
        working_directory,
        output_directory,
        tool,
        interpreter,
        collector_libraries,
        collector_execution,
        collector_tool_bytes,
        collector_tool_digest,
        target,
        environment,
        environment_bytes,
        environment_digest,
        devices,
        configuration,
        configuration_digest,
        collector_arguments,
        authorization,
        verified_kir_v7,
    })
}

fn admit_kir_v7(path: &Path, devices: &[DeviceIdentity]) -> Result<VerifiedKirInputV1, String> {
    if !path.is_absolute() {
        return Err("--kir-v7 must be an absolute path".to_owned());
    }
    let pin = FilePin::open(
        path,
        "canonical Kernel IR V7",
        MAX_MODULE_BYTES_V1 as u64,
        false,
    )?;
    let bytes = pin.read_retained("canonical Kernel IR V7", MAX_MODULE_BYTES_V1 as u64)?;
    let (owner, module) = VerifiedCanonicalKernelIrV7::from_canonical_bytes_with_module(bytes)
        .map_err(|error| format!("--kir-v7 is not exact verified canonical KIR V7: {error}"))?;
    owner
        .revalidate()
        .map_err(|error| format!("retained canonical Kernel IR V7 failed revalidation: {error}"))?;
    let compatibility = kir_target_compatibility_v1(&module, devices);
    Ok(VerifiedKirInputV1 {
        pin,
        owner,
        compatibility,
    })
}

fn kir_target_compatibility_v1(
    module: &Module,
    devices: &[DeviceIdentity],
) -> KirTargetCompatibilityV1 {
    if devices.is_empty() {
        return KirTargetCompatibilityV1::Unavailable(
            KirTargetUnavailableReasonV1::KfdProfileUnavailable,
        );
    }
    let capabilities = module
        .effective_capabilities()
        .into_iter()
        .chain(
            module
                .functions
                .iter()
                .flat_map(|function| function.effective_capabilities()),
        )
        .chain(
            module
                .kernels
                .iter()
                .flat_map(|kernel| kernel.required_capabilities.iter().cloned()),
        )
        .collect::<BTreeSet<_>>();
    let mut exact_target = None;
    let mut saw_wave64 = false;
    for capability in &capabilities {
        match capability {
            TargetCapability::Extension { namespace, name }
                if namespace == AMDGPU_EXACT_TARGET_CAPABILITY_NAMESPACE =>
            {
                let target = match name.as_str() {
                    AMDGPU_GFX942_XNACK_MINUS_TARGET_CAPABILITY_NAME => {
                        ObservedGpuTargetProfileV1::Gfx942
                    }
                    AMDGPU_GFX950_XNACK_MINUS_TARGET_CAPABILITY_NAME => {
                        ObservedGpuTargetProfileV1::Gfx950
                    }
                    _ => {
                        return KirTargetCompatibilityV1::Unavailable(
                            KirTargetUnavailableReasonV1::UnknownExactTarget,
                        );
                    }
                };
                if exact_target
                    .replace(target)
                    .is_some_and(|prior| prior != target)
                {
                    return KirTargetCompatibilityV1::Unavailable(
                        KirTargetUnavailableReasonV1::ConflictingExactTargets,
                    );
                }
            }
            TargetCapability::WaveWidth(WaveWidth::Wave64) => saw_wave64 = true,
            TargetCapability::WaveWidth(_) => {
                return KirTargetCompatibilityV1::Unavailable(
                    KirTargetUnavailableReasonV1::ConflictingWaveWidth,
                );
            }
            _ => {}
        }
    }
    let Some(exact_target) = exact_target else {
        return KirTargetCompatibilityV1::Unavailable(
            KirTargetUnavailableReasonV1::MissingExactTarget,
        );
    };
    if !saw_wave64 {
        return KirTargetCompatibilityV1::Unavailable(KirTargetUnavailableReasonV1::MissingWave64);
    }
    for device in devices {
        let ObservedGpuTargetProfileStatusV1::Observed(observed) = device.target_profile.status
        else {
            return KirTargetCompatibilityV1::Unavailable(
                KirTargetUnavailableReasonV1::KfdProfileUnavailable,
            );
        };
        if observed != exact_target || device.target_profile.wave_width != PRODUCTION_WAVE_WIDTH {
            return KirTargetCompatibilityV1::Unavailable(
                KirTargetUnavailableReasonV1::KfdFamilyMismatch,
            );
        }
    }
    KirTargetCompatibilityV1::Ready(exact_target)
}

fn default_tool_path() -> PathBuf {
    for path in [
        "/opt/rocm/bin/rocprofv3",
        "/opt/rocm-7.2.0/bin/rocprofv3",
        "/opt/rocm-7.1.0/bin/rocprofv3",
    ] {
        if Path::new(path).is_file() {
            return PathBuf::from(path);
        }
    }
    PathBuf::from("/opt/rocm/bin/rocprofv3")
}

fn discover_python() -> Option<PathBuf> {
    ["/usr/bin/python3.12", "/usr/bin/python3.13"]
        .into_iter()
        .map(PathBuf::from)
        .find(|path| path.is_file())
}

fn require_absolute_named(path: &Path, option: &str, name: &str) -> Result<(), String> {
    if !path.is_absolute() || path.file_name() != Some(OsStr::new(name)) {
        return Err(format!("{option} must be an absolute path named {name}"));
    }
    Ok(())
}

fn require_absolute_python(path: &Path) -> Result<(), String> {
    if !path.is_absolute()
        || !matches!(
            path.file_name().and_then(OsStr::to_str),
            Some("python3.12" | "python3.13")
        )
    {
        return Err("--python must be an absolute native python3.12 or python3.13 path".to_owned());
    }
    Ok(())
}

fn require_elf(pin: &FilePin, label: &str) -> Result<(), String> {
    if !pin.prefix.starts_with(b"\x7fELF") {
        return Err(format!("{label} must be a native ELF executable"));
    }
    Ok(())
}

fn resolve_directory(requested: Option<&Path>, option: &str) -> Result<PathBuf, String> {
    let path = match requested {
        Some(path) if path.is_absolute() => path.to_path_buf(),
        Some(_) => return Err(format!("{option} must be absolute")),
        None => env::current_dir()
            .map_err(|error| format!("failed to read current directory: {error}"))?,
    };
    let canonical = fs::canonicalize(&path).map_err(|error| {
        format!(
            "failed to canonicalize {option} {}: {error}",
            path.display()
        )
    })?;
    if !canonical.is_dir() {
        return Err(format!("{option} is not a directory"));
    }
    if canonical.as_os_str().as_encoded_bytes().len() > MAX_ARGUMENT_BYTES {
        return Err(format!("canonical {option} exceeds the path bound"));
    }
    Ok(canonical)
}

fn resolve_new_output(path: &Path) -> Result<PathBuf, String> {
    if !path.is_absolute() {
        return Err("--output-dir must be absolute".to_owned());
    }
    if fs::symlink_metadata(path).is_ok() {
        return Err("--output-dir must not already exist".to_owned());
    }
    let leaf = path
        .file_name()
        .ok_or("--output-dir must have a final component")?;
    if matches!(leaf.to_str(), None | Some("" | "." | "..")) {
        return Err("--output-dir has an invalid final component".to_owned());
    }
    let parent = path.parent().ok_or("--output-dir must have a parent")?;
    let parent = fs::canonicalize(parent)
        .map_err(|error| format!("failed to canonicalize --output-dir parent: {error}"))?;
    if !parent.is_dir() {
        return Err("--output-dir parent is not a directory".to_owned());
    }
    let output = parent.join(leaf);
    if output.as_os_str().as_encoded_bytes().len() > MAX_ARGUMENT_BYTES {
        return Err("canonical --output-dir exceeds the path bound".to_owned());
    }
    if output.to_str().is_none() {
        return Err("canonical --output-dir must be valid UTF-8".to_owned());
    }
    Ok(output)
}

fn capture_environment() -> Vec<EnvironmentEntry> {
    let mut entries = Vec::new();
    for &name in ALLOWED_ENVIRONMENT {
        if let Some(value) = env::var_os(name) {
            entries.push(EnvironmentEntry { name, value });
        }
    }
    for (name, value) in [("LANG", "C"), ("LC_ALL", "C")] {
        if let Some(entry) = entries.iter_mut().find(|entry| entry.name == name) {
            entry.value = value.into();
        } else {
            entries.push(EnvironmentEntry {
                name,
                value: value.into(),
            });
        }
    }
    entries.sort_by_key(|entry| entry.name);
    entries
}

fn canonical_environment(entries: &[EnvironmentEntry]) -> Vec<u8> {
    let mut output = b"fe2o3-profile-environment-v1\0".to_vec();
    for entry in entries {
        append_field(&mut output, entry.name.as_bytes());
        append_field(&mut output, entry.value.as_encoded_bytes());
    }
    output
}

fn discover_devices(root: &Path) -> io::Result<Vec<DeviceIdentity>> {
    let mut nodes = fs::read_dir(root)
        .map_err(|error| {
            io::Error::new(
                error.kind(),
                format!(
                    "direct-KFD topology is unavailable at {}: {error}",
                    root.display()
                ),
            )
        })?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| {
            io::Error::new(
                error.kind(),
                format!("failed to enumerate direct-KFD topology: {error}"),
            )
        })?;
    if nodes.len() > MAX_DEVICES + 16 {
        return Err(invalid_data("direct-KFD topology exceeds the node bound"));
    }
    nodes.sort_by_key(|entry| entry.file_name());
    let mut devices = Vec::new();
    let mut stable = BTreeSet::new();
    for entry in nodes {
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            return Err(invalid_data(
                "direct-KFD topology contains a non-UTF-8 node",
            ));
        };
        let Ok(node) = name.parse::<u32>() else {
            continue;
        };
        if node.to_string() != name {
            return Err(invalid_data(
                "direct-KFD topology node uses noncanonical numbering",
            ));
        }
        let properties_path = entry.path().join("properties");
        let gpu_id = read_kfd_scalar(&entry.path().join("gpu_id"), node, "gpu_id")?;
        if gpu_id == 0 {
            continue;
        }
        let metadata = fs::symlink_metadata(&properties_path).map_err(|error| {
            io::Error::new(
                error.kind(),
                format!("failed to inspect KFD node {node}: {error}"),
            )
        })?;
        if metadata.file_type().is_symlink()
            || !metadata.is_file()
            || metadata.len() > MAX_TOPOLOGY_BYTES
        {
            return Err(invalid_data(format!(
                "KFD node {node} properties are not a bounded regular file"
            )));
        }
        let mut properties = Vec::new();
        File::open(&properties_path)
            .and_then(|file| {
                file.take(MAX_TOPOLOGY_BYTES + 1)
                    .read_to_end(&mut properties)
            })
            .map_err(|error| {
                io::Error::new(
                    error.kind(),
                    format!("failed to read KFD node {node}: {error}"),
                )
            })?;
        if properties.len() as u64 > MAX_TOPOLOGY_BYTES {
            return Err(invalid_data(format!(
                "KFD node {node} exceeds the properties bound"
            )));
        }
        let after = fs::symlink_metadata(&properties_path).map_err(|error| {
            io::Error::new(
                error.kind(),
                format!("failed to re-inspect KFD node {node}: {error}"),
            )
        })?;
        if ObjectIdentity::from_metadata(&after) != ObjectIdentity::from_metadata(&metadata) {
            return Err(invalid_data(format!(
                "KFD node {node} changed while it was read"
            )));
        }
        let parsed = parse_properties(&properties).map_err(invalid_data)?;
        if parsed.get("simd_count").is_none_or(|value| value == "0") {
            continue;
        }
        let fields = [
            "unique_id",
            "vendor_id",
            "device_id",
            "domain",
            "location_id",
            "gfx_target_version",
            "wave_front_size",
            "num_xcc",
        ];
        let unique = parsed
            .get("unique_id")
            .ok_or_else(|| invalid_data(format!("GPU KFD node {node} has no unique_id")))?;
        if unique == "0" || !stable.insert(unique.clone()) {
            return Err(invalid_data(
                "GPU KFD topology has a missing or duplicate stable unique_id",
            ));
        }
        let hardware = KfdGpuHardwareV1 {
            gpu_id,
            simd_count: required_u64_property(&parsed, node, "simd_count")?,
            vendor_id: required_u64_property(&parsed, node, "vendor_id")?,
            device_id: required_u64_property(&parsed, node, "device_id")?,
            location_id: required_u64_property(&parsed, node, "location_id")?,
            domain: required_u64_property(&parsed, node, "domain")?,
            gfx_target_version: required_u64_property(&parsed, node, "gfx_target_version")?,
            wave_front_size: required_u64_property(&parsed, node, "wave_front_size")?,
            num_xcc: required_u64_property(&parsed, node, "num_xcc")?,
        };
        let target_profile = ObservedGpuTargetProfileRecordV1::from_direct_kfd_properties(
            hardware.vendor_id,
            hardware.gfx_target_version,
            hardware.wave_front_size,
        );
        let mut bytes = b"fe2o3-kfd-stable-device-v1\n".to_vec();
        bytes.extend_from_slice(b"gpu_id=");
        bytes.extend_from_slice(gpu_id.to_string().as_bytes());
        bytes.push(b'\n');
        for field in fields {
            let value = parsed
                .get(field)
                .ok_or_else(|| invalid_data(format!("GPU KFD node {node} lacks {field}")))?;
            bytes.extend_from_slice(field.as_bytes());
            bytes.push(b'=');
            bytes.extend_from_slice(value.as_bytes());
            bytes.push(b'\n');
        }
        devices.push(DeviceIdentity {
            node,
            hardware,
            digest: Sha256::digest(&bytes).into(),
            bytes,
            target_profile,
        });
        if devices.len() > MAX_DEVICES {
            return Err(invalid_data(
                "direct-KFD GPU count exceeds the device bound",
            ));
        }
    }
    devices.sort_by_key(|device| device.digest);
    Ok(devices)
}

fn read_kfd_scalar(path: &Path, node: u32, field: &'static str) -> io::Result<u64> {
    let metadata = fs::symlink_metadata(path).map_err(|error| {
        io::Error::new(
            error.kind(),
            format!("failed to inspect KFD node {node} {field}: {error}"),
        )
    })?;
    if metadata.file_type().is_symlink()
        || !metadata.is_file()
        || metadata.len() > MAX_KFD_SCALAR_BACKING_BYTES
    {
        return Err(invalid_data(format!(
            "KFD node {node} {field} is not a bounded regular file"
        )));
    }
    let mut bytes = Vec::new();
    File::open(path)
        .and_then(|file| {
            file.take(MAX_KFD_SCALAR_CONTENT_BYTES + 1)
                .read_to_end(&mut bytes)
        })
        .map_err(|error| {
            io::Error::new(
                error.kind(),
                format!("failed to read KFD node {node} {field}: {error}"),
            )
        })?;
    if bytes.len() as u64 > MAX_KFD_SCALAR_CONTENT_BYTES {
        return Err(invalid_data(format!(
            "KFD node {node} {field} exceeds the content bound"
        )));
    }
    let after = fs::symlink_metadata(path).map_err(|error| {
        io::Error::new(
            error.kind(),
            format!("failed to re-inspect KFD node {node} {field}: {error}"),
        )
    })?;
    if ObjectIdentity::from_metadata(&metadata) != ObjectIdentity::from_metadata(&after) {
        return Err(invalid_data(format!(
            "KFD node {node} {field} changed while it was read"
        )));
    }
    let text = std::str::from_utf8(&bytes)
        .map_err(|_| invalid_data(format!("KFD node {node} {field} is not UTF-8")))?;
    let value = text
        .strip_suffix('\n')
        .ok_or_else(|| invalid_data(format!("KFD node {node} {field} lacks newline")))?;
    if value.is_empty()
        || (value.len() > 1 && value.starts_with('0'))
        || !value.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(invalid_data(format!(
            "KFD node {node} {field} is not canonical decimal"
        )));
    }
    value
        .parse()
        .map_err(|_| invalid_data(format!("KFD node {node} {field} is out of range")))
}

fn required_u64_property(
    properties: &BTreeMap<String, String>,
    node: u32,
    field: &'static str,
) -> io::Result<u64> {
    properties
        .get(field)
        .ok_or_else(|| invalid_data(format!("GPU KFD node {node} lacks {field}")))?
        .parse::<u64>()
        .map_err(|_| invalid_data(format!("GPU KFD node {node} has an out-of-range {field}")))
}

fn discover_visible_devices() -> Result<Vec<DeviceIdentity>, String> {
    match discover_devices(Path::new(KFD_TOPOLOGY_ROOT)) {
        Ok(devices) => Ok(devices),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(error) => Err(error.to_string()),
    }
}

fn validate_device_bindings(expected: &[DeviceIdentity]) -> Result<(), String> {
    let observed = discover_visible_devices()?;
    if device_bindings_match(expected, &observed) {
        Ok(())
    } else {
        Err("direct-KFD node-to-device identity mapping changed after planning".to_owned())
    }
}

fn device_bindings_match(expected: &[DeviceIdentity], observed: &[DeviceIdentity]) -> bool {
    expected == observed
}

fn invalid_data(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}

fn parse_properties(bytes: &[u8]) -> Result<BTreeMap<String, String>, String> {
    let text = std::str::from_utf8(bytes).map_err(|_| "KFD properties are not UTF-8")?;
    let mut output = BTreeMap::new();
    for line in text.lines() {
        if line.is_empty() {
            continue;
        }
        let (name, value) = line
            .split_once(' ')
            .ok_or("KFD property is not a name/value pair")?;
        if name.is_empty()
            || value.is_empty()
            || value.contains(' ')
            || !name
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
            || !name.as_bytes()[0].is_ascii_lowercase()
            || !value.bytes().all(|byte| byte.is_ascii_digit())
            || (value.len() > 1 && value.starts_with('0'))
            || output.insert(name.to_owned(), value.to_owned()).is_some()
        {
            return Err("KFD properties contain a malformed or duplicate field".to_owned());
        }
    }
    Ok(output)
}

fn collector_arguments(
    kind: ProfileKind,
    output: &Path,
    target: &FilePin,
    options: &Options,
) -> Result<Vec<String>, String> {
    let output = output
        .to_str()
        .ok_or("canonical --output-dir must be valid UTF-8")?;
    let target = target
        .external_path()
        .to_str()
        .ok_or("retained target descriptor path must be valid UTF-8")?
        .to_owned();
    let mut arguments = vec![
        kind.collector_flag().to_owned(),
        "--agent-index".to_owned(),
        "absolute".to_owned(),
        "--output-format".to_owned(),
        kind.output_format().to_owned(),
        "--output-directory".to_owned(),
        output.to_owned(),
        "--output-file".to_owned(),
        "capture".to_owned(),
        "--".to_owned(),
        target,
    ];
    arguments.extend(options.program_arguments.iter().cloned());
    Ok(arguments)
}

fn canonical_configuration(kind: ProfileKind, devices: &[DeviceIdentity]) -> Vec<u8> {
    let mut bytes = b"fe2o3-rocprofv3-configuration-v2\0".to_vec();
    append_field(&mut bytes, kind.name().as_bytes());
    for argument in [
        kind.collector_flag(),
        "--agent-index",
        "absolute",
        "--output-format",
        kind.output_format(),
    ] {
        append_field(&mut bytes, argument.as_bytes());
    }
    for device in devices {
        append_field(&mut bytes, device.target_profile_record().as_bytes());
    }
    bytes
}

fn canonical_collector_tool(
    script: &FilePin,
    interpreter: &FilePin,
    libraries: Option<&CollectorLibraries>,
    execution: &CollectorExecutionV1,
) -> Vec<u8> {
    let mut bytes = b"fe2o3-rocprofv3-toolchain-v2\0".to_vec();
    append_field(&mut bytes, SEALED_COLLECTOR_ADAPTER_SCHEMA_V1);
    append_field(&mut bytes, execution.mode_name().as_bytes());
    append_field(
        &mut bytes,
        &<[u8; 32]>::from(Sha256::digest(SEALED_COLLECTOR_BOOTSTRAP_V1.as_bytes())),
    );
    append_field(
        &mut bytes,
        &(SEALED_COLLECTOR_BOOTSTRAP_V1.len() as u64).to_le_bytes(),
    );
    for pin in [script, interpreter] {
        append_field(
            &mut bytes,
            pin.canonical_path.as_os_str().as_encoded_bytes(),
        );
        append_field(&mut bytes, &pin.digest);
        append_field(&mut bytes, &pin.identity.size.to_le_bytes());
    }
    if let Some(libraries) = libraries {
        for (route, pin) in [
            (&libraries.core_route, &libraries.core),
            (&libraries.tool_route, &libraries.tool),
        ] {
            append_field(&mut bytes, route.as_os_str().as_encoded_bytes());
            append_field(
                &mut bytes,
                pin.canonical_path.as_os_str().as_encoded_bytes(),
            );
            append_field(&mut bytes, &pin.digest);
            append_field(&mut bytes, &pin.identity.size.to_le_bytes());
        }
    }
    if let CollectorExecutionV1::InstalledAdapter { digest, length, .. } = execution {
        append_field(&mut bytes, digest);
        append_field(&mut bytes, &length.to_le_bytes());
    }
    bytes
}

struct AuthorizationInputs<'a> {
    options: &'a Options,
    working_directory: &'a Path,
    output_directory: &'a Path,
    tool: &'a FilePin,
    interpreter: &'a FilePin,
    target: &'a FilePin,
    environment: &'a [u8; 32],
    devices: &'a [DeviceIdentity],
    configuration: &'a [u8; 32],
    collector_tool: &'a [u8; 32],
    verified_kir_v7: Option<&'a VerifiedKirInputV1>,
}

fn authorization_digest(input: AuthorizationInputs<'_>) -> [u8; 32] {
    let AuthorizationInputs {
        options,
        working_directory,
        output_directory,
        tool,
        interpreter,
        target,
        environment,
        devices,
        configuration,
        collector_tool,
        verified_kir_v7,
    } = input;
    let mut hasher = Sha256::new();
    hash_field(&mut hasher, b"fe2o3-profile-authorization-v1");
    hash_field(&mut hasher, options.kind.name().as_bytes());
    hash_field(&mut hasher, &tool.digest);
    hash_field(&mut hasher, &tool.identity.size.to_le_bytes());
    hash_field(
        &mut hasher,
        tool.canonical_path.as_os_str().as_encoded_bytes(),
    );
    hash_field(&mut hasher, &interpreter.digest);
    hash_field(&mut hasher, &interpreter.identity.size.to_le_bytes());
    hash_field(
        &mut hasher,
        interpreter.canonical_path.as_os_str().as_encoded_bytes(),
    );
    hash_field(&mut hasher, &target.digest);
    hash_field(&mut hasher, &target.identity.size.to_le_bytes());
    hash_field(
        &mut hasher,
        target.canonical_path.as_os_str().as_encoded_bytes(),
    );
    hash_field(
        &mut hasher,
        working_directory.as_os_str().as_encoded_bytes(),
    );
    hash_field(&mut hasher, output_directory.as_os_str().as_encoded_bytes());
    hash_field(&mut hasher, environment);
    hash_field(&mut hasher, configuration);
    hash_field(&mut hasher, collector_tool);
    hash_field(
        &mut hasher,
        &(options.timeout.as_millis() as u64).to_le_bytes(),
    );
    hash_field(&mut hasher, &(options.stdout_limit as u64).to_le_bytes());
    hash_field(&mut hasher, &(options.stderr_limit as u64).to_le_bytes());
    hash_field(&mut hasher, &options.storage_limit.to_le_bytes());
    hash_field(&mut hasher, options.program.as_bytes());
    for argument in &options.program_arguments {
        hash_field(&mut hasher, argument.as_bytes());
    }
    hash_device_bindings(&mut hasher, devices);
    if let Some(kir) = &options.kir_binding {
        hash_field(&mut hasher, &kir.digest);
        hash_field(&mut hasher, &kir.length.to_le_bytes());
        hash_field(&mut hasher, &[kir.wave_width]);
    }
    if let Some(kir) = verified_kir_v7 {
        hash_field(&mut hasher, kir.owner.identity().digest());
        hash_field(
            &mut hasher,
            &kir.owner.identity().canonical_length().to_le_bytes(),
        );
        hash_field(&mut hasher, &kir.pin.digest);
        hash_field(&mut hasher, &kir.pin.identity.size.to_le_bytes());
        hash_field(
            &mut hasher,
            &[match kir.compatibility {
                KirTargetCompatibilityV1::Ready(ObservedGpuTargetProfileV1::Gfx942) => 1,
                KirTargetCompatibilityV1::Ready(ObservedGpuTargetProfileV1::Gfx950) => 2,
                KirTargetCompatibilityV1::Unavailable(reason) => 16 + reason.tag(),
            }],
        );
    }
    hasher.finalize().into()
}

fn hash_device_bindings(hasher: &mut Sha256, devices: &[DeviceIdentity]) {
    for device in devices {
        hash_field(hasher, &device.node.to_le_bytes());
        hash_field(hasher, &device.digest);
        hash_field(hasher, &(device.bytes.len() as u64).to_le_bytes());
        hash_field(hasher, device.target_profile_record().as_bytes());
    }
}

fn render_plan(plan: &Plan) -> String {
    let mut output = String::new();
    line(&mut output, "schema", "fe2o3-profile-plan-v1");
    line(&mut output, "authority", "plan-only");
    line(&mut output, "stateful-action", "not-executed");
    line(&mut output, "collector", "rocprofv3");
    line(&mut output, "profile-kind", plan.options.kind.name());
    line(
        &mut output,
        "collector-execution-mode",
        plan.collector_execution.mode_name(),
    );
    line(
        &mut output,
        "collector-execution-inputs",
        "sealed-read-only-memfd-images-with-original-path-provenance",
    );
    if plan.options.kind == ProfileKind::Att {
        line(&mut output, "collection-readiness", "unavailable");
        line(
            &mut output,
            "collection-unavailable-reason",
            "att-decoder-requires-mutable-directory-namespace-without-sealed-route",
        );
    } else {
        line(
            &mut output,
            "collection-readiness",
            "ready-after-authorization",
        );
    }
    line(
        &mut output,
        "collector-option-surface",
        "option-tokens-present-in-exact-script",
    );
    line(
        &mut output,
        "direct-kfd-target",
        "supported-as-exact-argv-launch",
    );
    line(&mut output, "dispatch-observability-origin", "unavailable");
    line(
        &mut output,
        "dispatch-observability-reason",
        "bundle-v4-import-not-run",
    );
    line(&mut output, "att-observability-origin", "unavailable");
    line(
        &mut output,
        "att-observability-reason",
        "bundle-v4-import-not-run",
    );
    line(
        &mut output,
        "hip-runtime-dependency",
        "none-in-fe2o3-orchestrator",
    );
    line(
        &mut output,
        "hsa-runtime-dependency",
        "none-in-fe2o3-orchestrator",
    );
    line(
        &mut output,
        "collector-runtime-limitation",
        "rocprofv3-injects-rocprofiler-sdk-and-may-not-observe-direct-kfd-submission",
    );
    identity_lines(&mut output, "tool", &plan.tool);
    identity_lines(&mut output, "python", &plan.interpreter);
    line(
        &mut output,
        "collector-tool-identity",
        content_identity(
            &plan.collector_tool_digest,
            plan.collector_tool_bytes.len() as u64,
        ),
    );
    if let Some(libraries) = &plan.collector_libraries {
        line(
            &mut output,
            "collector-tool-identity-scope",
            "launcher-script,native-interpreter,sdk-tool-library,sdk-core-library",
        );
        identity_lines(&mut output, "collector-sdk-tool-library", &libraries.tool);
        identity_lines(&mut output, "collector-sdk-core-library", &libraries.core);
    } else {
        line(
            &mut output,
            "collector-tool-identity-scope",
            "launcher-script,native-interpreter",
        );
        line(
            &mut output,
            "collector-sdk-library-identity-origin",
            "unavailable",
        );
        line(
            &mut output,
            "collector-sdk-library-identity-reason",
            "non-installed-test-or-nonstandard-layout",
        );
    }
    line(
        &mut output,
        "collector-transitive-dynamic-closure-origin",
        "unavailable",
    );
    line(
        &mut output,
        "collector-transitive-dynamic-closure-reason",
        "shared-library-dependency-closure-not-content-bound",
    );
    identity_lines(&mut output, "target", &plan.target);
    line_debug(&mut output, "target-requested", &plan.options.program);
    for (index, argument) in plan.options.program_arguments.iter().enumerate() {
        redacted_value(
            &mut output,
            &format!("target-arg[{index}]"),
            argument.as_bytes(),
        );
    }
    line_debug(&mut output, "working-directory", &plan.working_directory);
    line_debug(&mut output, "output-directory", &plan.output_directory);
    line(
        &mut output,
        "output-directory-policy",
        "new-private-0700-owned-and-bounded",
    );
    line(&mut output, "timeout-ms", plan.options.timeout.as_millis());
    line(&mut output, "stdout-limit", plan.options.stdout_limit);
    line(&mut output, "stderr-limit", plan.options.stderr_limit);
    line(&mut output, "storage-limit", plan.options.storage_limit);
    line(
        &mut output,
        "environment-policy",
        "clear-then-bounded-allowlist",
    );
    line(
        &mut output,
        "environment-identity",
        content_identity(
            &plan.environment_digest,
            plan.environment_bytes.len() as u64,
        ),
    );
    for (index, entry) in plan.environment.iter().enumerate() {
        let digest: [u8; 32] = Sha256::digest(entry.value.as_encoded_bytes()).into();
        line(
            &mut output,
            &format!("environment[{index}]"),
            format!(
                "{}:sha256:{}:{}",
                entry.name,
                hex(&digest),
                entry.value.as_encoded_bytes().len()
            ),
        );
    }
    line(
        &mut output,
        "configuration-identity",
        content_identity(&plan.configuration_digest, plan.configuration.len() as u64),
    );
    for (index, argument) in plan.collector_arguments.iter().enumerate() {
        if index < 10 {
            line_debug(&mut output, &format!("collector-arg[{index}]"), argument);
        } else if index == 10 {
            line(
                &mut output,
                &format!("collector-arg[{index}]"),
                "retained-target-descriptor",
            );
        } else {
            redacted_value(
                &mut output,
                &format!("collector-arg[{index}]"),
                argument.as_bytes(),
            );
        }
    }
    if plan.devices.is_empty() {
        line(&mut output, "device-identity-origin", "unavailable");
        line(
            &mut output,
            "device-identity-reason",
            "no-visible-direct-kfd-gpu-topology",
        );
    }
    for (index, device) in plan.devices.iter().enumerate() {
        line(
            &mut output,
            &format!("device[{index}]"),
            format!(
                "node={};identity={}",
                device.node,
                device.content_identity()
            ),
        );
    }
    render_target_profile_observations(&mut output, &plan.devices);
    render_expected(&mut output, plan.options.kind);
    render_import_plan(&mut output, plan);
    line(
        &mut output,
        "collection-authorization",
        hex(&plan.authorization),
    );
    line(
        &mut output,
        "collect-next",
        format!(
            "rerun-with---collect---authorize-collection={}",
            hex(&plan.authorization)
        ),
    );
    output.pop();
    output
}

fn identity_lines(output: &mut String, prefix: &str, pin: &FilePin) {
    line_debug(output, &format!("{prefix}-path"), &pin.canonical_path);
    line(
        output,
        &format!("{prefix}-identity"),
        pin.content_identity(),
    );
    line(
        output,
        &format!("{prefix}-object"),
        format!(
            "dev={};ino={};mode={:o}",
            pin.identity.device, pin.identity.inode, pin.identity.mode
        ),
    );
}

fn render_target_profile_observations(output: &mut String, devices: &[DeviceIdentity]) {
    for (index, device) in devices.iter().enumerate() {
        line(
            output,
            &format!("observed-gpu-target-profile-v1[{index}]"),
            device.target_profile_record(),
        );
    }
}

fn render_expected(output: &mut String, kind: ProfileKind) {
    match kind {
        ProfileKind::DispatchJson => line(
            output,
            "expected-artifact[0]",
            "rocprofv3-json:*.json:required-for-import",
        ),
        ProfileKind::DispatchCsv => line(
            output,
            "expected-artifact[0]",
            "rocprofv3-kernel-csv:*.csv:required-for-import",
        ),
        ProfileKind::Att => {
            line(
                output,
                "expected-artifact[0]",
                "rocprofv3-att-manifest:*.json:required-for-import",
            );
            line(
                output,
                "expected-artifact[1]",
                "rocprofv3-att-reference:manifest-relative-files:required-for-import",
            );
        }
    }
    line(
        output,
        "expected-artifact-truth",
        "filename-class-only-not-observation-or-validity",
    );
}

fn render_import_plan(output: &mut String, plan: &Plan) {
    if plan.options.kind == ProfileKind::Att {
        line(
            output,
            "next-import-status",
            "unavailable-att-decoder-has-no-mutation-proof-sealed-directory-route",
        );
        line(
            output,
            "next-query-status",
            "unavailable-until-sealed-att-collection-and-bundle-v4-import",
        );
        return;
    }
    if !render_dispatch_import_plan(
        output,
        plan.verified_kir_v7.as_ref(),
        plan.options.kir_binding.as_ref(),
        plan.devices.is_empty(),
    ) {
        return;
    }
    line(output, "next-query-program", "fe2o3-profiler-query");
    line(output, "next-query-arg[0]", "capabilities");
    line(output, "next-query-stdin", "imported-fe2o3prof4-bundle");
}

fn render_dispatch_import_plan(
    output: &mut String,
    kir: Option<&VerifiedKirInputV1>,
    legacy_kir: Option<&KirBinding>,
    devices_empty: bool,
) -> bool {
    let Some(kir) = kir else {
        line(
            output,
            "next-import-status",
            if legacy_kir.is_some() {
                "unavailable-legacy-kir-declaration-is-not-admitted-canonical-kir"
            } else {
                "unavailable-missing-exact-canonical-kir-v7"
            },
        );
        line(
            output,
            "next-query-status",
            "unavailable-until-bundle-v4-import",
        );
        return false;
    };
    if devices_empty {
        line(
            output,
            "next-import-status",
            "unavailable-no-stable-direct-kfd-device-identity",
        );
        line(
            output,
            "next-query-status",
            "unavailable-until-bundle-v4-import",
        );
        return false;
    }
    if let KirTargetCompatibilityV1::Unavailable(reason) = kir.compatibility {
        line(
            output,
            "next-import-status",
            "unavailable-kir-v7-target-compatibility",
        );
        line(output, "next-import-unavailable-reason", reason.name());
        line(
            output,
            "next-query-status",
            "unavailable-until-bundle-v4-import",
        );
        return false;
    }

    line(output, "next-import-program", "cargo-fe2o3-in-process");
    line(
        output,
        "next-import-status",
        "ready-after-collector-artifact-and-source-size-validation",
    );
    line(
        output,
        "next-import-source-byte-limit",
        MAX_PROFILER_IMPORT_SOURCE_BYTES,
    );
    line(
        output,
        "next-import-kir-v7-policy-identity",
        format!(
            "sha256:{}:{}",
            hex(kir.owner.identity().digest()),
            kir.owner.identity().canonical_length()
        ),
    );
    line(
        output,
        "next-import-stdin",
        "validated-collected-json-or-csv-artifact",
    );
    line(
        output,
        "next-import-artifact-identity-origin",
        "unavailable",
    );
    line(
        output,
        "next-import-artifact-identity-reason",
        "profile-target-is-not-proof-of-executed-kernel-code-object",
    );
    line(
        output,
        "next-comparison-limitation",
        "duration-deltas-require-a-separately-content-bound-kernel-artifact",
    );
    true
}

fn collect(plan: Plan) -> Result<CommandReport, String> {
    collect_with_device_revalidator(plan, validate_device_bindings)
}

fn collect_with_device_revalidator<F>(
    plan: Plan,
    device_revalidator: F,
) -> Result<CommandReport, String>
where
    F: Fn(&[DeviceIdentity]) -> Result<(), String>,
{
    plan.tool.validate("rocprofv3 script")?;
    plan.interpreter.validate("rocprofv3 Python interpreter")?;
    plan.collector_execution.validate()?;
    if let Some(libraries) = &plan.collector_libraries {
        libraries.validate()?;
    }
    plan.target.validate("profile target")?;
    if let Some(kir) = &plan.verified_kir_v7 {
        kir.revalidate()?;
    }
    device_revalidator(&plan.devices)?;
    let custody = OutputCustody::create(&plan.output_directory, &plan.authorization)?;
    let result = run_collector(&plan);
    let supervised = match result {
        Ok(result) => result,
        Err(error) => {
            custody.cleanup()?;
            return Err(error);
        }
    };
    let identity_error = plan
        .tool
        .validate("rocprofv3 script")
        .and_then(|()| plan.interpreter.validate("rocprofv3 Python interpreter"))
        .and_then(|()| match &plan.collector_libraries {
            Some(libraries) => libraries.validate(),
            None => Ok(()),
        })
        .and_then(|()| plan.target.validate("profile target"))
        .and_then(|()| match &plan.verified_kir_v7 {
            Some(kir) => kir.revalidate(),
            None => Ok(()),
        })
        .and_then(|()| device_revalidator(&plan.devices))
        .err();
    if supervised.reason != StopReason::Exited
        || !supervised.status.is_some_and(|status| status.success())
        || identity_error.is_some()
    {
        let cleanup_error = custody.cleanup().err();
        return Ok(render_failed_collection(
            &plan,
            supervised,
            identity_error,
            cleanup_error,
        ));
    }
    let artifacts = match scan_artifacts(&custody, plan.options.storage_limit) {
        Ok(artifacts) => artifacts,
        Err(error) => {
            custody.cleanup()?;
            return Err(format!("collector output rejected and cleaned: {error}"));
        }
    };
    let dispatch_import = match select_dispatch_import_v1(&plan, &custody, &artifacts) {
        Ok(outcome) => outcome,
        Err(error) => {
            custody.cleanup()?;
            return Err(format!(
                "dispatch import custody failed and was cleaned: {error}"
            ));
        }
    };
    let manifest = render_manifest(&plan, &artifacts, &dispatch_import);
    let artifact_bytes = artifacts
        .iter()
        .try_fold(0_u64, |total, artifact| total.checked_add(artifact.length));
    let generated_bytes = match &dispatch_import {
        DispatchImportOutcomeV1::Unavailable(_) => Some(0_u64),
        DispatchImportOutcomeV1::Imported { product, .. } => product
            .bundle_bytes
            .len()
            .checked_add(product.receipt_bytes.len())
            .and_then(|length| u64::try_from(length).ok()),
    };
    let total_publication_bytes = artifact_bytes
        .and_then(|total| total.checked_add(generated_bytes?))
        .and_then(|total| total.checked_add(u64::try_from(manifest.len()).ok()?));
    if total_publication_bytes.is_none_or(|total| total > plan.options.storage_limit) {
        custody.cleanup()?;
        return Err(
            "collector and generated transaction would exceed the storage limit; output was cleaned"
                .to_owned(),
        );
    }
    if let DispatchImportOutcomeV1::Imported {
        source,
        source_kind,
        binding,
        product,
    } = &dispatch_import
    {
        let persistence = (|| {
            source.revalidate(&custody)?;
            revalidate_collection_inputs_v1(&plan, &device_revalidator)?;
            readmit_dispatch_import_tuple_v1(
                *source_kind,
                &source.bytes,
                binding.as_ref().clone(),
                &product.bundle_bytes,
                &product.capture_bytes,
                &product.receipt_bytes,
            )
            .map_err(|error| format!("pre-publication tuple readmission failed: {error}"))?;
            custody.commit_record(
                PROFILE_DISPATCH_BUNDLE_FILE_V1,
                PROFILE_DISPATCH_BUNDLE_REDO_FILE_V1,
                &product.bundle_bytes,
                product.bundle_bytes.len(),
            )?;
            source.revalidate(&custody)?;
            revalidate_collection_inputs_v1(&plan, &device_revalidator)?;
            custody.commit_record(
                PROFILE_DISPATCH_RECEIPT_FILE_V1,
                PROFILE_DISPATCH_RECEIPT_REDO_FILE_V1,
                &product.receipt_bytes,
                product.receipt_bytes.len(),
            )?;
            let durable_bundle =
                custody.read_record(PROFILE_DISPATCH_BUNDLE_FILE_V1, product.bundle_bytes.len())?;
            let durable_receipt = custody.read_record(
                PROFILE_DISPATCH_RECEIPT_FILE_V1,
                product.receipt_bytes.len(),
            )?;
            readmit_dispatch_import_tuple_v1(
                *source_kind,
                &source.bytes,
                binding.as_ref().clone(),
                &durable_bundle,
                &product.capture_bytes,
                &durable_receipt,
            )
            .map_err(|error| format!("durable tuple readmission failed: {error}"))?;
            source.revalidate(&custody)?;
            revalidate_collection_inputs_v1(&plan, &device_revalidator)
        })();
        if let Err(error) = persistence {
            let cleanup = custody.cleanup().err();
            return Err(match cleanup {
                Some(cleanup) => format!(
                    "dispatch import publication failed: {error}; cleanup also failed: {cleanup}"
                ),
                None => format!("dispatch import publication failed and was cleaned: {error}"),
            });
        }
    }
    let final_revalidation = (|| {
        if let DispatchImportOutcomeV1::Imported { source, .. } = &dispatch_import {
            source.revalidate(&custody)?;
        }
        revalidate_collection_inputs_v1(&plan, &device_revalidator)
    })();
    if let Err(error) = final_revalidation {
        let cleanup = custody.cleanup().err();
        return Err(match cleanup {
            Some(cleanup) => {
                format!(
                    "manifest-last revalidation failed: {error}; cleanup also failed: {cleanup}"
                )
            }
            None => format!("manifest-last revalidation failed and was cleaned: {error}"),
        });
    }
    if let Err(error) = custody.write_manifest(manifest.as_bytes()) {
        let cleanup = custody.cleanup().err();
        return Err(match cleanup {
            Some(cleanup) => {
                format!("{error}; additionally failed to clean owned output: {cleanup}")
            }
            None => format!("{error}; owned output was cleaned"),
        });
    }
    Ok(render_successful_collection(
        &plan,
        supervised,
        &artifacts,
        &dispatch_import,
    ))
}

fn revalidate_collection_inputs_v1<F>(plan: &Plan, device_revalidator: &F) -> Result<(), String>
where
    F: Fn(&[DeviceIdentity]) -> Result<(), String>,
{
    plan.tool.validate("rocprofv3 script")?;
    plan.interpreter.validate("rocprofv3 Python interpreter")?;
    plan.collector_execution.validate()?;
    if let Some(libraries) = &plan.collector_libraries {
        libraries.validate()?;
    }
    plan.target.validate("profile target")?;
    if let Some(kir) = &plan.verified_kir_v7 {
        kir.revalidate()?;
    }
    device_revalidator(&plan.devices)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum StopReason {
    Exited,
    Timeout,
    OutputOverflow,
    WaitFailure,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CollectorLeaderExitObservation {
    Running,
    Exited,
}

enum CollectorExitDecision {
    RevokeAndReap(StopReason),
    RevokeAndReapAfterWaitFailure(String),
    AmbiguousWait(String),
}

// SIGCHLD disposition is process-global. cargo-fe2o3 profile exclusively owns this direct child;
// no unrelated thread may wait on it or change SIGCHLD while this scope is active, and the capture
// readers do neither. This mutex serializes cooperating in-process profile calls through spawn,
// revoke, and reap; it cannot serialize arbitrary signal or wait code outside this module.
static COLLECTOR_SIGCHLD_SCOPE_LOCK_V1: OnceLock<Mutex<()>> = OnceLock::new();

struct CollectorSigchldScopeV1 {
    _lock: MutexGuard<'static, ()>,
    previous: libc::sigaction,
    restored: bool,
}

impl CollectorSigchldScopeV1 {
    fn enter() -> Result<Self, String> {
        let lock = COLLECTOR_SIGCHLD_SCOPE_LOCK_V1
            .get_or_init(|| Mutex::new(()))
            .lock()
            .map_err(|_| "collector SIGCHLD scope lock was poisoned".to_owned())?;
        let mut owned = MaybeUninit::<libc::sigaction>::zeroed();
        // SAFETY: a zeroed sigaction is initialized below before it is installed.
        let owned = unsafe {
            let pointer = owned.as_mut_ptr();
            (*pointer).sa_sigaction = libc::SIG_DFL;
            (*pointer).sa_flags = 0;
            if libc::sigemptyset(&mut (*pointer).sa_mask) != 0 {
                return Err(format!(
                    "failed to initialize collector SIGCHLD mask: {}",
                    io::Error::last_os_error()
                ));
            }
            owned.assume_init()
        };
        let mut previous = MaybeUninit::<libc::sigaction>::zeroed();
        // SAFETY: `owned` is fully initialized and `previous` is writable. Passing both makes the
        // disposition replacement and prior-action capture one kernel operation.
        if unsafe { libc::sigaction(libc::SIGCHLD, &owned, previous.as_mut_ptr()) } != 0 {
            return Err(format!(
                "failed to establish collector SIGCHLD ownership: {}",
                io::Error::last_os_error()
            ));
        }
        Ok(Self {
            _lock: lock,
            // SAFETY: the successful query initialized `previous`.
            previous: unsafe { previous.assume_init() },
            restored: false,
        })
    }

    fn validate_owned(&self) -> Result<(), String> {
        let mut current = MaybeUninit::<libc::sigaction>::zeroed();
        // SAFETY: `current` is writable and a null action requests the current disposition.
        if unsafe { libc::sigaction(libc::SIGCHLD, std::ptr::null(), current.as_mut_ptr()) } != 0 {
            return Err(format!(
                "failed to revalidate collector SIGCHLD ownership: {}",
                io::Error::last_os_error()
            ));
        }
        // SAFETY: the successful query initialized `current`.
        let current = unsafe { current.assume_init() };
        if current.sa_sigaction != libc::SIG_DFL || current.sa_flags & libc::SA_NOCLDWAIT != 0 {
            return Err(
                "collector SIGCHLD ownership changed before process-group revoke".to_owned(),
            );
        }
        Ok(())
    }

    fn restore(&mut self) -> Result<(), String> {
        if self.restored {
            return Ok(());
        }
        // SAFETY: the saved action was initialized by sigaction and remains live for the call.
        if unsafe { libc::sigaction(libc::SIGCHLD, &self.previous, std::ptr::null_mut()) } != 0 {
            return Err(format!(
                "failed to restore SIGCHLD disposition: {}",
                io::Error::last_os_error()
            ));
        }
        self.restored = true;
        Ok(())
    }
}

impl Drop for CollectorSigchldScopeV1 {
    fn drop(&mut self) {
        let _ = self.restore();
    }
}

struct BoundedCapture {
    bytes: Vec<u8>,
    overflow: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CaptureStreamV1 {
    Stdout,
    Stderr,
}

impl CaptureStreamV1 {
    const fn name(self) -> &'static str {
        match self {
            Self::Stdout => "stdout",
            Self::Stderr => "stderr",
        }
    }
}

#[cfg(test)]
#[derive(Clone)]
struct CaptureThreadSpawnFailureInjectionV1 {
    stream: CaptureStreamV1,
    collector_ready: PathBuf,
    active_workers: Arc<std::sync::atomic::AtomicUsize>,
}

#[cfg(test)]
thread_local! {
    static CAPTURE_THREAD_SPAWN_FAILURE_INJECTION_V1: std::cell::RefCell<Option<CaptureThreadSpawnFailureInjectionV1>> = const { std::cell::RefCell::new(None) };
}

#[cfg(test)]
struct CaptureThreadWorkerGuardV1 {
    active_workers: Arc<std::sync::atomic::AtomicUsize>,
}

#[cfg(test)]
impl CaptureThreadWorkerGuardV1 {
    fn start(active_workers: Arc<std::sync::atomic::AtomicUsize>) -> Self {
        active_workers.fetch_add(1, Ordering::SeqCst);
        Self { active_workers }
    }
}

#[cfg(test)]
impl Drop for CaptureThreadWorkerGuardV1 {
    fn drop(&mut self) {
        self.active_workers.fetch_sub(1, Ordering::SeqCst);
    }
}

struct Supervised {
    status: Option<ExitStatus>,
    reason: StopReason,
    stdout: BoundedCapture,
    stderr: BoundedCapture,
    wait_error: Option<String>,
}

fn run_collector(plan: &Plan) -> Result<Supervised, String> {
    plan.tool.validate("rocprofv3 script")?;
    plan.interpreter.validate("rocprofv3 Python interpreter")?;
    plan.target.validate("profile target")?;
    plan.collector_execution.validate()?;
    if let Some(libraries) = &plan.collector_libraries {
        libraries.validate()?;
    }
    let mut command = Command::new(plan.interpreter.execution_path());
    command
        .arg0(&plan.interpreter.canonical_path)
        .arg(plan.collector_execution.bootstrap_path())
        .args(&plan.collector_arguments)
        .current_dir(&plan.working_directory)
        .env_clear()
        .envs(
            plan.environment
                .iter()
                .map(|entry| (entry.name, &entry.value)),
        )
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .process_group(0);
    command
        .env(
            SEALED_SCRIPT_ENV_V1,
            plan.collector_execution.source_path(&plan.tool),
        )
        .env(LOGICAL_SCRIPT_ENV_V1, &plan.tool.canonical_path);
    if let (CollectorExecutionV1::InstalledAdapter { .. }, Some(libraries)) =
        (&plan.collector_execution, &plan.collector_libraries)
    {
        let logical_root = plan
            .tool
            .canonical_path
            .parent()
            .and_then(Path::parent)
            .ok_or("installed rocprofv3 script has no logical ROCm root")?;
        command
            .env(LOGICAL_ROCM_ROOT_ENV_V1, logical_root)
            .env(SEALED_CORE_ENV_V1, libraries.core.external_path())
            .env(SEALED_TOOL_ENV_V1, libraries.tool.external_path());
    }
    spawn_and_supervise_collector_v1(
        &mut command,
        plan.options.timeout,
        plan.options.stdout_limit,
        plan.options.stderr_limit,
    )
}

fn spawn_and_supervise_collector_v1(
    command: &mut Command,
    timeout: Duration,
    stdout_limit: usize,
    stderr_limit: usize,
) -> Result<Supervised, String> {
    let mut sigchld = CollectorSigchldScopeV1::enter()?;
    let result = (|| {
        let mut child = crate::process_execution::spawn(command)
            .map_err(|error| format!("failed to spawn pinned rocprofv3 collector: {error}"))?;
        supervise_with_sigchld_scope(
            &mut child,
            timeout,
            stdout_limit,
            stderr_limit,
            Some(&sigchld),
        )
    })();
    let restoration = sigchld.restore();
    match (result, restoration) {
        (Ok(supervised), Ok(())) => Ok(supervised),
        (Err(error), Ok(())) => Err(error),
        (Ok(_), Err(error)) => Err(error),
        (Err(error), Err(restoration)) => Err(format!(
            "{error}; collector SIGCHLD restoration also failed: {restoration}"
        )),
    }
}

#[cfg(test)]
fn supervise(
    child: &mut Child,
    timeout: Duration,
    stdout_limit: usize,
    stderr_limit: usize,
) -> Result<Supervised, String> {
    supervise_with_sigchld_scope(child, timeout, stdout_limit, stderr_limit, None)
}

fn supervise_with_sigchld_scope(
    child: &mut Child,
    timeout: Duration,
    stdout_limit: usize,
    stderr_limit: usize,
    sigchld: Option<&CollectorSigchldScopeV1>,
) -> Result<Supervised, String> {
    let (stdout, stderr) = match (child.stdout.take(), child.stderr.take()) {
        (Some(stdout), Some(stderr)) => (stdout, stderr),
        (stdout, stderr) => {
            drop(stdout);
            drop(stderr);
            let ownership = sigchld.map_or(Ok(()), CollectorSigchldScopeV1::validate_owned);
            return Err(finalize_capture_setup_failure_with(
                child,
                "collector stdout or stderr pipe was unavailable".to_owned(),
                ownership,
                revoke_owned_child,
                Child::wait,
            ));
        }
    };
    if let Err(error) =
        make_capture_pipe_nonblocking(&stdout).and_then(|()| make_capture_pipe_nonblocking(&stderr))
    {
        let ownership = sigchld.map_or(Ok(()), CollectorSigchldScopeV1::validate_owned);
        return Err(finalize_capture_setup_failure_with(
            child,
            format!("failed to configure bounded collector output capture: {error}"),
            ownership,
            revoke_owned_child,
            Child::wait,
        ));
    }
    let overflow = Arc::new(AtomicBool::new(false));
    let capture_cancelled = Arc::new(AtomicBool::new(false));
    let stdout_thread = match capture_thread(
        CaptureStreamV1::Stdout,
        stdout,
        stdout_limit,
        Arc::clone(&overflow),
        Arc::clone(&capture_cancelled),
    ) {
        Ok(worker) => worker,
        Err(error) => {
            drop(stderr);
            return Err(finalize_capture_thread_spawn_failure_v1(
                child,
                CaptureStreamV1::Stdout,
                error,
                &capture_cancelled,
                None,
                sigchld,
            ));
        }
    };
    let stderr_thread = match capture_thread(
        CaptureStreamV1::Stderr,
        stderr,
        stderr_limit,
        Arc::clone(&overflow),
        Arc::clone(&capture_cancelled),
    ) {
        Ok(worker) => worker,
        Err(error) => {
            return Err(finalize_capture_thread_spawn_failure_v1(
                child,
                CaptureStreamV1::Stderr,
                error,
                &capture_cancelled,
                Some(stdout_thread),
                sigchld,
            ));
        }
    };
    let started = Instant::now();
    let decision = loop {
        if overflow.load(Ordering::Acquire) {
            break CollectorExitDecision::RevokeAndReap(StopReason::OutputOverflow);
        }
        match observe_collector_leader_exit_without_reaping(child) {
            Ok(CollectorLeaderExitObservation::Exited) => {
                break CollectorExitDecision::RevokeAndReap(StopReason::Exited);
            }
            Ok(CollectorLeaderExitObservation::Running) if started.elapsed() >= timeout => {
                break CollectorExitDecision::RevokeAndReap(StopReason::Timeout);
            }
            Ok(CollectorLeaderExitObservation::Running) => thread::sleep(POLL_INTERVAL),
            Err(error) => {
                let message = format!(
                    "failed to inspect collector leader without reaping it: {error} (errno {:?})",
                    error.raw_os_error()
                );
                break if error.raw_os_error() == Some(libc::ECHILD) {
                    CollectorExitDecision::AmbiguousWait(message)
                } else {
                    CollectorExitDecision::RevokeAndReapAfterWaitFailure(message)
                };
            }
        }
    };
    let decision = if matches!(
        &decision,
        CollectorExitDecision::RevokeAndReap(_)
            | CollectorExitDecision::RevokeAndReapAfterWaitFailure(_)
    ) {
        if let Some(sigchld) = sigchld {
            if let Err(error) = sigchld.validate_owned() {
                CollectorExitDecision::AmbiguousWait(error)
            } else {
                decision
            }
        } else {
            decision
        }
    } else {
        decision
    };
    let (status, reason, wait_error) =
        finalize_collector_exit_with(child, decision, revoke_owned_child, Child::wait);
    capture_cancelled.store(true, Ordering::Release);
    let stdout = stdout_thread
        .join()
        .map_err(|_| "collector stdout capture thread panicked")?;
    let stderr = stderr_thread
        .join()
        .map_err(|_| "collector stderr capture thread panicked")?;
    let reason = if stdout.overflow || stderr.overflow {
        StopReason::OutputOverflow
    } else {
        reason
    };
    Ok(Supervised {
        status,
        reason,
        stdout,
        stderr,
        wait_error,
    })
}

fn observe_collector_leader_exit_without_reaping(
    child: &Child,
) -> io::Result<CollectorLeaderExitObservation> {
    let leader = libc::pid_t::try_from(child.id()).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "collector leader PID does not fit pid_t",
        )
    })?;
    loop {
        let mut information = MaybeUninit::<libc::siginfo_t>::zeroed();
        // SAFETY: `information` is writable, P_PID selects the owned collector leader, WNOHANG
        // bounds the observation, and WNOWAIT retains an exited leader until its dedicated
        // process group has been revoked.
        let result = unsafe {
            libc::waitid(
                libc::P_PID,
                leader as libc::id_t,
                information.as_mut_ptr(),
                libc::WEXITED | libc::WNOHANG | libc::WNOWAIT,
            )
        };
        if result == 0 {
            // SAFETY: successful waitid initialized the siginfo record.
            let observed = unsafe { information.assume_init().si_pid() };
            return match observed {
                0 => Ok(CollectorLeaderExitObservation::Running),
                observed if observed == leader => Ok(CollectorLeaderExitObservation::Exited),
                _ => Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "waitid returned an unexpected collector child",
                )),
            };
        }
        let error = io::Error::last_os_error();
        if error.kind() != io::ErrorKind::Interrupted {
            return Err(error);
        }
    }
}

fn revoke_then_reap_with<T, S>(
    subject: &mut T,
    reason: StopReason,
    revoke: impl FnOnce(&mut T),
    reap: impl FnOnce(&mut T) -> io::Result<S>,
) -> (Option<S>, StopReason, Option<String>) {
    revoke(subject);
    match reap(subject) {
        Ok(status) => (Some(status), reason, None),
        Err(error) => (
            None,
            StopReason::WaitFailure,
            Some(format!("failed to reap collector leader: {error}")),
        ),
    }
}

fn finalize_capture_setup_failure_with<T, S>(
    subject: &mut T,
    setup_error: String,
    ownership: Result<(), String>,
    revoke: impl FnOnce(&mut T),
    reap: impl FnOnce(&mut T) -> io::Result<S>,
) -> String {
    let Err(ownership_error) = ownership else {
        let (_, _, wait_error) =
            revoke_then_reap_with(subject, StopReason::WaitFailure, revoke, reap);
        return match wait_error {
            Some(wait_error) => format!("{setup_error}; {wait_error}"),
            None => setup_error,
        };
    };
    format!(
        "{setup_error}; {ownership_error}; collector numeric identity was not signaled after ownership became ambiguous"
    )
}

fn finalize_capture_thread_spawn_failure_v1(
    child: &mut Child,
    stream: CaptureStreamV1,
    spawn_error: io::Error,
    capture_cancelled: &AtomicBool,
    started_worker: Option<thread::JoinHandle<BoundedCapture>>,
    sigchld: Option<&CollectorSigchldScopeV1>,
) -> String {
    let setup_error = format!(
        "failed to spawn collector {} capture worker: {spawn_error}",
        stream.name()
    );
    let ownership = sigchld.map_or(Ok(()), CollectorSigchldScopeV1::validate_owned);
    let mut error = finalize_capture_setup_failure_with(
        child,
        setup_error,
        ownership,
        revoke_owned_child,
        Child::wait,
    );
    capture_cancelled.store(true, Ordering::Release);
    if started_worker.is_some_and(|worker| worker.join().is_err()) {
        error.push_str("; collector stdout capture thread panicked during setup rollback");
    }
    error
}

fn finalize_collector_exit_with<T, S>(
    subject: &mut T,
    decision: CollectorExitDecision,
    revoke: impl FnOnce(&mut T),
    reap: impl FnOnce(&mut T) -> io::Result<S>,
) -> (Option<S>, StopReason, Option<String>) {
    match decision {
        CollectorExitDecision::RevokeAndReap(reason) => {
            revoke_then_reap_with(subject, reason, revoke, reap)
        }
        CollectorExitDecision::RevokeAndReapAfterWaitFailure(error) => {
            let (status, _, reap_error) =
                revoke_then_reap_with(subject, StopReason::WaitFailure, revoke, reap);
            let wait_error = match reap_error {
                Some(reap_error) => Some(format!("{error}; {reap_error}")),
                None => Some(error),
            };
            (status, StopReason::WaitFailure, wait_error)
        }
        CollectorExitDecision::AmbiguousWait(error) => (None, StopReason::WaitFailure, Some(error)),
    }
}

fn capture_thread(
    stream: CaptureStreamV1,
    mut pipe: impl Read + Send + 'static,
    limit: usize,
    global_overflow: Arc<AtomicBool>,
    cancelled: Arc<AtomicBool>,
) -> io::Result<thread::JoinHandle<BoundedCapture>> {
    #[cfg(test)]
    let active_workers = capture_thread_spawn_test_control_v1(stream)?;
    thread::Builder::new()
        .name(format!("fe2o3-profile-{}-capture-v1", stream.name()))
        .spawn(move || {
            #[cfg(test)]
            let _worker_guard = active_workers.map(CaptureThreadWorkerGuardV1::start);
            let mut bytes = Vec::new();
            let mut overflow = false;
            let mut buffer = [0_u8; 8192];
            let mut drain_deadline = None;
            loop {
                if cancelled.load(Ordering::Acquire) && drain_deadline.is_none() {
                    drain_deadline = Instant::now().checked_add(CAPTURE_DRAIN_TIMEOUT);
                }
                if drain_deadline.is_some_and(|deadline| Instant::now() >= deadline) {
                    break;
                }
                match pipe.read(&mut buffer) {
                    Ok(0) => break,
                    Ok(read) => {
                        let remaining = limit.saturating_sub(bytes.len());
                        bytes.extend_from_slice(&buffer[..read.min(remaining)]);
                        if read > remaining {
                            overflow = true;
                            global_overflow.store(true, Ordering::Release);
                            break;
                        }
                        if cancelled.load(Ordering::Acquire) && drain_deadline.is_none() {
                            drain_deadline = Instant::now().checked_add(CAPTURE_DRAIN_TIMEOUT);
                        }
                    }
                    Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                    Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                        if cancelled.load(Ordering::Acquire) {
                            break;
                        }
                        thread::sleep(POLL_INTERVAL);
                    }
                    Err(_) => {
                        overflow = true;
                        global_overflow.store(true, Ordering::Release);
                        break;
                    }
                }
            }
            BoundedCapture { bytes, overflow }
        })
}

#[cfg(test)]
fn capture_thread_spawn_test_control_v1(
    stream: CaptureStreamV1,
) -> io::Result<Option<Arc<std::sync::atomic::AtomicUsize>>> {
    let injection =
        CAPTURE_THREAD_SPAWN_FAILURE_INJECTION_V1.with(|current| current.borrow().clone());
    let Some(injection) = injection else {
        return Ok(None);
    };
    if injection.stream == stream {
        for _ in 0..800 {
            if injection.collector_ready.is_file() {
                return Err(io::Error::other(format!(
                    "injected {} capture worker creation failure",
                    stream.name()
                )));
            }
            thread::sleep(Duration::from_millis(5));
        }
        return Err(io::Error::new(
            io::ErrorKind::TimedOut,
            format!(
                "collector did not reach the injected {} capture worker failure",
                stream.name()
            ),
        ));
    }
    Ok(Some(injection.active_workers))
}

fn make_capture_pipe_nonblocking(pipe: &impl rustix::fd::AsFd) -> Result<(), String> {
    let flags = rustix::fs::fcntl_getfl(pipe)
        .map_err(|error| format!("failed to inspect collector capture pipe: {error}"))?;
    rustix::fs::fcntl_setfl(pipe, flags | rustix::fs::OFlags::NONBLOCK)
        .map_err(|error| format!("failed to make collector capture pipe nonblocking: {error}"))
}

fn revoke_owned_child(child: &mut Child) {
    let Ok(pid) = i32::try_from(child.id()) else {
        return;
    };
    // SAFETY: this is called only while the owned leader is live or waitable-but-unreaped, so a
    // negative, positive process-group id still identifies the fresh child group created above.
    let _ = unsafe { libc::kill(-pid, libc::SIGKILL) };
    let _ = child.kill();
}

struct OutputCustody {
    path: PathBuf,
    identity: ObjectIdentity,
    guard: Vec<u8>,
    guard_file: File,
    guard_identity: ObjectIdentity,
    root: rustix::fd::OwnedFd,
    durable: RetainedDurableDirectoryV1,
}

impl OutputCustody {
    fn create(path: &Path, authorization: &[u8; 32]) -> Result<Self, String> {
        fs::create_dir(path)
            .map_err(|error| format!("failed to create private output directory: {error}"))?;
        if let Err(error) = fs::set_permissions(path, fs::Permissions::from_mode(0o700)) {
            let _ = fs::remove_dir(path);
            return Err(format!("failed to make output directory private: {error}"));
        }
        let metadata = match fs::symlink_metadata(path) {
            Ok(metadata) => metadata,
            Err(error) => {
                let _ = fs::remove_dir(path);
                return Err(format!("failed to inspect output directory: {error}"));
            }
        };
        if !metadata.is_dir()
            || metadata.file_type().is_symlink()
            || metadata.permissions().mode() & 0o777 != 0o700
        {
            let _ = fs::remove_dir(path);
            return Err("new output directory failed private-custody validation".to_owned());
        }
        let guard = format!("fe2o3-profile-owned-v1\n{}\n", hex(authorization)).into_bytes();
        let guard_path = path.join(OWNERSHIP_FILE);
        let mut file = match OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&guard_path)
        {
            Ok(file) => file,
            Err(error) => {
                let _ = fs::remove_dir_all(path);
                return Err(format!("failed to create output ownership guard: {error}"));
            }
        };
        if let Err(error) = file.write_all(&guard).and_then(|()| file.sync_all()) {
            drop(file);
            let _ = fs::remove_dir_all(path);
            return Err(format!("failed to persist output ownership guard: {error}"));
        }
        let guard_metadata = match file.metadata() {
            Ok(metadata) => metadata,
            Err(error) => {
                drop(file);
                let _ = fs::remove_dir_all(path);
                return Err(format!("failed to inspect output ownership guard: {error}"));
            }
        };
        if !is_private_regular_file(&guard_metadata) {
            drop(file);
            let _ = fs::remove_dir_all(path);
            return Err("output ownership guard is not a private regular file".to_owned());
        }
        let guard_identity = ObjectIdentity::from_metadata(&guard_metadata);
        let admitted = (|| {
            let root = rustix::fs::open(
                path,
                rustix::fs::OFlags::RDONLY
                    | rustix::fs::OFlags::DIRECTORY
                    | rustix::fs::OFlags::NOFOLLOW
                    | rustix::fs::OFlags::CLOEXEC,
                rustix::fs::Mode::empty(),
            )
            .map_err(|error| format!("failed to retain output directory: {error}"))?;
            rustix::fs::fsync(&root)
                .map_err(|error| format!("failed to persist output directory custody: {error}"))?;
            let durable_root = rustix::io::fcntl_dupfd_cloexec(&root, 0).map_err(|error| {
                format!("failed to duplicate output directory custody: {error}")
            })?;
            let durable = RetainedDurableDirectoryV1::admit_service_owned(durable_root)
                .map_err(|error| format!("failed to admit durable output custody: {error}"))?;
            Ok::<_, String>((root, durable))
        })();
        let (root, durable) = match admitted {
            Ok(admitted) => admitted,
            Err(error) => {
                drop(file);
                let _ = fs::remove_dir_all(path);
                return Err(error);
            }
        };
        Ok(Self {
            path: path.to_path_buf(),
            identity: ObjectIdentity::from_metadata(&metadata),
            guard,
            guard_file: file,
            guard_identity,
            root,
            durable,
        })
    }

    fn validate(&self) -> Result<(), String> {
        let metadata = fs::symlink_metadata(&self.path)
            .map_err(|error| format!("output custody changed: {error}"))?;
        let current = ObjectIdentity::from_metadata(&metadata);
        if metadata.file_type().is_symlink()
            || !metadata.is_dir()
            || current.device != self.identity.device
            || current.inode != self.identity.inode
            || current.mode != self.identity.mode
        {
            return Err("output custody object was substituted".to_owned());
        }
        let retained_guard_metadata = self
            .guard_file
            .metadata()
            .map_err(|error| format!("output ownership guard unavailable: {error}"))?;
        if !is_private_regular_file(&retained_guard_metadata)
            || ObjectIdentity::from_metadata(&retained_guard_metadata) != self.guard_identity
        {
            return Err("retained output ownership guard changed".to_owned());
        }
        let (guard_file, guard_identity) = open_retained_leaf(
            &self.root,
            OWNERSHIP_FILE,
            Some(self.guard_identity),
            true,
            "output ownership guard",
        )?;
        let guard = read_bounded_leaf(
            guard_file,
            guard_identity,
            self.guard.len(),
            "output ownership guard",
        )?;
        if guard != self.guard {
            return Err("output ownership guard changed".to_owned());
        }
        Ok(())
    }

    fn cleanup(&self) -> Result<(), String> {
        self.validate()?;
        fs::remove_dir_all(&self.path)
            .map_err(|error| format!("failed to clean owned output directory: {error}"))
    }

    fn write_manifest(&self, bytes: &[u8]) -> Result<(), String> {
        self.validate()?;
        self.commit_record(MANIFEST_FILE, MANIFEST_REDO_FILE, bytes, bytes.len())
    }

    fn commit_record(
        &self,
        canonical: &str,
        redo: &str,
        bytes: &[u8],
        maximum: usize,
    ) -> Result<(), String> {
        self.validate()?;
        let mut hooks = NoRetainedDurableDirectoryHooksV1;
        self.durable
            .commit_record(canonical, redo, bytes, maximum, &mut hooks)
            .map_err(|error| format!("failed to durably commit {canonical}: {error}"))
    }

    fn read_record(&self, name: &str, maximum: usize) -> Result<Vec<u8>, String> {
        self.validate()?;
        let (descriptor, identity) =
            open_retained_leaf(&self.root, name, None, true, &format!("durable {name}"))?;
        let bytes = read_bounded_leaf(descriptor, identity, maximum, &format!("durable {name}"))?;
        let _ = open_retained_leaf(
            &self.root,
            name,
            Some(identity),
            true,
            &format!("durable {name}"),
        )?;
        Ok(bytes)
    }
}

fn is_private_regular_file(metadata: &Metadata) -> bool {
    metadata.is_file()
        && metadata.uid() == rustix::process::geteuid().as_raw()
        && metadata.nlink() == 1
        && metadata.permissions().mode() & 0o077 == 0
}

fn open_retained_leaf(
    root: &rustix::fd::OwnedFd,
    relative: &str,
    expected: Option<ObjectIdentity>,
    require_private_mode: bool,
    label: &str,
) -> Result<(File, ObjectIdentity), String> {
    validate_relative(relative)?;
    let descriptor = rustix::fs::openat2(
        root,
        relative,
        rustix::fs::OFlags::RDONLY
            | rustix::fs::OFlags::NOFOLLOW
            | rustix::fs::OFlags::NONBLOCK
            | rustix::fs::OFlags::CLOEXEC,
        rustix::fs::Mode::empty(),
        rustix::fs::ResolveFlags::BENEATH
            | rustix::fs::ResolveFlags::NO_SYMLINKS
            | rustix::fs::ResolveFlags::NO_MAGICLINKS
            | rustix::fs::ResolveFlags::NO_XDEV,
    )
    .map(File::from)
    .map_err(|error| format!("failed to open {label}: {error}"))?;
    let metadata = descriptor
        .metadata()
        .map_err(|error| format!("failed to inspect {label}: {error}"))?;
    let private_mode_is_valid = !require_private_mode || metadata.permissions().mode() & 0o077 == 0;
    if !metadata.is_file()
        || metadata.uid() != rustix::process::geteuid().as_raw()
        || metadata.nlink() != 1
        || !private_mode_is_valid
    {
        return Err(format!("{label} is not a private regular file"));
    }
    let identity = ObjectIdentity::from_metadata(&metadata);
    if expected.is_some_and(|expected| identity != expected) {
        return Err(format!("{label} object identity changed"));
    }
    Ok((descriptor, identity))
}

fn read_bounded_leaf(
    mut file: File,
    identity: ObjectIdentity,
    maximum: usize,
    label: &str,
) -> Result<Vec<u8>, String> {
    let length = usize::try_from(identity.size)
        .map_err(|_| format!("{label} length does not fit memory"))?;
    if length > maximum {
        return Err(format!("{label} exceeds its byte bound"));
    }
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(length)
        .map_err(|_| format!("failed to reserve {label} bytes"))?;
    let read_limit = u64::try_from(maximum)
        .map_err(|_| format!("{label} bound does not fit u64"))?
        .checked_add(1)
        .ok_or_else(|| format!("{label} bound overflow"))?;
    Read::by_ref(&mut file)
        .take(read_limit)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("failed to read {label}: {error}"))?;
    if bytes.len() > maximum || bytes.len() != length {
        return Err(format!("{label} changed while it was read"));
    }
    let after = file
        .metadata()
        .map_err(|error| format!("failed to re-inspect {label}: {error}"))?;
    if ObjectIdentity::from_metadata(&after) != identity {
        return Err(format!("{label} changed while it was read"));
    }
    Ok(bytes)
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Artifact {
    relative: String,
    length: u64,
    digest: [u8; 32],
    identity: ObjectIdentity,
}

struct RetainedDispatchSourceV1 {
    relative: String,
    file: File,
    identity: ObjectIdentity,
    bytes: Vec<u8>,
    digest: [u8; 32],
}

impl RetainedDispatchSourceV1 {
    fn open(custody: &OutputCustody, artifact: &Artifact) -> Result<Self, String> {
        if artifact.length == 0 || artifact.length > MAX_PROFILER_IMPORT_SOURCE_BYTES {
            return Err("dispatch source is outside the import byte bound".to_owned());
        }
        let (mut file, identity) = open_retained_leaf(
            &custody.root,
            artifact.relative.as_str(),
            Some(artifact.identity),
            false,
            "dispatch source",
        )?;
        let metadata = file
            .metadata()
            .map_err(|error| format!("failed to inspect retained dispatch source: {error}"))?;
        if metadata.len() != artifact.length {
            return Err("dispatch source changed before retention".to_owned());
        }
        let maximum = usize::try_from(MAX_PROFILER_IMPORT_SOURCE_BYTES)
            .map_err(|_| "dispatch source bound does not fit memory".to_owned())?;
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(
                usize::try_from(artifact.length)
                    .map_err(|_| "dispatch source length does not fit memory".to_owned())?,
            )
            .map_err(|_| "failed to reserve dispatch source bytes".to_owned())?;
        Read::by_ref(&mut file)
            .take(MAX_PROFILER_IMPORT_SOURCE_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|error| format!("failed to read retained dispatch source: {error}"))?;
        if bytes.is_empty()
            || bytes.len() > maximum
            || bytes.len() as u64 != artifact.length
            || <[u8; 32]>::from(Sha256::digest(&bytes)) != artifact.digest
        {
            return Err("dispatch source changed while it was retained".to_owned());
        }
        Ok(Self {
            relative: artifact.relative.clone(),
            file,
            identity,
            bytes,
            digest: artifact.digest,
        })
    }

    fn revalidate(&self, custody: &OutputCustody) -> Result<(), String> {
        if ObjectIdentity::from_metadata(
            &self
                .file
                .metadata()
                .map_err(|error| format!("failed to re-inspect dispatch source: {error}"))?,
        ) != self.identity
        {
            return Err("retained dispatch source descriptor changed".to_owned());
        }
        let (reopened, reopened_identity) = open_retained_leaf(
            &custody.root,
            self.relative.as_str(),
            Some(self.identity),
            false,
            "retained dispatch source",
        )?;
        let maximum = usize::try_from(MAX_PROFILER_IMPORT_SOURCE_BYTES)
            .map_err(|_| "dispatch source bound does not fit memory".to_owned())?;
        let bytes = read_bounded_leaf(
            reopened,
            reopened_identity,
            maximum,
            "retained dispatch source",
        )?;
        if bytes != self.bytes || <[u8; 32]>::from(Sha256::digest(&bytes)) != self.digest {
            return Err("dispatch source bytes changed after import".to_owned());
        }
        Ok(())
    }
}

enum DispatchImportOutcomeV1 {
    Unavailable(&'static str),
    Imported {
        source: RetainedDispatchSourceV1,
        source_kind: DispatchImportSourceKindV1,
        binding: Box<DispatchImportBindingV1>,
        product: Box<DispatchImportProductV1>,
    },
}

fn select_dispatch_import_v1(
    plan: &Plan,
    custody: &OutputCustody,
    artifacts: &[Artifact],
) -> Result<DispatchImportOutcomeV1, String> {
    if !matches!(
        plan.options.kind,
        ProfileKind::DispatchJson | ProfileKind::DispatchCsv
    ) {
        return Ok(DispatchImportOutcomeV1::Unavailable(
            "att-decoding-remains-deferred",
        ));
    }
    let Some(kir) = &plan.verified_kir_v7 else {
        return Ok(DispatchImportOutcomeV1::Unavailable(
            "exact-canonical-kir-v7-not-provided",
        ));
    };
    if !matches!(kir.compatibility, KirTargetCompatibilityV1::Ready(_)) {
        return Ok(DispatchImportOutcomeV1::Unavailable(
            "kir-v7-and-direct-kfd-family-compatibility-unavailable",
        ));
    }
    kir.revalidate()?;
    let source_kind = match plan.options.kind {
        ProfileKind::DispatchJson => DispatchImportSourceKindV1::Rocprofv3KernelDispatchJson,
        ProfileKind::DispatchCsv => DispatchImportSourceKindV1::Rocprofv3KernelDispatchCsv,
        ProfileKind::Att => {
            return Ok(DispatchImportOutcomeV1::Unavailable(
                "att-decoding-remains-deferred",
            ));
        }
    };
    let mut selected_source = None;
    for artifact in artifacts.iter().filter(|artifact| {
        artifact.length > 0 && artifact.length <= MAX_PROFILER_IMPORT_SOURCE_BYTES
    }) {
        let source = RetainedDispatchSourceV1::open(custody, artifact)?;
        let admission = match source_kind {
            DispatchImportSourceKindV1::Rocprofv3KernelDispatchJson => {
                project_rocprofv3_json_dispatch_agents_v4(&source.bytes).map(|_| ())
            }
            DispatchImportSourceKindV1::Rocprofv3KernelDispatchCsv => {
                rocprofv3_csv_source_agent_bindings_v4(&source.bytes).map(|_| ())
            }
        };
        let schema_valid = classify_dispatch_source_admission_v1(admission)?;
        if !schema_valid {
            continue;
        }
        if selected_source.is_some() {
            return Ok(DispatchImportOutcomeV1::Unavailable(
                "multiple-schema-valid-dispatch-sources",
            ));
        }
        selected_source = Some(source);
    }
    let Some(source) = selected_source else {
        return Ok(DispatchImportOutcomeV1::Unavailable(
            "no-schema-valid-dispatch-source",
        ));
    };
    let targets = match source_kind {
        DispatchImportSourceKindV1::Rocprofv3KernelDispatchJson => {
            let projection = project_rocprofv3_json_dispatch_agents_v4(&source.bytes)
                .map_err(|_| "selected rocprof JSON source failed exact re-admission".to_owned())?;
            match json_dispatch_targets_v1(projection.agent_bindings(), &plan.devices) {
                Ok(targets) => targets,
                Err(_) => {
                    return Ok(DispatchImportOutcomeV1::Unavailable(
                        "schema-valid-dispatch-source-target-incompatible",
                    ));
                }
            }
        }
        DispatchImportSourceKindV1::Rocprofv3KernelDispatchCsv => {
            match csv_dispatch_targets_v1(&source.bytes, &plan.devices) {
                Ok(targets) => targets,
                Err(_) => {
                    return Ok(DispatchImportOutcomeV1::Unavailable(
                        "schema-valid-dispatch-source-target-incompatible",
                    ));
                }
            }
        }
    };
    let binding = DispatchImportBindingV1 {
        collection_authorization: CaptureIdentityV1::new(plan.authorization)
            .map_err(|_| "collection authorization identity is invalid".to_owned())?,
        source_relative: source.relative.clone(),
        source_artifact: raw_content_identity_v1(source.digest, source.bytes.len() as u64)?,
        kernel_ir: ContentIdentityRecordV1 {
            scheme: ContentSchemeV1::DomainSeparatedSha256,
            format_version: 1,
            digest: CaptureIdentityV1::new(*kir.owner.identity().digest())
                .map_err(|_| "canonical KIR V7 identity is invalid".to_owned())?,
            canonical_len: kir.owner.identity().canonical_length(),
        },
        environment: raw_content_identity_v1(
            plan.environment_digest,
            plan.environment_bytes.len() as u64,
        )?,
        collector_tool: raw_content_identity_v1(
            plan.collector_tool_digest,
            plan.collector_tool_bytes.len() as u64,
        )?,
        collector_configuration: raw_content_identity_v1(
            plan.configuration_digest,
            plan.configuration.len() as u64,
        )?,
        targets,
        wave_width: WaveWidthV1::Wave64,
    };
    let product = import_dispatch_v1(source_kind, &source.bytes, binding.clone())
        .map_err(|error| format!("selected dispatch source import failed: {error}"))?;
    readmit_dispatch_import_tuple_v1(
        source_kind,
        &source.bytes,
        binding.clone(),
        &product.bundle_bytes,
        &product.capture_bytes,
        &product.receipt_bytes,
    )
    .map_err(|error| format!("dispatch import tuple readmission failed: {error}"))?;
    Ok(DispatchImportOutcomeV1::Imported {
        source,
        source_kind,
        binding: Box::new(binding),
        product: Box::new(product),
    })
}

fn classify_dispatch_source_admission_v1(
    admission: Result<(), ProfilerBundleErrorV4>,
) -> Result<bool, String> {
    match admission {
        Ok(()) => Ok(true),
        Err(
            ProfilerBundleErrorV4::SizeOverflow
            | ProfilerBundleErrorV4::AllocationFailure
            | ProfilerBundleErrorV4::JsonEncode
            | ProfilerBundleErrorV4::IdentityFailure,
        ) => Err("dispatch source admission failed internally".to_owned()),
        Err(_) => Ok(false),
    }
}

fn json_dispatch_targets_v1(
    catalog: &[RocprofJsonGpuAgentBindingV4],
    devices: &[DeviceIdentity],
) -> Result<Vec<DispatchImportTargetBindingV1>, String> {
    let mut targets = Vec::new();
    targets
        .try_reserve(catalog.len())
        .map_err(|_| "failed to reserve JSON dispatch targets".to_owned())?;
    for agent in catalog {
        let device = devices
            .iter()
            .find(|device| device.node == agent.node_id)
            .ok_or_else(|| "rocprof JSON agent node is absent from direct KFD".to_owned())?;
        if agent.gpu_id != device.hardware.gpu_id
            || agent.simd_count != device.hardware.simd_count
            || agent.vendor_id != device.hardware.vendor_id
            || agent.device_id != device.hardware.device_id
            || agent.location_id != device.hardware.location_id
            || agent.domain != device.hardware.domain
            || agent.gfx_target_version != device.hardware.gfx_target_version
            || agent.wave_front_size != device.hardware.wave_front_size
            || agent.num_xcc != device.hardware.num_xcc
        {
            return Err("rocprof JSON agent contradicts direct KFD hardware fields".to_owned());
        }
        targets.push(dispatch_target_v1(
            agent.process_index,
            Some(agent.process_id),
            agent.source_agent_id,
            device,
        )?);
    }
    Ok(targets)
}

fn csv_dispatch_targets_v1(
    source: &[u8],
    devices: &[DeviceIdentity],
) -> Result<Vec<DispatchImportTargetBindingV1>, String> {
    rocprofv3_csv_source_agent_bindings_v4(source)
        .map_err(|_| "rocprof CSV process/agent relation is invalid".to_owned())?
        .into_iter()
        .map(|binding| {
            let device = devices
                .iter()
                .find(|device| device.node == binding.node_id)
                .ok_or_else(|| "rocprof CSV agent node is absent from direct KFD".to_owned())?;
            dispatch_target_v1(
                binding.process_index,
                binding.process_id,
                u64::from(binding.node_id),
                device,
            )
        })
        .collect()
}

fn dispatch_target_v1(
    process_index: u32,
    source_process_id: Option<u64>,
    source_agent_id: u64,
    device: &DeviceIdentity,
) -> Result<DispatchImportTargetBindingV1, String> {
    let family = match device.target_profile.status {
        ObservedGpuTargetProfileStatusV1::Observed(ObservedGpuTargetProfileV1::Gfx942) => {
            ObservedTargetFamilyV1::Gfx942
        }
        ObservedGpuTargetProfileStatusV1::Observed(ObservedGpuTargetProfileV1::Gfx950) => {
            ObservedTargetFamilyV1::Gfx950
        }
        ObservedGpuTargetProfileStatusV1::Unavailable(_) => {
            return Err("direct KFD target profile is unavailable".to_owned());
        }
    };
    let record = device.target_profile_record();
    Ok(DispatchImportTargetBindingV1 {
        process_index,
        source_process_id,
        source_agent_id,
        kfd_node: device.node,
        stable_identity: raw_content_identity_v1(device.digest, device.bytes.len() as u64)?,
        target_profile_record: domain_content_identity_v1(
            b"fe2o3.observed-gpu-target-profile.v1\0",
            record.as_bytes(),
        )?,
        family,
        gfx_target_version: device.hardware.gfx_target_version,
        wave_width: u16::try_from(device.hardware.wave_front_size)
            .map_err(|_| "direct KFD wave width is out of range".to_owned())?,
    })
}

fn raw_content_identity_v1(
    digest: [u8; 32],
    length: u64,
) -> Result<ContentIdentityRecordV1, String> {
    Ok(ContentIdentityRecordV1 {
        scheme: ContentSchemeV1::RawCanonicalSha256,
        format_version: 1,
        digest: CaptureIdentityV1::new(digest)
            .map_err(|_| "raw content identity is invalid".to_owned())?,
        canonical_len: length,
    })
}

fn domain_content_identity_v1(
    domain: &[u8],
    bytes: &[u8],
) -> Result<ContentIdentityRecordV1, String> {
    let mut digest = Sha256::new();
    digest.update(domain);
    digest.update((bytes.len() as u64).to_le_bytes());
    digest.update(bytes);
    Ok(ContentIdentityRecordV1 {
        scheme: ContentSchemeV1::DomainSeparatedSha256,
        format_version: 1,
        digest: CaptureIdentityV1::new(digest.finalize().into())
            .map_err(|_| "domain content identity is invalid".to_owned())?,
        canonical_len: bytes.len() as u64,
    })
}

fn scan_artifacts(custody: &OutputCustody, storage_limit: u64) -> Result<Vec<Artifact>, String> {
    scan_artifacts_with_entry_limit(custody, storage_limit, MAX_ARTIFACTS)
}

fn scan_artifacts_with_entry_limit(
    custody: &OutputCustody,
    storage_limit: u64,
    maximum_entries: usize,
) -> Result<Vec<Artifact>, String> {
    if maximum_entries == 0 || maximum_entries > MAX_ARTIFACTS {
        return Err("collector output has an invalid entry-count bound".to_owned());
    }
    let root = &custody.path;
    let mut pending = vec![(root.to_path_buf(), 0_usize)];
    let mut artifacts = Vec::new();
    let mut total = 0_u64;
    let mut entry_count = 0_usize;
    while let Some((directory, depth)) = pending.pop() {
        if depth > MAX_ARTIFACT_DEPTH {
            return Err("collector output exceeds the directory-depth bound".to_owned());
        }
        let directory_entries = fs::read_dir(&directory)
            .map_err(|error| format!("failed to enumerate collector output: {error}"))?;
        let mut entries = Vec::new();
        for entry in directory_entries {
            entry_count = entry_count
                .checked_add(1)
                .ok_or("collector output entry count overflow")?;
            if entry_count > maximum_entries {
                return Err("collector output exceeds the global entry-count bound".to_owned());
            }
            entries
                .try_reserve(1)
                .map_err(|_| "failed to reserve collector output entry metadata".to_owned())?;
            entries.push(
                entry.map_err(|error| format!("failed to enumerate collector output: {error}"))?,
            );
        }
        entries.sort_by_key(|entry| entry.file_name());
        for entry in entries {
            let path = entry.path();
            let relative_path = path
                .strip_prefix(root)
                .map_err(|_| "collector output escaped custody")?;
            let relative = relative_path
                .to_str()
                .ok_or("collector output path is not UTF-8")?
                .replace('\\', "/");
            if relative == OWNERSHIP_FILE {
                continue;
            }
            if [
                MANIFEST_FILE,
                MANIFEST_REDO_FILE,
                PROFILE_DISPATCH_BUNDLE_FILE_V1,
                PROFILE_DISPATCH_BUNDLE_REDO_FILE_V1,
                PROFILE_DISPATCH_RECEIPT_FILE_V1,
                PROFILE_DISPATCH_RECEIPT_REDO_FILE_V1,
            ]
            .contains(&relative.as_str())
            {
                return Err(format!(
                    "collector precreated reserved transaction entry {relative}"
                ));
            }
            validate_relative(&relative)?;
            let metadata = fs::symlink_metadata(&path).map_err(|error| {
                format!("failed to inspect collector output {relative}: {error}")
            })?;
            if metadata.file_type().is_symlink() {
                return Err(format!(
                    "collector output contains symbolic link {relative}"
                ));
            }
            if metadata.is_dir() {
                pending
                    .try_reserve(1)
                    .map_err(|_| "failed to reserve collector directory traversal".to_owned())?;
                pending.push((path, depth + 1));
                continue;
            }
            if !metadata.is_file() {
                return Err(format!(
                    "collector output contains non-regular object {relative}"
                ));
            }
            if metadata.nlink() != 1 {
                return Err(format!(
                    "collector output contains multiply linked file {relative}"
                ));
            }
            total = total
                .checked_add(metadata.len())
                .ok_or("collector output length overflow")?;
            if total > storage_limit {
                return Err("collector output exceeds the storage limit".to_owned());
            }
            if artifacts.len() == MAX_ARTIFACTS {
                return Err("collector output exceeds the artifact-count bound".to_owned());
            }
            let (digest, length) = hash_file(custody, &relative, &metadata)?;
            artifacts
                .try_reserve(1)
                .map_err(|_| "failed to reserve collector artifact metadata".to_owned())?;
            artifacts.push(Artifact {
                relative,
                length,
                digest,
                identity: ObjectIdentity::from_metadata(&metadata),
            });
        }
    }
    artifacts.sort_by(|left, right| left.relative.cmp(&right.relative));
    Ok(artifacts)
}

fn validate_relative(path: &str) -> Result<(), String> {
    if path.is_empty() || path.len() > 4096 || path.contains("//") || Path::new(path).is_absolute()
    {
        return Err("collector output has an invalid relative path".to_owned());
    }
    if Path::new(path)
        .components()
        .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err("collector output has a noncanonical relative path".to_owned());
    }
    Ok(())
}

fn hash_file(
    custody: &OutputCustody,
    relative: &str,
    expected: &Metadata,
) -> Result<([u8; 32], u64), String> {
    hash_file_with_reopen_hook(custody, relative, expected, || {})
}

fn hash_file_with_reopen_hook(
    custody: &OutputCustody,
    relative: &str,
    expected: &Metadata,
    before_reopen: impl FnOnce(),
) -> Result<([u8; 32], u64), String> {
    let expected_identity = ObjectIdentity::from_metadata(expected);
    let (mut file, opened_identity) = open_retained_leaf(
        &custody.root,
        relative,
        Some(expected_identity),
        false,
        "collector artifact",
    )?;
    debug_assert_eq!(opened_identity, expected_identity);
    let mut hasher = Sha256::new();
    let mut length = 0_u64;
    let mut buffer = [0_u8; 16 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|error| format!("failed to read collector artifact: {error}"))?;
        if read == 0 {
            break;
        }
        length = length
            .checked_add(read as u64)
            .ok_or("collector artifact length overflow")?;
        if length > expected.len() {
            return Err("collector artifact changed while hashing".to_owned());
        }
        hasher.update(&buffer[..read]);
    }
    before_reopen();
    if length != expected.len()
        || ObjectIdentity::from_metadata(
            &file
                .metadata()
                .map_err(|error| format!("failed to re-inspect collector artifact: {error}"))?,
        ) != expected_identity
        || open_retained_leaf(
            &custody.root,
            relative,
            Some(expected_identity),
            false,
            "collector artifact path",
        )
        .is_err()
    {
        return Err("collector artifact changed while hashing".to_owned());
    }
    Ok((hasher.finalize().into(), length))
}

fn render_manifest(
    plan: &Plan,
    artifacts: &[Artifact],
    dispatch_import: &DispatchImportOutcomeV1,
) -> String {
    let mut output = String::new();
    line(&mut output, "schema", "fe2o3-profile-artifact-manifest-v1");
    line(&mut output, "plan-sha256", hex(&plan.authorization));
    line(&mut output, "profile-kind", plan.options.kind.name());
    line(
        &mut output,
        "environment-identity",
        content_identity(
            &plan.environment_digest,
            plan.environment_bytes.len() as u64,
        ),
    );
    line(
        &mut output,
        "tool-identity",
        content_identity(
            &plan.collector_tool_digest,
            plan.collector_tool_bytes.len() as u64,
        ),
    );
    line(
        &mut output,
        "configuration-identity",
        content_identity(&plan.configuration_digest, plan.configuration.len() as u64),
    );
    render_target_profile_observations(&mut output, &plan.devices);
    render_expected(&mut output, plan.options.kind);
    for (index, artifact) in artifacts.iter().enumerate() {
        line(
            &mut output,
            &format!("artifact[{index}]"),
            format!(
                "path={:?};identity={}",
                artifact.relative,
                content_identity(&artifact.digest, artifact.length)
            ),
        );
    }
    for (index, artifact) in artifacts.iter().enumerate() {
        line(
            &mut output,
            &format!("import-source-candidate[{index}]"),
            format!(
                "path={:?};bytes={};status={}",
                artifact.relative,
                artifact.length,
                if artifact.length <= MAX_PROFILER_IMPORT_SOURCE_BYTES {
                    "content-schema-eligible-requires-admission"
                } else {
                    "unavailable-exceeds-import-source-byte-limit"
                }
            ),
        );
    }
    render_import_plan(&mut output, plan);
    render_dispatch_import_outcome_v1(&mut output, dispatch_import);
    line(&mut output, "att-observation-origin", "unavailable");
    line(
        &mut output,
        "att-observation-reason",
        "bundle-v4-import-not-run",
    );
    output
}

fn render_successful_collection(
    plan: &Plan,
    result: Supervised,
    artifacts: &[Artifact],
    dispatch_import: &DispatchImportOutcomeV1,
) -> CommandReport {
    let mut output = String::new();
    line(&mut output, "schema", "fe2o3-profile-collection-v1");
    line(&mut output, "authority", "explicit-plan-bound-collection");
    line(
        &mut output,
        "outcome",
        if artifacts.is_empty() {
            "collector-completed-no-artifacts"
        } else {
            "collector-completed-artifacts-unvalidated"
        },
    );
    line(&mut output, "plan-sha256", hex(&plan.authorization));
    line_debug(&mut output, "output-directory", &plan.output_directory);
    line_debug(
        &mut output,
        "manifest",
        plan.output_directory.join(MANIFEST_FILE),
    );
    render_status(&mut output, result.status.as_ref());
    render_capture(&mut output, "stdout", &result.stdout);
    render_capture(&mut output, "stderr", &result.stderr);
    for (index, artifact) in artifacts.iter().enumerate() {
        line(
            &mut output,
            &format!("artifact[{index}]"),
            format!(
                "path={:?};identity={}",
                artifact.relative,
                content_identity(&artifact.digest, artifact.length)
            ),
        );
    }
    render_dispatch_import_outcome_v1(&mut output, dispatch_import);
    line(&mut output, "att-observability-origin", "unavailable");
    line(
        &mut output,
        "att-observability-reason",
        "bundle-v4-import-not-run",
    );
    line(
        &mut output,
        "direct-kfd-limitation",
        "rocprofv3-may-complete-without-observing-direct-kfd-dispatches",
    );
    render_import_plan(&mut output, plan);
    output.pop();
    CommandReport {
        output,
        succeeded: true,
    }
}

fn render_dispatch_import_outcome_v1(output: &mut String, outcome: &DispatchImportOutcomeV1) {
    match outcome {
        DispatchImportOutcomeV1::Unavailable(reason) => {
            line(output, "dispatch-observation-origin", "unavailable");
            line(output, "dispatch-observation-reason", reason);
        }
        DispatchImportOutcomeV1::Imported {
            source, product, ..
        } => {
            line(
                output,
                "dispatch-observation-origin",
                "observed-rocprof-source",
            );
            line_debug(output, "dispatch-import-source", &source.relative);
            line(
                output,
                "dispatch-import-source-identity",
                content_identity(&source.digest, source.bytes.len() as u64),
            );
            line(
                output,
                "dispatch-import-bundle",
                PROFILE_DISPATCH_BUNDLE_FILE_V1,
            );
            line(
                output,
                "dispatch-import-bundle-identity",
                content_record(&product.bundle_identity),
            );
            line(
                output,
                "dispatch-import-capture-identity",
                content_record(&product.capture_identity),
            );
            line(
                output,
                "dispatch-import-receipt",
                PROFILE_DISPATCH_RECEIPT_FILE_V1,
            );
            line(
                output,
                "dispatch-import-receipt-identity",
                content_record(&product.receipt_identity),
            );
            line(
                output,
                "dispatch-import-run-identity",
                hex(&product.bundle.run_identity.as_bytes()),
            );
            line(
                output,
                "dispatch-import-count",
                product.bundle.coverage.imported_dispatches,
            );
            for name in [
                "compiler-authority",
                "runtime-authority",
                "executed-artifact-identity",
                "source-map-identity",
                "kernel-symbol-association",
                "characteristic-correlation",
                "decoded-att-events",
                "performance-authority",
            ] {
                line(output, name, false);
            }
            line(
                output,
                "target-compatibility-scope",
                "direct-kfd-gfx-family-and-wave64-only-xnack-unobserved",
            );
        }
    }
}

fn render_failed_collection(
    plan: &Plan,
    result: Supervised,
    identity_error: Option<String>,
    cleanup_error: Option<String>,
) -> CommandReport {
    let mut output = String::new();
    line(&mut output, "schema", "fe2o3-profile-collection-v1");
    line(&mut output, "authority", "explicit-plan-bound-collection");
    let outcome = if identity_error.is_some() {
        "input-identity-changed"
    } else {
        match result.reason {
            StopReason::Exited => "collector-exit-failure",
            StopReason::Timeout => "timeout",
            StopReason::OutputOverflow => "output-overflow",
            StopReason::WaitFailure => "wait-failure",
        }
    };
    line(&mut output, "outcome", outcome);
    line(&mut output, "plan-sha256", hex(&plan.authorization));
    render_status(&mut output, result.status.as_ref());
    render_capture(&mut output, "stdout", &result.stdout);
    render_capture(&mut output, "stderr", &result.stderr);
    if let Some(error) = result.wait_error {
        line_debug(&mut output, "wait-error", &error);
    }
    if let Some(error) = identity_error {
        line_debug(&mut output, "identity-error", &error);
    }
    line(
        &mut output,
        "output-cleanup",
        if cleanup_error.is_some() {
            "failed"
        } else {
            "complete"
        },
    );
    if let Some(error) = cleanup_error {
        line_debug(&mut output, "cleanup-error", &error);
    }
    output.pop();
    CommandReport {
        output,
        succeeded: false,
    }
}

fn render_status(output: &mut String, status: Option<&ExitStatus>) {
    use std::os::unix::process::ExitStatusExt as _;
    match status {
        Some(status) if status.success() => line(output, "collector-status", "success"),
        Some(status) if status.code().is_some() => line(
            output,
            "collector-status",
            format!("exit:{}", status.code().unwrap_or_default()),
        ),
        Some(status) => line(
            output,
            "collector-status",
            format!("signal:{}", status.signal().unwrap_or_default()),
        ),
        None => line(output, "collector-status", "unavailable"),
    }
}

fn render_capture(output: &mut String, name: &str, capture: &BoundedCapture) {
    let digest: [u8; 32] = Sha256::digest(&capture.bytes).into();
    line(output, &format!("{name}-bytes"), capture.bytes.len());
    line(output, &format!("{name}-sha256"), hex(&digest));
    line(output, &format!("{name}-overflow"), capture.overflow);
}

fn redacted_value(output: &mut String, name: &str, value: &[u8]) {
    let digest: [u8; 32] = Sha256::digest(value).into();
    line(
        output,
        name,
        format!("redacted:sha256:{}:len={}", hex(&digest), value.len()),
    );
}

fn append_field(output: &mut Vec<u8>, value: &[u8]) {
    output.extend_from_slice(&(value.len() as u64).to_le_bytes());
    output.extend_from_slice(value);
}

fn hash_field(hasher: &mut Sha256, value: &[u8]) {
    hasher.update((value.len() as u64).to_le_bytes());
    hasher.update(value);
}

fn content_identity(digest: &[u8; 32], length: u64) -> String {
    format!("raw:1:{}:{length}", hex(digest))
}

fn content_record(identity: &ContentIdentityRecordV1) -> String {
    format!(
        "{}:{}:{}:{}",
        match identity.scheme {
            ContentSchemeV1::RawCanonicalSha256 => "raw",
            ContentSchemeV1::DomainSeparatedSha256 => "domain",
        },
        identity.format_version,
        hex(&identity.digest.as_bytes()),
        identity.canonical_len,
    )
}

fn hex(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        let _ = write!(output, "{byte:02x}");
    }
    output
}

fn line(output: &mut String, name: &str, value: impl std::fmt::Display) {
    let _ = writeln!(output, "{name}: {value}");
}

fn line_debug(output: &mut String, name: &str, value: impl std::fmt::Debug) {
    let _ = writeln!(output, "{name}: {value:?}");
}

#[cfg(test)]
mod tests {
    use super::*;
    use fe2o3_semantic_import::decode_profiler_bundle_v4;
    use std::cell::Cell;
    use std::sync::atomic::AtomicBool as TestAtomicBool;
    use std::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};

    static NEXT_TOPOLOGY_FIXTURE: AtomicU64 = AtomicU64::new(0);

    struct DelayedFifoWriter {
        stop: Arc<TestAtomicBool>,
        thread: Option<thread::JoinHandle<()>>,
    }

    impl DelayedFifoWriter {
        fn start(path: PathBuf) -> Self {
            let stop = Arc::new(TestAtomicBool::new(false));
            let worker_stop = Arc::clone(&stop);
            let thread = thread::spawn(move || {
                for _ in 0..75 {
                    if worker_stop.load(Ordering::Acquire) {
                        return;
                    }
                    thread::sleep(Duration::from_millis(10));
                }
                loop {
                    if worker_stop.load(Ordering::Acquire) {
                        return;
                    }
                    match OpenOptions::new()
                        .write(true)
                        .custom_flags(libc::O_NONBLOCK | libc::O_CLOEXEC)
                        .open(&path)
                    {
                        Ok(mut fifo) => {
                            let _ = fifo.write_all(b"substituted\n");
                            return;
                        }
                        Err(error) if error.raw_os_error() == Some(libc::ENXIO) => {
                            thread::sleep(Duration::from_millis(5));
                        }
                        Err(_) => return,
                    }
                }
            });
            Self {
                stop,
                thread: Some(thread),
            }
        }

        fn finish(mut self) {
            self.stop.store(true, Ordering::Release);
            self.thread.take().unwrap().join().unwrap();
        }
    }

    impl Drop for DelayedFifoWriter {
        fn drop(&mut self) {
            self.stop.store(true, Ordering::Release);
            if let Some(thread) = self.thread.take() {
                let _ = thread.join();
            }
        }
    }

    fn test_custody(label: &str) -> (PathBuf, OutputCustody) {
        let id = NEXT_TOPOLOGY_FIXTURE.fetch_add(1, AtomicOrdering::Relaxed);
        let output = env::temp_dir().join(format!(
            "cargo-fe2o3-profile-{label}-{}-{id}",
            std::process::id()
        ));
        let custody = OutputCustody::create(&output, &[0x42; 32]).unwrap();
        (output, custody)
    }

    fn replace_with_fifo(path: &Path) {
        fs::remove_file(path).unwrap();
        rustix::fs::mkfifoat(
            rustix::fs::CWD,
            path,
            rustix::fs::Mode::RUSR | rustix::fs::Mode::WUSR,
        )
        .unwrap();
    }

    fn assert_fifo_rejection_is_bounded(
        started: Instant,
        result: Result<(), String>,
        writer: DelayedFifoWriter,
    ) {
        let elapsed = started.elapsed();
        writer.finish();
        assert!(result.is_err());
        assert!(
            elapsed < Duration::from_millis(250),
            "FIFO substitution blocked for {elapsed:?}"
        );
    }

    struct TopologyFixture {
        root: PathBuf,
    }

    impl TopologyFixture {
        fn new(node: u32, properties: &str) -> Self {
            let id = NEXT_TOPOLOGY_FIXTURE.fetch_add(1, AtomicOrdering::Relaxed);
            let root = env::temp_dir().join(format!(
                "cargo-fe2o3-profile-topology-{}-{id}",
                std::process::id()
            ));
            let node_root = root.join(node.to_string());
            fs::create_dir_all(&node_root).expect("create topology fixture");
            fs::write(
                node_root.join("gpu_id"),
                format!("{}\n", u64::from(node) + 1_000),
            )
            .expect("write topology gpu id");
            fs::write(node_root.join("properties"), properties).expect("write topology properties");
            Self { root }
        }

        fn replace(&self, node: u32, properties: &str) {
            fs::write(
                self.root.join(node.to_string()).join("properties"),
                properties,
            )
            .expect("replace topology properties");
        }
    }

    impl Drop for TopologyFixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    fn topology_properties(vendor: u64, target: u64, wave: u64) -> String {
        format!(
            "simd_count 1\nunique_id 42\nvendor_id {vendor}\ndevice_id 29857\ndomain 0\nlocation_id 1\ngfx_target_version {target}\nwave_front_size {wave}\nnum_xcc 8\n"
        )
    }

    fn profile_device(node: u32, vendor: u64, target: u64, wave: u64) -> DeviceIdentity {
        let bytes = format!("stable-device-{node}").into_bytes();
        DeviceIdentity {
            node,
            hardware: test_hardware(node, vendor, target, wave),
            digest: Sha256::digest(&bytes).into(),
            bytes,
            target_profile: ObservedGpuTargetProfileRecordV1::from_direct_kfd_properties(
                vendor, target, wave,
            ),
        }
    }

    fn test_hardware(node: u32, vendor: u64, target: u64, wave: u64) -> KfdGpuHardwareV1 {
        KfdGpuHardwareV1 {
            gpu_id: u64::from(node) + 1_000,
            simd_count: 304,
            vendor_id: vendor,
            device_id: 29_857,
            location_id: 1,
            domain: 0,
            gfx_target_version: target,
            wave_front_size: wave,
            num_xcc: 8,
        }
    }

    fn target_module(target: &str, wave: WaveWidth) -> Module {
        let mut module = Module::new("profile-test");
        module
            .required_capabilities
            .insert(TargetCapability::Extension {
                namespace: AMDGPU_EXACT_TARGET_CAPABILITY_NAMESPACE.to_owned(),
                name: target.to_owned(),
            });
        module
            .required_capabilities
            .insert(TargetCapability::WaveWidth(wave));
        module
    }

    #[test]
    fn parser_is_closed_and_authorization_is_canonical() {
        let base = ["--output-dir", "/tmp/new-profile-output", "--", "/bin/true"];
        assert!(parse_options(&base.map(str::to_owned)).is_ok());
        let hostile = [
            vec!["--unknown", "--output-dir", "/tmp/x", "--", "/bin/true"]
                .into_iter()
                .map(str::to_owned)
                .collect(),
            vec![
                "--output-dir",
                "/tmp/x",
                "--output-dir",
                "/tmp/y",
                "--",
                "/bin/true",
            ]
            .into_iter()
            .map(str::to_owned)
            .collect(),
            vec!["--collect", "--output-dir", "/tmp/x", "--", "/bin/true"]
                .into_iter()
                .map(str::to_owned)
                .collect(),
            vec![
                "--authorize-collection".to_owned(),
                "A".repeat(64),
                "--output-dir".to_owned(),
                "/tmp/x".to_owned(),
                "--".to_owned(),
                "/bin/true".to_owned(),
            ],
            vec![
                "--timeout-ms",
                "01",
                "--output-dir",
                "/tmp/x",
                "--",
                "/bin/true",
            ]
            .into_iter()
            .map(str::to_owned)
            .collect(),
        ];
        for args in hostile {
            assert!(parse_options(&args).is_err());
        }
    }

    #[test]
    fn sealed_images_are_read_only_content_exact_and_distinct_from_provenance() {
        let pin = FilePin::open(
            Path::new("/bin/true"),
            "test target",
            MAX_TARGET_BYTES,
            true,
        )
        .unwrap();
        assert_ne!(pin.identity.inode, pin.image.identity.inode);
        assert_eq!(pin.digest, pin.image.digest);
        assert_eq!(pin.identity.size, pin.image.identity.size);
        pin.validate("test target").unwrap();
        assert!(
            OpenOptions::new()
                .write(true)
                .open(pin.image.external_path())
                .is_err()
        );
    }

    #[test]
    fn installed_adapter_is_closed_stable_and_removes_role_environment() {
        let source = format!(
            "import os\ndef run():\n    app_env = dict(os.environ)\n{ROCPROF_RUN_ROOT_BLOCK_V1}\n{ROCPROF_TOOL_ASSIGNMENT_V1}\n{ROCPROF_CORE_ASSIGNMENT_V1}\n{ROCPROF_TOOL_RESOLUTION_V1}\n{ROCPROF_CORE_RESOLUTION_V1}\n{ROCPROF_PRELOAD_ORDER_V1}\n"
        );
        let adapted =
            String::from_utf8(derive_installed_collector_adapter_v1(&source).unwrap()).unwrap();
        assert!(adapted.contains(&format!(
            "ROCPROF_TOOL_LIBRARY = app_env.pop(\"{SEALED_TOOL_ENV_V1}\")"
        )));
        assert!(adapted.contains(&format!(
            "ROCPROF_SDK_LIBRARY = app_env.pop(\"{SEALED_CORE_ENV_V1}\")"
        )));
        assert!(!adapted.contains("os.environ.pop"));
        assert!(adapted.contains(&format!(
            "ROCM_DIR = app_env.pop(\"{LOGICAL_ROCM_ROOT_ENV_V1}\")"
        )));
        assert!(
            adapted.find("ROCPROF_SDK_LIBRARY,\n").unwrap()
                < adapted.find("ROCPROF_TOOL_LIBRARY,\n").unwrap()
        );
        for hostile in [
            format!("{source}\n{ROCPROF_TOOL_ASSIGNMENT_V1}\n"),
            source.replace(
                ROCPROF_CORE_ASSIGNMENT_V1,
                "    ROCPROF_SDK_LIBRARY = 'other'",
            ),
            source.replace(ROCPROF_PRELOAD_ORDER_V1, "    append_preload = []"),
            source.replace(ROCPROF_RUN_ROOT_BLOCK_V1, "    ROCM_DIR = '/unsupported'"),
        ] {
            assert!(derive_installed_collector_adapter_v1(&hostile).is_err());
        }
    }

    #[test]
    fn att_plan_discloses_sealed_decoder_boundary_and_collection_fails_closed() {
        let id = NEXT_TOPOLOGY_FIXTURE.fetch_add(1, AtomicOrdering::Relaxed);
        let root = env::temp_dir().join(format!(
            "cargo-fe2o3-profile-att-sealed-{}-{id}",
            std::process::id()
        ));
        fs::create_dir(&root).unwrap();
        let tool = root.join("rocprofv3");
        fs::write(
            &tool,
            "#!/usr/bin/env python3\n# --kernel-trace --advanced-thread-trace\nraise SystemExit(99)\n",
        )
        .unwrap();
        fs::set_permissions(&tool, fs::Permissions::from_mode(0o700)).unwrap();
        let output = root.join("capture");
        let python = discover_python().expect("test requires the reviewed native Python");
        let base = vec![
            "--kind".to_owned(),
            "att".to_owned(),
            "--tool".to_owned(),
            tool.to_string_lossy().into_owned(),
            "--python".to_owned(),
            python.to_string_lossy().into_owned(),
            "--output-dir".to_owned(),
            output.to_string_lossy().into_owned(),
            "--".to_owned(),
            "/bin/true".to_owned(),
        ];
        let plan = command(&base).unwrap();
        assert!(plan.output().contains("collection-readiness: unavailable"));
        assert!(plan.output().contains(
            "collection-unavailable-reason: att-decoder-requires-mutable-directory-namespace-without-sealed-route"
        ));
        let authorization = plan
            .output()
            .lines()
            .find_map(|line| line.strip_prefix("collection-authorization: "))
            .unwrap();
        let mut collect = base;
        collect.splice(
            0..0,
            [
                "--collect".to_owned(),
                "--authorize-collection".to_owned(),
                authorization.to_owned(),
            ],
        );
        let error = command(&collect).unwrap_err();
        assert!(error.contains("ATT collection is unavailable under sealed execution"));
        assert!(!output.exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn dispatch_source_admission_propagates_internal_failures() {
        assert!(classify_dispatch_source_admission_v1(Ok(())).unwrap());
        assert!(
            !classify_dispatch_source_admission_v1(Err(ProfilerBundleErrorV4::InvalidRocprofJson))
                .unwrap()
        );
        for error in [
            ProfilerBundleErrorV4::SizeOverflow,
            ProfilerBundleErrorV4::AllocationFailure,
            ProfilerBundleErrorV4::JsonEncode,
            ProfilerBundleErrorV4::IdentityFailure,
        ] {
            assert!(classify_dispatch_source_admission_v1(Err(error)).is_err());
        }
    }

    #[test]
    fn generic_core_collects_imports_and_publishes_without_host_kfd() {
        let id = NEXT_TOPOLOGY_FIXTURE.fetch_add(1, AtomicOrdering::Relaxed);
        let root = env::temp_dir().join(format!(
            "cargo-fe2o3-profile-generic-core-{}-{id}",
            std::process::id()
        ));
        fs::create_dir(&root).unwrap();
        let tool = root.join("rocprofv3");
        let source = include_bytes!(
            "../../fe2o3-semantic-import/tests/fixtures/rocprofv3-installed-97f5574-kernel-dispatch-schema.json"
        );
        let source_literal =
            serde_json::to_string(&String::from_utf8(source.to_vec()).unwrap()).unwrap();
        let execution_evidence = root.join("exact-script-execution.txt");
        let execution_evidence_literal =
            serde_json::to_string(&execution_evidence.to_string_lossy()).unwrap();
        fs::write(
            &tool,
            format!(
                "#!/usr/bin/env python3\n# --kernel-trace --advanced-thread-trace\nimport os, subprocess, sys\nwith open({execution_evidence_literal}, 'w', encoding='utf-8') as stream:\n    stream.write(__file__ + '\\n' + sys.argv[0] + '\\n' + str(any(key.startswith('FE2O3_ROCPROF_') for key in os.environ)))\nargs=sys.argv[1:]\nout=args[args.index('--output-directory')+1]\nwith open(os.path.join(out, 'dispatch.json'), 'w', encoding='utf-8') as stream:\n    stream.write({source_literal})\ntarget=args[args.index('--')+1:]\nraise SystemExit(subprocess.run(target, check=False).returncode)\n"
            ),
        )
        .unwrap();
        fs::set_permissions(&tool, fs::Permissions::from_mode(0o755)).unwrap();

        let module = target_module(
            AMDGPU_GFX942_XNACK_MINUS_TARGET_CAPABILITY_NAME,
            WaveWidth::Wave64,
        );
        let kir = VerifiedCanonicalKernelIrV7::from_module(module).unwrap();
        let kir_path = root.join("generic.kir");
        fs::write(&kir_path, kir.canonical_bytes()).unwrap();
        let output = root.join("capture");
        let python = discover_python().expect("test requires the reviewed native Python");
        let args = [
            "--kind".to_owned(),
            "dispatch-json".to_owned(),
            "--tool".to_owned(),
            tool.to_string_lossy().into_owned(),
            "--python".to_owned(),
            python.to_string_lossy().into_owned(),
            "--output-dir".to_owned(),
            output.to_string_lossy().into_owned(),
            "--kir-v7".to_owned(),
            kir_path.to_string_lossy().into_owned(),
            "--".to_owned(),
            "/bin/true".to_owned(),
        ];
        let mut plan = prepare_plan(parse_options(&args).unwrap()).unwrap();
        let device_bytes = b"generic-core-stable-kfd-device".to_vec();
        let device = DeviceIdentity {
            node: 7,
            hardware: KfdGpuHardwareV1 {
                gpu_id: 42,
                simd_count: 304,
                vendor_id: EXPECTED_AMD_VENDOR_ID,
                device_id: 29_857,
                location_id: 1,
                domain: 0,
                gfx_target_version: GFX942_TARGET_VERSION,
                wave_front_size: PRODUCTION_WAVE_WIDTH,
                num_xcc: 8,
            },
            digest: Sha256::digest(&device_bytes).into(),
            bytes: device_bytes,
            target_profile: ObservedGpuTargetProfileRecordV1::from_direct_kfd_properties(
                EXPECTED_AMD_VENDOR_ID,
                GFX942_TARGET_VERSION,
                PRODUCTION_WAVE_WIDTH,
            ),
        };
        plan.devices = vec![device.clone()];
        plan.verified_kir_v7.as_mut().unwrap().compatibility =
            KirTargetCompatibilityV1::Ready(ObservedGpuTargetProfileV1::Gfx942);
        plan.configuration = canonical_configuration(plan.options.kind, &plan.devices);
        plan.configuration_digest = Sha256::digest(&plan.configuration).into();
        plan.authorization = authorization_digest(AuthorizationInputs {
            options: &plan.options,
            working_directory: &plan.working_directory,
            output_directory: &plan.output_directory,
            tool: &plan.tool,
            interpreter: &plan.interpreter,
            target: &plan.target,
            environment: &plan.environment_digest,
            devices: &plan.devices,
            configuration: &plan.configuration_digest,
            collector_tool: &plan.collector_tool_digest,
            verified_kir_v7: plan.verified_kir_v7.as_ref(),
        });

        let revalidations = Cell::new(0_u8);
        let report = collect_with_device_revalidator(plan, |devices| {
            revalidations.set(revalidations.get().checked_add(1).unwrap());
            if devices == [device.clone()] {
                Ok(())
            } else {
                Err("synthetic KFD device binding changed".to_owned())
            }
        })
        .unwrap();
        assert_eq!(revalidations.get(), 6);
        assert!(report.succeeded, "{}", report.output);
        assert_eq!(
            fs::read_to_string(execution_evidence).unwrap(),
            format!("{}\n{}\nFalse", tool.display(), tool.display())
        );
        let bundle = output.join(PROFILE_DISPATCH_BUNDLE_FILE_V1);
        let receipt = output.join(PROFILE_DISPATCH_RECEIPT_FILE_V1);
        let manifest = output.join(MANIFEST_FILE);
        assert!(bundle.is_file() && receipt.is_file() && manifest.is_file());
        assert!(decode_profiler_bundle_v4(&fs::read(bundle).unwrap()).is_ok());
        assert!(
            fs::read_to_string(manifest)
                .unwrap()
                .contains("dispatch-observation-origin: observed-rocprof-source")
        );
        assert!(!output.join(PROFILE_DISPATCH_BUNDLE_REDO_FILE_V1).exists());
        assert!(!output.join(PROFILE_DISPATCH_RECEIPT_REDO_FILE_V1).exists());
        assert!(!output.join(MANIFEST_REDO_FILE).exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn kfd_properties_reject_duplicates_and_noncanonical_values() {
        assert!(parse_properties(b"simd_count 1\nunique_id 42\n").is_ok());
        for hostile in [
            b"simd_count 1\nsimd_count 2\n".as_slice(),
            b"simd_count 01\n",
            b"simd-count 1\n",
            b"simd_count -1\n",
            b"simd_count 1 extra\n",
        ] {
            assert!(parse_properties(hostile).is_err());
        }
    }

    #[test]
    fn absolute_node_mapping_is_authorized_and_revalidated() {
        let device = |node| DeviceIdentity {
            node,
            hardware: test_hardware(
                node,
                EXPECTED_AMD_VENDOR_ID,
                GFX942_TARGET_VERSION,
                PRODUCTION_WAVE_WIDTH,
            ),
            bytes: b"stable-device".to_vec(),
            digest: [7; 32],
            target_profile: ObservedGpuTargetProfileRecordV1::from_direct_kfd_properties(
                EXPECTED_AMD_VENDOR_ID,
                GFX942_TARGET_VERSION,
                PRODUCTION_WAVE_WIDTH,
            ),
        };
        let planned = vec![device(2)];
        let remapped = vec![device(7)];
        assert!(device_bindings_match(&planned, &planned));
        assert!(!device_bindings_match(&planned, &remapped));

        let digest = |devices: &[DeviceIdentity]| {
            let mut hasher = Sha256::new();
            hash_device_bindings(&mut hasher, devices);
            <[u8; 32]>::from(hasher.finalize())
        };
        assert_ne!(digest(&planned), digest(&remapped));
    }

    #[test]
    fn direct_kfd_target_profile_mapping_is_exact_and_typed() {
        let observed = |target| {
            ObservedGpuTargetProfileRecordV1::from_direct_kfd_properties(
                EXPECTED_AMD_VENDOR_ID,
                target,
                PRODUCTION_WAVE_WIDTH,
            )
        };
        assert_eq!(
            observed(GFX942_TARGET_VERSION).status,
            ObservedGpuTargetProfileStatusV1::Observed(ObservedGpuTargetProfileV1::Gfx942)
        );
        assert_eq!(
            observed(GFX950_TARGET_VERSION).status,
            ObservedGpuTargetProfileStatusV1::Observed(ObservedGpuTargetProfileV1::Gfx950)
        );
        assert_eq!(
            observed(90_401).status,
            ObservedGpuTargetProfileStatusV1::Unavailable(
                ObservedGpuTargetProfileUnavailableReasonV1::UnknownGfxTargetVersion
            )
        );
        let unknown = DeviceIdentity {
            node: 3,
            hardware: test_hardware(3, EXPECTED_AMD_VENDOR_ID, 90_401, PRODUCTION_WAVE_WIDTH),
            bytes: b"unknown-target-device".to_vec(),
            digest: [9; 32],
            target_profile: observed(90_401),
        }
        .target_profile_record();
        assert!(unknown.contains("availability=unavailable;profile=unavailable"));
        assert!(unknown.ends_with("unavailable-reason=unknown-gfx-target-version"));

        for (vendor, wave, reason) in [
            (
                0,
                PRODUCTION_WAVE_WIDTH,
                ObservedGpuTargetProfileUnavailableReasonV1::VendorContradiction,
            ),
            (
                EXPECTED_AMD_VENDOR_ID,
                32,
                ObservedGpuTargetProfileUnavailableReasonV1::WaveWidthContradiction,
            ),
            (
                0,
                32,
                ObservedGpuTargetProfileUnavailableReasonV1::VendorAndWaveWidthContradiction,
            ),
        ] {
            assert_eq!(
                ObservedGpuTargetProfileRecordV1::from_direct_kfd_properties(
                    vendor,
                    GFX942_TARGET_VERSION,
                    wave,
                )
                .status,
                ObservedGpuTargetProfileStatusV1::Unavailable(reason)
            );
        }
    }

    #[test]
    fn topology_discovery_reobserves_target_profile_substitutions() {
        let fixture = TopologyFixture::new(
            7,
            &topology_properties(
                EXPECTED_AMD_VENDOR_ID,
                GFX942_TARGET_VERSION,
                PRODUCTION_WAVE_WIDTH,
            ),
        );
        let gfx942 = discover_devices(&fixture.root).expect("discover gfx942");
        assert_eq!(gfx942.len(), 1);
        assert!(gfx942[0].target_profile_record().contains(
            "gfx-target-version=90402;wave-width=64;availability=observed;profile=gfx942"
        ));

        fixture.replace(
            7,
            &topology_properties(EXPECTED_AMD_VENDOR_ID, 90_401, PRODUCTION_WAVE_WIDTH),
        );
        let unknown = discover_devices(&fixture.root).expect("discover unknown target");
        assert_ne!(gfx942, unknown);
        assert!(
            unknown[0]
                .target_profile_record()
                .ends_with("unavailable-reason=unknown-gfx-target-version")
        );

        fixture.replace(7, &topology_properties(0, GFX950_TARGET_VERSION, 32));
        let contradictory = discover_devices(&fixture.root).expect("discover contradiction");
        assert_ne!(unknown, contradictory);
        assert!(
            contradictory[0]
                .target_profile_record()
                .ends_with("unavailable-reason=vendor-and-wave-width-contradict-target")
        );
    }

    #[test]
    fn kir_compatibility_rejects_every_unavailable_kfd_profile() {
        for device in [
            profile_device(1, EXPECTED_AMD_VENDOR_ID, 90_401, PRODUCTION_WAVE_WIDTH),
            profile_device(2, 0, GFX942_TARGET_VERSION, PRODUCTION_WAVE_WIDTH),
            profile_device(3, EXPECTED_AMD_VENDOR_ID, GFX950_TARGET_VERSION, 32),
        ] {
            assert_eq!(
                kir_target_compatibility_v1(
                    &target_module(
                        AMDGPU_GFX942_XNACK_MINUS_TARGET_CAPABILITY_NAME,
                        WaveWidth::Wave64,
                    ),
                    &[device],
                ),
                KirTargetCompatibilityV1::Unavailable(
                    KirTargetUnavailableReasonV1::KfdProfileUnavailable
                )
            );
        }
    }

    #[test]
    fn kir_compatibility_rejects_empty_mixed_and_wave_mismatch() {
        let gfx942 = profile_device(
            1,
            EXPECTED_AMD_VENDOR_ID,
            GFX942_TARGET_VERSION,
            PRODUCTION_WAVE_WIDTH,
        );
        let gfx950 = profile_device(
            2,
            EXPECTED_AMD_VENDOR_ID,
            GFX950_TARGET_VERSION,
            PRODUCTION_WAVE_WIDTH,
        );
        let module = target_module(
            AMDGPU_GFX942_XNACK_MINUS_TARGET_CAPABILITY_NAME,
            WaveWidth::Wave64,
        );
        assert_eq!(
            kir_target_compatibility_v1(&module, &[]),
            KirTargetCompatibilityV1::Unavailable(
                KirTargetUnavailableReasonV1::KfdProfileUnavailable
            )
        );
        assert_eq!(
            kir_target_compatibility_v1(&module, &[gfx942.clone(), gfx950]),
            KirTargetCompatibilityV1::Unavailable(KirTargetUnavailableReasonV1::KfdFamilyMismatch)
        );
        assert_eq!(
            kir_target_compatibility_v1(
                &target_module(
                    AMDGPU_GFX942_XNACK_MINUS_TARGET_CAPABILITY_NAME,
                    WaveWidth::Wave32,
                ),
                &[gfx942],
            ),
            KirTargetCompatibilityV1::Unavailable(
                KirTargetUnavailableReasonV1::ConflictingWaveWidth
            )
        );
    }

    #[test]
    fn kir_compatibility_is_ready_only_for_one_matching_family_and_wave64() {
        let devices = [profile_device(
            1,
            EXPECTED_AMD_VENDOR_ID,
            GFX942_TARGET_VERSION,
            PRODUCTION_WAVE_WIDTH,
        )];
        assert_eq!(
            kir_target_compatibility_v1(
                &target_module(
                    AMDGPU_GFX942_XNACK_MINUS_TARGET_CAPABILITY_NAME,
                    WaveWidth::Wave64,
                ),
                &devices,
            ),
            KirTargetCompatibilityV1::Ready(ObservedGpuTargetProfileV1::Gfx942)
        );
    }

    #[test]
    fn dispatch_import_readiness_prioritizes_kir_admission_before_hardware() {
        let legacy = KirBinding {
            digest: [0x11; 32],
            length: 1,
            wave_width: 64,
        };
        for (legacy_kir, expected) in [
            (None, "unavailable-missing-exact-canonical-kir-v7"),
            (
                Some(&legacy),
                "unavailable-legacy-kir-declaration-is-not-admitted-canonical-kir",
            ),
        ] {
            for devices_empty in [false, true] {
                let mut output = String::new();
                assert!(!render_dispatch_import_plan(
                    &mut output,
                    None,
                    legacy_kir,
                    devices_empty,
                ));
                let statuses = output
                    .lines()
                    .filter(|line| line.starts_with("next-import-status: "))
                    .collect::<Vec<_>>();
                assert_eq!(statuses.len(), 1);
                assert_eq!(statuses[0], format!("next-import-status: {expected}"));
                assert!(!output.contains("next-import-program:"));
                assert!(!output.contains("next-import-arg["));
            }
        }

        let fixture = TopologyFixture::new(
            7,
            &topology_properties(
                EXPECTED_AMD_VENDOR_ID,
                GFX942_TARGET_VERSION,
                PRODUCTION_WAVE_WIDTH,
            ),
        );
        let kir_path = fixture.root.join("readiness.kir");
        let owner = VerifiedCanonicalKernelIrV7::from_module(target_module(
            AMDGPU_GFX942_XNACK_MINUS_TARGET_CAPABILITY_NAME,
            WaveWidth::Wave64,
        ))
        .unwrap();
        fs::write(&kir_path, owner.canonical_bytes()).unwrap();

        let no_device_kir = admit_kir_v7(&kir_path, &[]).unwrap();
        let mut output = String::new();
        assert!(!render_dispatch_import_plan(
            &mut output,
            Some(&no_device_kir),
            None,
            true,
        ));
        assert_eq!(
            output
                .lines()
                .filter(|line| line.starts_with("next-import-status: "))
                .collect::<Vec<_>>(),
            ["next-import-status: unavailable-no-stable-direct-kfd-device-identity"]
        );
        assert!(!output.contains("next-import-program:"));
        assert!(!output.contains("next-import-arg["));

        let devices = [profile_device(
            7,
            EXPECTED_AMD_VENDOR_ID,
            GFX942_TARGET_VERSION,
            PRODUCTION_WAVE_WIDTH,
        )];
        let ready_kir = admit_kir_v7(&kir_path, &devices).unwrap();
        let mut output = String::new();
        assert!(render_dispatch_import_plan(
            &mut output,
            Some(&ready_kir),
            None,
            false,
        ));
        assert_eq!(
            output
                .lines()
                .filter(|line| line.starts_with("next-import-status: "))
                .collect::<Vec<_>>(),
            ["next-import-status: ready-after-collector-artifact-and-source-size-validation"]
        );
        assert!(output.contains("next-import-program: cargo-fe2o3-in-process"));
    }

    #[test]
    fn target_profile_record_is_canonical_bounded_and_authorized() {
        let mut device = DeviceIdentity {
            node: 7,
            hardware: test_hardware(
                7,
                EXPECTED_AMD_VENDOR_ID,
                GFX950_TARGET_VERSION,
                PRODUCTION_WAVE_WIDTH,
            ),
            bytes: b"stable-device".to_vec(),
            digest: [0x42; 32],
            target_profile: ObservedGpuTargetProfileRecordV1::from_direct_kfd_properties(
                EXPECTED_AMD_VENDOR_ID,
                GFX950_TARGET_VERSION,
                PRODUCTION_WAVE_WIDTH,
            ),
        };
        let record = device.target_profile_record();
        assert_eq!(
            record,
            format!(
                "schema=fe2o3-observed-gpu-target-profile-v1;origin=direct-kfd-properties;node=7;stable-device-identity={};vendor-id=4098;gfx-target-version=90500;wave-width=64;availability=observed;profile=gfx950;unavailable-reason=none",
                device.content_identity()
            )
        );
        assert!(record.len() <= MAX_OBSERVED_GPU_TARGET_PROFILE_RECORD_BYTES_V1);

        let digest = |device: &DeviceIdentity| {
            let mut hasher = Sha256::new();
            hash_device_bindings(&mut hasher, std::slice::from_ref(device));
            <[u8; 32]>::from(hasher.finalize())
        };
        let authorized = digest(&device);
        device.target_profile = ObservedGpuTargetProfileRecordV1::from_direct_kfd_properties(
            EXPECTED_AMD_VENDOR_ID,
            GFX942_TARGET_VERSION,
            PRODUCTION_WAVE_WIDTH,
        );
        assert_ne!(authorized, digest(&device));

        let mut rendered = String::new();
        render_target_profile_observations(&mut rendered, &[device]);
        assert!(rendered.starts_with("observed-gpu-target-profile-v1[0]: schema="));
    }

    #[test]
    fn relative_artifact_paths_are_closed() {
        for path in ["../escape", "/absolute", "a/../b", "", "a//b"] {
            assert!(validate_relative(path).is_err(), "{path}");
        }
        assert!(validate_relative("agent/capture_results.json").is_ok());
    }

    #[test]
    fn artifact_enumeration_bound_counts_directories_and_zero_byte_files() {
        let (output, custody) = test_custody("artifact-fanout");
        fs::create_dir(output.join("nested")).unwrap();
        fs::write(output.join("empty-root"), []).unwrap();
        fs::write(output.join("nested/empty-child"), []).unwrap();
        let artifacts = scan_artifacts_with_entry_limit(&custody, 1024, 4).unwrap();
        assert_eq!(artifacts.len(), 2);
        assert!(
            scan_artifacts_with_entry_limit(&custody, 1024, 3)
                .unwrap_err()
                .contains("global entry-count bound")
        );
        custody.cleanup().unwrap();
    }

    #[test]
    fn capture_retains_final_bytes_after_normal_leader_exit() {
        let payload = "x".repeat(64 * 1024);
        let mut child = Command::new("/bin/sh")
            .args([
                "-c",
                "printf %s \"$1\"; printf final-stderr >&2",
                "sh",
                &payload,
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .process_group(0)
            .spawn()
            .unwrap();
        let supervised =
            supervise(&mut child, Duration::from_secs(5), payload.len(), 1024).unwrap();
        assert_eq!(supervised.reason, StopReason::Exited);
        assert_eq!(supervised.stdout.bytes, payload.as_bytes());
        assert_eq!(supervised.stderr.bytes, b"final-stderr");
    }

    fn inspect_sigchld_for_test() -> libc::sigaction {
        let mut action = MaybeUninit::<libc::sigaction>::zeroed();
        // SAFETY: `action` is writable and a null action requests the current disposition.
        assert_eq!(
            unsafe { libc::sigaction(libc::SIGCHLD, std::ptr::null(), action.as_mut_ptr()) },
            0
        );
        // SAFETY: the successful query initialized `action`.
        unsafe { action.assume_init() }
    }

    fn install_sigchld_for_test(handler: libc::sighandler_t, flags: libc::c_int) {
        let mut action = MaybeUninit::<libc::sigaction>::zeroed();
        // SAFETY: the zeroed action is fully initialized below before installation.
        let action = unsafe {
            let pointer = action.as_mut_ptr();
            (*pointer).sa_sigaction = handler;
            (*pointer).sa_flags = flags;
            assert_eq!(libc::sigemptyset(&mut (*pointer).sa_mask), 0);
            action.assume_init()
        };
        // SAFETY: `action` is fully initialized and remains live for the call.
        assert_eq!(
            unsafe { libc::sigaction(libc::SIGCHLD, &action, std::ptr::null_mut()) },
            0
        );
    }

    fn assert_sigchld_action_restored(restored: &libc::sigaction, expected: &libc::sigaction) {
        // The libc sigaction wrapper may synthesize Linux's private SA_RESTORER bit when an
        // otherwise identical saved action is reinstalled. Compare every caller-controlled bit.
        const LINUX_SA_RESTORER: libc::c_int = 0x0400_0000;
        assert_eq!(restored.sa_sigaction, expected.sa_sigaction);
        assert_eq!(
            restored.sa_flags & !LINUX_SA_RESTORER,
            expected.sa_flags & !LINUX_SA_RESTORER
        );
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    struct TestProcessIdentityV1 {
        pid: i32,
        start_time: u64,
        process_group: i32,
        session: i32,
    }

    #[derive(Debug)]
    struct TestProcessStatV1 {
        state: char,
        process_group: i32,
        session: i32,
        start_time: u64,
    }

    struct PinnedTestProcessV1 {
        identity: TestProcessIdentityV1,
        descriptor: rustix::fd::OwnedFd,
    }

    impl Drop for PinnedTestProcessV1 {
        fn drop(&mut self) {
            let _ =
                rustix::process::pidfd_send_signal(&self.descriptor, rustix::process::Signal::KILL);
        }
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    struct TestProcessPublicationV1 {
        leader: TestProcessIdentityV1,
        descendant: TestProcessIdentityV1,
    }

    #[derive(Clone, Copy)]
    enum ExpectedTestTopologyV1 {
        CollectorGroup { expected_session: i32 },
        EscapedSession { expected_leader_session: i32 },
    }

    fn parse_test_process_identity_fields_v1(
        fields: &[&str],
    ) -> Result<TestProcessIdentityV1, String> {
        if fields.len() != 4 {
            return Err("unexpected test process identity width".to_owned());
        }
        Ok(TestProcessIdentityV1 {
            pid: fields[0]
                .parse()
                .map_err(|_| "invalid published test PID".to_owned())?,
            start_time: fields[1]
                .parse()
                .map_err(|_| "invalid published test start time".to_owned())?,
            process_group: fields[2]
                .parse()
                .map_err(|_| "invalid published test process group".to_owned())?,
            session: fields[3]
                .parse()
                .map_err(|_| "invalid published test session".to_owned())?,
        })
    }

    fn parse_test_process_publication_v1(source: &str) -> Result<TestProcessPublicationV1, String> {
        let fields: Vec<_> = source.split_whitespace().collect();
        if fields.len() != 8 {
            return Err("unexpected test process publication width".to_owned());
        }
        Ok(TestProcessPublicationV1 {
            leader: parse_test_process_identity_fields_v1(&fields[..4])?,
            descendant: parse_test_process_identity_fields_v1(&fields[4..])?,
        })
    }

    fn read_test_process_stat_v1(pid: i32) -> Result<Option<TestProcessStatV1>, String> {
        let source = match fs::read_to_string(format!("/proc/{pid}/stat")) {
            Ok(source) => source,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(format!("failed to read test process {pid} stat: {error}")),
        };
        let (identity, fields) = source
            .trim_end()
            .rsplit_once(") ")
            .ok_or_else(|| format!("test process {pid} stat has no final command delimiter"))?;
        let observed_pid = identity
            .split_once(" (")
            .ok_or_else(|| format!("test process {pid} stat has no command prefix"))?
            .0
            .parse::<i32>()
            .map_err(|_| format!("test process {pid} stat has an invalid PID"))?;
        if observed_pid != pid {
            return Err(format!(
                "test process {pid} stat reported PID {observed_pid}"
            ));
        }
        let fields: Vec<_> = fields.split_whitespace().collect();
        if fields.len() < 20 {
            return Err(format!("test process {pid} stat is truncated"));
        }
        let parse_i32 = |index: usize, label: &str| {
            fields[index]
                .parse::<i32>()
                .map_err(|_| format!("test process {pid} stat has an invalid {label}"))
        };
        Ok(Some(TestProcessStatV1 {
            state: fields[0]
                .parse::<char>()
                .map_err(|_| format!("test process {pid} stat has an invalid state"))?,
            process_group: parse_i32(2, "process group")?,
            session: parse_i32(3, "session")?,
            start_time: fields[19]
                .parse::<u64>()
                .map_err(|_| format!("test process {pid} stat has an invalid start time"))?,
        }))
    }

    fn test_process_is_terminal_v1(stat: &TestProcessStatV1) -> bool {
        matches!(stat.state, 'Z' | 'X' | 'x')
    }

    fn validate_live_test_process_v1(
        identity: TestProcessIdentityV1,
    ) -> Result<TestProcessStatV1, String> {
        let Some(before) = read_test_process_stat_v1(identity.pid)? else {
            return Err(format!(
                "test process {} disappeared before pinning",
                identity.pid
            ));
        };
        if before.start_time != identity.start_time
            || before.process_group != identity.process_group
            || before.session != identity.session
        {
            return Err(format!(
                "test process {} changed identity before pinning: {before:?}",
                identity.pid
            ));
        }
        if test_process_is_terminal_v1(&before) {
            return Err(format!(
                "test process {} was already terminal before pinning: {before:?}",
                identity.pid
            ));
        }
        Ok(before)
    }

    fn pin_live_test_process_v1(
        identity: TestProcessIdentityV1,
    ) -> Result<PinnedTestProcessV1, String> {
        validate_live_test_process_v1(identity)?;
        let pid = rustix::process::Pid::from_raw(identity.pid)
            .ok_or_else(|| format!("invalid test process PID {}", identity.pid))?;
        let descriptor = rustix::process::pidfd_open(pid, rustix::process::PidfdFlags::empty())
            .map_err(|error| {
                format!("failed to pin live test process {}: {error}", identity.pid)
            })?;
        validate_live_test_process_v1(identity)?;
        Ok(PinnedTestProcessV1 {
            identity,
            descriptor,
        })
    }

    fn current_test_session_v1() -> i32 {
        // SAFETY: getsid with zero reads the calling process's session without mutation.
        let session = unsafe { libc::getsid(0) };
        assert!(session > 0, "failed to read the test process session");
        session
    }

    fn pin_published_test_descendant_v1(
        publication: TestProcessPublicationV1,
        expected: ExpectedTestTopologyV1,
    ) -> Result<PinnedTestProcessV1, String> {
        let leader = validate_live_test_process_v1(publication.leader)?;
        if publication.leader.process_group != publication.leader.pid {
            return Err(format!(
                "collector leader did not own its fresh process group: {publication:?}"
            ));
        }
        let process = pin_live_test_process_v1(publication.descendant)?;
        let valid = match expected {
            ExpectedTestTopologyV1::CollectorGroup { expected_session } => {
                publication.leader.session == expected_session
                    && publication.descendant.process_group == publication.leader.pid
                    && publication.descendant.session == expected_session
                    && publication.descendant.pid != publication.leader.pid
            }
            ExpectedTestTopologyV1::EscapedSession {
                expected_leader_session,
            } => {
                publication.leader.session == expected_leader_session
                    && publication.descendant.pid == publication.descendant.process_group
                    && publication.descendant.pid == publication.descendant.session
                    && publication.descendant.session != publication.leader.session
            }
        };
        if !valid {
            return Err(format!(
                "published process topology did not match the fixture contract: publication={publication:?} leader={leader:?}"
            ));
        }
        Ok(process)
    }

    fn monitor_test_process_publication_v1(
        publication_path: PathBuf,
        release_path: PathBuf,
        expected: ExpectedTestTopologyV1,
    ) -> thread::JoinHandle<Result<PinnedTestProcessV1, String>> {
        thread::Builder::new()
            .name("fe2o3-test-descendant-pin-v1".to_owned())
            .spawn(move || {
                let deadline = Instant::now()
                    .checked_add(Duration::from_secs(2))
                    .ok_or("test publication deadline overflow".to_owned())?;
                let result = loop {
                    match fs::read_to_string(&publication_path) {
                        Ok(source) => {
                            break parse_test_process_publication_v1(&source).and_then(
                                |publication| {
                                    pin_published_test_descendant_v1(publication, expected)
                                },
                            );
                        }
                        Err(error) if error.kind() == io::ErrorKind::NotFound => {
                            if Instant::now() >= deadline {
                                break Err(format!(
                                    "test process publication {} was not created",
                                    publication_path.display()
                                ));
                            }
                            thread::sleep(Duration::from_millis(5));
                        }
                        Err(error) => {
                            break Err(format!(
                                "failed to read test process publication {}: {error}",
                                publication_path.display()
                            ));
                        }
                    }
                };
                match result {
                    Ok(process) => {
                        fs::write(&release_path, b"release").map_err(|error| {
                            format!(
                                "failed to release test collector through {}: {error}",
                                release_path.display()
                            )
                        })?;
                        Ok(process)
                    }
                    Err(error) => Err(error),
                }
            })
            .unwrap()
    }

    fn join_test_process_monitor_v1(
        monitor: thread::JoinHandle<Result<PinnedTestProcessV1, String>>,
    ) -> PinnedTestProcessV1 {
        monitor
            .join()
            .expect("test process monitor panicked")
            .expect("test process publication failed validation")
    }

    fn wait_for_test_process_terminal_v1(
        process: &PinnedTestProcessV1,
        timeout: Duration,
    ) -> Result<bool, String> {
        let deadline = Instant::now()
            .checked_add(timeout)
            .ok_or("test process wait deadline overflow")?;
        loop {
            let now = Instant::now();
            if now >= deadline {
                return Ok(false);
            }
            let remaining = deadline.duration_since(now);
            let milliseconds = remaining
                .as_millis()
                .saturating_add(u128::from(remaining.subsec_nanos() % 1_000_000 != 0))
                .min(i32::MAX as u128) as i32;
            let mut poll = libc::pollfd {
                fd: process.descriptor.as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            };
            // SAFETY: `poll` points to one initialized pollfd and the timeout is bounded above.
            let result = unsafe { libc::poll(&mut poll, 1, milliseconds) };
            if result > 0 {
                let terminal = libc::POLLIN | libc::POLLHUP;
                let rejected = libc::POLLERR | libc::POLLNVAL;
                if poll.revents & rejected == 0
                    && poll.revents != 0
                    && poll.revents & !terminal == 0
                {
                    return Ok(true);
                }
                return Err(format!(
                    "test process {} pidfd returned unexpected poll events {:#x}",
                    process.identity.pid, poll.revents
                ));
            }
            if result == 0 {
                return Ok(false);
            }
            let error = io::Error::last_os_error();
            if error.kind() != io::ErrorKind::Interrupted {
                return Err(format!(
                    "failed to poll test process {} pidfd: {error}",
                    process.identity.pid
                ));
            }
        }
    }

    fn assert_test_descendant_is_terminated(process: &PinnedTestProcessV1) {
        if wait_for_test_process_terminal_v1(process, Duration::from_secs(1)).unwrap() {
            return;
        }
        let state = read_test_process_stat_v1(process.identity.pid).unwrap();
        let cleanup =
            rustix::process::pidfd_send_signal(&process.descriptor, rustix::process::Signal::KILL);
        panic!(
            "live collector descendant survived process-group revoke: identity={:?} state={state:?} cleanup={cleanup:?}",
            process.identity
        );
    }

    fn terminate_escaped_test_process_v1(process: &PinnedTestProcessV1) {
        rustix::process::pidfd_send_signal(&process.descriptor, rustix::process::Signal::KILL)
            .unwrap();
        assert!(
            wait_for_test_process_terminal_v1(process, Duration::from_secs(1)).unwrap(),
            "escaped test process did not reach a terminal state: {:?}",
            process.identity
        );
    }

    fn capture_worker_spawn_failure_collection_case(stream: CaptureStreamV1, mode: &str) {
        let id = NEXT_TOPOLOGY_FIXTURE.fetch_add(1, AtomicOrdering::Relaxed);
        let root = env::temp_dir().join(format!(
            "cargo-fe2o3-profile-capture-spawn-failure-{mode}-{}-{id}",
            std::process::id()
        ));
        fs::create_dir(&root).unwrap();
        let descendant_file = root.join(format!("{}-descendant.pid", stream.name()));
        let descendant_literal = serde_json::to_string(&descendant_file.to_string_lossy()).unwrap();
        let pin_gate = root.join(format!("{}-descendant.pinned", stream.name()));
        let tool = root.join("rocprofv3");
        fs::write(
            &tool,
            format!(
                "#!/usr/bin/env python3\n# --kernel-trace --advanced-thread-trace\nimport os,subprocess,time\ndef identity(pid):\n stat=open(f'/proc/{{pid}}/stat',encoding='utf-8').read().rsplit(') ',1)[1].split()\n return f'{{pid}} {{stat[19]}} {{stat[2]}} {{stat[3]}}'\nchild=subprocess.Popen(['/bin/sleep','30'])\npublication=identity(os.getpid())+' '+identity(child.pid)\ntemporary={descendant_literal}+'.tmp'\nwith open(temporary,'w',encoding='utf-8') as stream:\n stream.write(publication)\nos.replace(temporary,{descendant_literal})\nwhile True:\n time.sleep(1)\n"
            ),
        )
        .unwrap();
        fs::set_permissions(&tool, fs::Permissions::from_mode(0o700)).unwrap();
        let output = root.join("capture");
        let python = discover_python().expect("test requires the reviewed native Python");
        let args = [
            "--kind".to_owned(),
            "dispatch-json".to_owned(),
            "--tool".to_owned(),
            tool.to_string_lossy().into_owned(),
            "--python".to_owned(),
            python.to_string_lossy().into_owned(),
            "--output-dir".to_owned(),
            output.to_string_lossy().into_owned(),
            "--".to_owned(),
            "/bin/true".to_owned(),
        ];
        let plan = prepare_plan(parse_options(&args).unwrap()).unwrap();
        let monitor = monitor_test_process_publication_v1(
            descendant_file.clone(),
            pin_gate.clone(),
            ExpectedTestTopologyV1::CollectorGroup {
                expected_session: current_test_session_v1(),
            },
        );
        let active_workers = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        CAPTURE_THREAD_SPAWN_FAILURE_INJECTION_V1.with(|current| {
            assert!(
                current
                    .replace(Some(CaptureThreadSpawnFailureInjectionV1 {
                        stream,
                        collector_ready: pin_gate,
                        active_workers: Arc::clone(&active_workers),
                    }))
                    .is_none(),
                "nested capture worker spawn failure injection"
            );
        });
        let result = collect_with_device_revalidator(plan, |_| Ok(()));
        CAPTURE_THREAD_SPAWN_FAILURE_INJECTION_V1.with(|current| {
            assert!(current.replace(None).is_some());
        });
        let descendant = join_test_process_monitor_v1(monitor);
        let error = result.unwrap_err();
        assert!(
            error.contains(&format!(
                "failed to spawn collector {} capture worker",
                stream.name()
            )),
            "{error}"
        );
        assert_eq!(active_workers.load(AtomicOrdering::SeqCst), 0);
        assert!(
            !output.exists(),
            "capture worker setup failure left publication custody behind"
        );
        assert_test_descendant_is_terminated(&descendant);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn capture_worker_spawn_failures_revoke_join_and_prevent_publication() {
        const MODE: &str = "FE2O3_CAPTURE_WORKER_SPAWN_FAILURE_TEST_V1";
        if env::var_os(MODE).is_some() {
            let inherited = inspect_sigchld_for_test();
            for stream in [CaptureStreamV1::Stdout, CaptureStreamV1::Stderr] {
                capture_worker_spawn_failure_collection_case(stream, "default");
                let restored = inspect_sigchld_for_test();
                assert_sigchld_action_restored(&restored, &inherited);
            }
            return;
        }
        let output = Command::new(env::current_exe().unwrap())
            .args([
                "--exact",
                "profile_command::tests::capture_worker_spawn_failures_revoke_join_and_prevent_publication",
                "--nocapture",
            ])
            .env(MODE, "1")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "capture worker spawn failure child failed:\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fn hostile_sigchld_child_case(mode: &str) {
        let (handler, flags) = match mode {
            "ignore" => (libc::SIG_IGN, 0),
            "no-cldwait" => (libc::SIG_DFL, libc::SA_NOCLDWAIT),
            _ => panic!("unknown hostile SIGCHLD test mode"),
        };
        if mode == "no-cldwait" {
            install_sigchld_for_test(handler, flags);
        }
        let hostile = inspect_sigchld_for_test();
        assert_eq!(hostile.sa_sigaction, handler);
        assert_eq!(hostile.sa_flags & libc::SA_NOCLDWAIT, flags);

        let mut missing = Command::new("/fe2o3-test-missing-collector");
        let error = match spawn_and_supervise_collector_v1(
            &mut missing,
            Duration::from_secs(5),
            1024,
            1024,
        ) {
            Ok(_) => panic!("missing collector unexpectedly spawned"),
            Err(error) => error,
        };
        assert!(error.contains("failed to spawn pinned rocprofv3 collector"));
        let restored = inspect_sigchld_for_test();
        assert_sigchld_action_restored(&restored, &hostile);

        let id = NEXT_TOPOLOGY_FIXTURE.fetch_add(1, AtomicOrdering::Relaxed);
        let normal_root = env::temp_dir().join(format!(
            "cargo-fe2o3-profile-sigchld-normal-{mode}-{}-{id}",
            std::process::id()
        ));
        fs::create_dir(&normal_root).unwrap();
        let normal_publication = normal_root.join("descendant.identity");
        let normal_release = normal_root.join("descendant.release");
        let normal_monitor = monitor_test_process_publication_v1(
            normal_publication.clone(),
            normal_release.clone(),
            ExpectedTestTopologyV1::CollectorGroup {
                expected_session: current_test_session_v1(),
            },
        );
        let mut normal = Command::new("/usr/bin/python3");
        normal
            .args([
                "-c",
                "import os,subprocess,sys,time\ndef identity(pid):\n stat=open(f'/proc/{pid}/stat',encoding='utf-8').read().rsplit(') ',1)[1].split()\n return f'{pid} {stat[19]} {stat[2]} {stat[3]}'\nchild=subprocess.Popen(['/bin/sleep','30'])\ntemporary=sys.argv[1]+'.tmp'\nwith open(temporary,'w',encoding='utf-8') as stream:\n stream.write(identity(os.getpid())+' '+identity(child.pid))\nos.replace(temporary,sys.argv[1])\ndeadline=time.monotonic()+5\nwhile not os.path.exists(sys.argv[2]) and time.monotonic()<deadline:\n time.sleep(0.005)\nif not os.path.exists(sys.argv[2]):\n raise SystemExit(98)\nos._exit(0)\n",
                normal_publication.to_str().unwrap(),
                normal_release.to_str().unwrap(),
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .process_group(0);
        let supervised =
            spawn_and_supervise_collector_v1(&mut normal, Duration::from_secs(5), 1024, 1024)
                .unwrap();
        let normal_descendant = join_test_process_monitor_v1(normal_monitor);
        assert_eq!(supervised.reason, StopReason::Exited);
        assert!(supervised.status.is_some_and(|status| status.success()));
        assert_test_descendant_is_terminated(&normal_descendant);
        let restored = inspect_sigchld_for_test();
        assert_sigchld_action_restored(&restored, &hostile);
        fs::remove_dir_all(normal_root).unwrap();

        let id = NEXT_TOPOLOGY_FIXTURE.fetch_add(1, AtomicOrdering::Relaxed);
        let overflow_root = env::temp_dir().join(format!(
            "cargo-fe2o3-profile-sigchld-overflow-{mode}-{}-{id}",
            std::process::id()
        ));
        fs::create_dir(&overflow_root).unwrap();
        let overflow_publication = overflow_root.join("descendant.identity");
        let overflow_release = overflow_root.join("descendant.release");
        let overflow_monitor = monitor_test_process_publication_v1(
            overflow_publication.clone(),
            overflow_release.clone(),
            ExpectedTestTopologyV1::CollectorGroup {
                expected_session: current_test_session_v1(),
            },
        );
        let mut overflow = Command::new("/usr/bin/python3");
        overflow
            .args([
                "-c",
                "import os,subprocess,sys,time\ndef identity(pid):\n stat=open(f'/proc/{pid}/stat',encoding='utf-8').read().rsplit(') ',1)[1].split()\n return f'{pid} {stat[19]} {stat[2]} {stat[3]}'\nchild=subprocess.Popen(['/bin/sleep','30'])\ntemporary=sys.argv[1]+'.tmp'\nwith open(temporary,'w',encoding='utf-8') as stream:\n stream.write(identity(os.getpid())+' '+identity(child.pid))\nos.replace(temporary,sys.argv[1])\ndeadline=time.monotonic()+5\nwhile not os.path.exists(sys.argv[2]) and time.monotonic()<deadline:\n time.sleep(0.005)\nif not os.path.exists(sys.argv[2]):\n raise SystemExit(98)\nos.write(1,b'x'*65536)\nos._exit(0)\n",
                overflow_publication.to_str().unwrap(),
                overflow_release.to_str().unwrap(),
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .process_group(0);
        let supervised =
            spawn_and_supervise_collector_v1(&mut overflow, Duration::from_secs(5), 1024, 1024)
                .unwrap();
        let overflow_descendant = join_test_process_monitor_v1(overflow_monitor);
        assert_eq!(supervised.reason, StopReason::OutputOverflow);
        assert!(supervised.stdout.overflow);
        assert_eq!(supervised.stdout.bytes.len(), 1024);
        assert_test_descendant_is_terminated(&overflow_descendant);
        let restored = inspect_sigchld_for_test();
        assert_sigchld_action_restored(&restored, &hostile);
        fs::remove_dir_all(overflow_root).unwrap();

        for stream in [CaptureStreamV1::Stdout, CaptureStreamV1::Stderr] {
            capture_worker_spawn_failure_collection_case(stream, mode);
            let restored = inspect_sigchld_for_test();
            assert_sigchld_action_restored(&restored, &hostile);
        }
    }

    #[test]
    fn inherited_sigchld_dispositions_preserve_collector_ownership() {
        const MODE: &str = "FE2O3_SIGCHLD_HOSTILE_TEST_V1";
        if let Ok(mode) = env::var(MODE) {
            hostile_sigchld_child_case(&mode);
            return;
        }
        for mode in ["ignore", "no-cldwait"] {
            let mut command = Command::new(env::current_exe().unwrap());
            command
                .args([
                    "--exact",
                    "profile_command::tests::inherited_sigchld_dispositions_preserve_collector_ownership",
                    "--nocapture",
                ])
                .env(MODE, mode);
            if mode == "ignore" {
                // SAFETY: the closure performs only async-signal-safe sigemptyset/sigaction calls
                // between fork and exec, without allocation or shared-state access. SIG_IGN is
                // preserved across exec; SA_NOCLDWAIT is installed inside its isolated test.
                unsafe {
                    command.pre_exec(|| {
                        let mut action = std::mem::zeroed::<libc::sigaction>();
                        action.sa_sigaction = libc::SIG_IGN;
                        if libc::sigemptyset(&mut action.sa_mask) != 0
                            || libc::sigaction(libc::SIGCHLD, &action, std::ptr::null_mut()) != 0
                        {
                            return Err(io::Error::last_os_error());
                        }
                        Ok(())
                    });
                }
            }
            let output = command.output().unwrap();
            assert!(
                output.status.success(),
                "hostile SIGCHLD mode {mode} failed:\nstdout:\n{}\nstderr:\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        }
    }

    #[test]
    fn owned_child_is_revoked_before_exactly_one_reap() {
        #[derive(Default)]
        struct InstrumentedChild {
            events: Vec<&'static str>,
        }

        let mut child = InstrumentedChild::default();
        let (status, reason, wait_error) = finalize_collector_exit_with(
            &mut child,
            CollectorExitDecision::RevokeAndReap(StopReason::Exited),
            |child| child.events.push("revoke"),
            |child| {
                assert_eq!(child.events.as_slice(), ["revoke"]);
                child.events.push("reap");
                Ok(())
            },
        );
        assert_eq!(child.events, ["revoke", "reap"]);
        assert_eq!(status, Some(()));
        assert_eq!(reason, StopReason::Exited);
        assert_eq!(wait_error, None);

        let mut wait_failure = InstrumentedChild::default();
        let (status, reason, wait_error) = finalize_collector_exit_with(
            &mut wait_failure,
            CollectorExitDecision::RevokeAndReapAfterWaitFailure("EIO".to_owned()),
            |child| child.events.push("revoke"),
            |child| {
                assert_eq!(child.events.as_slice(), ["revoke"]);
                child.events.push("reap");
                Ok(())
            },
        );
        assert_eq!(wait_failure.events, ["revoke", "reap"]);
        assert_eq!(status, Some(()));
        assert_eq!(reason, StopReason::WaitFailure);
        assert_eq!(wait_error.as_deref(), Some("EIO"));

        let mut setup_failure = InstrumentedChild::default();
        let error = finalize_capture_setup_failure_with(
            &mut setup_failure,
            "capture setup failed".to_owned(),
            Ok(()),
            |child| child.events.push("revoke"),
            |child| {
                assert_eq!(child.events.as_slice(), ["revoke"]);
                child.events.push("reap");
                Ok(())
            },
        );
        assert_eq!(setup_failure.events, ["revoke", "reap"]);
        assert_eq!(error, "capture setup failed");

        let mut lost_setup_ownership = InstrumentedChild::default();
        let error = finalize_capture_setup_failure_with(
            &mut lost_setup_ownership,
            "capture setup failed".to_owned(),
            Err("SIGCHLD changed".to_owned()),
            |child| child.events.push("revoke"),
            |child| {
                child.events.push("reap");
                Ok(())
            },
        );
        assert!(lost_setup_ownership.events.is_empty());
        assert!(error.contains("numeric identity was not signaled"));

        let mut ambiguous = InstrumentedChild::default();
        let (status, reason, wait_error) = finalize_collector_exit_with(
            &mut ambiguous,
            CollectorExitDecision::AmbiguousWait("ECHILD".to_owned()),
            |child| child.events.push("revoke"),
            |child| {
                child.events.push("reap");
                Ok(())
            },
        );
        assert!(ambiguous.events.is_empty());
        assert_eq!(status, None);
        assert_eq!(reason, StopReason::WaitFailure);
        assert_eq!(wait_error.as_deref(), Some("ECHILD"));
    }

    #[test]
    fn fast_exit_stdout_overflow_is_authoritative() {
        let mut child = Command::new("/bin/sh")
            .args(["-c", "head -c 65536 /dev/zero"])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .process_group(0)
            .spawn()
            .unwrap();
        let supervised = supervise(&mut child, Duration::from_secs(5), 1024, 1024).unwrap();
        assert_eq!(supervised.reason, StopReason::OutputOverflow);
        assert!(supervised.stdout.overflow);
        assert_eq!(supervised.stdout.bytes.len(), 1024);
        assert!(!supervised.stderr.overflow);
        assert!(supervised.stderr.bytes.len() <= 1024);
    }

    #[test]
    fn fast_exit_stderr_overflow_is_authoritative() {
        let mut child = Command::new("/bin/sh")
            .args(["-c", "head -c 65536 /dev/zero >&2"])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .process_group(0)
            .spawn()
            .unwrap();
        let supervised = supervise(&mut child, Duration::from_secs(5), 1024, 1024).unwrap();
        assert_eq!(supervised.reason, StopReason::OutputOverflow);
        assert!(!supervised.stdout.overflow);
        assert!(supervised.stdout.bytes.len() <= 1024);
        assert!(supervised.stderr.overflow);
        assert_eq!(supervised.stderr.bytes.len(), 1024);
    }

    #[test]
    fn escaped_pipe_holder_cannot_block_capture_completion() {
        let id = NEXT_TOPOLOGY_FIXTURE.fetch_add(1, AtomicOrdering::Relaxed);
        let root = env::temp_dir().join(format!(
            "cargo-fe2o3-profile-escaped-pipe-{}-{id}",
            std::process::id()
        ));
        fs::create_dir(&root).unwrap();
        let publication = root.join("descendant.identity");
        let release = root.join("descendant.release");
        let monitor = monitor_test_process_publication_v1(
            publication.clone(),
            release.clone(),
            ExpectedTestTopologyV1::EscapedSession {
                expected_leader_session: current_test_session_v1(),
            },
        );
        let python_path = discover_python().expect("test requires the reviewed native Python");
        let python = FilePin::open(
            &python_path,
            "test Python interpreter",
            MAX_INTERPRETER_BYTES,
            true,
        )
        .unwrap();
        let mut child = Command::new(python.execution_path())
            .arg0(&python.canonical_path)
            .args([
                "-c",
                "import os,sys,time\ndef identity(pid):\n stat=open(f'/proc/{pid}/stat',encoding='utf-8').read().rsplit(') ',1)[1].split()\n return f'{pid} {stat[19]} {stat[2]} {stat[3]}'\npid=os.fork()\nif pid == 0:\n os.setsid()\n temporary=sys.argv[1]+'.tmp'\n with open(temporary,'w',encoding='utf-8') as stream:\n  stream.write(identity(os.getppid())+' '+identity(os.getpid()))\n os.replace(temporary,sys.argv[1])\n deadline=time.monotonic()+5\n while not os.path.exists(sys.argv[2]) and time.monotonic()<deadline:\n  time.sleep(0.005)\n if not os.path.exists(sys.argv[2]):\n  os._exit(98)\n time.sleep(30)\n os._exit(0)\ndeadline=time.monotonic()+5\nwhile not os.path.exists(sys.argv[2]) and time.monotonic()<deadline:\n time.sleep(0.005)\nif not os.path.exists(sys.argv[2]):\n os._exit(98)\nos._exit(0)\n",
                publication.to_str().unwrap(),
                release.to_str().unwrap(),
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .process_group(0)
            .spawn()
            .unwrap();
        let started = Instant::now();
        let supervised = supervise(&mut child, Duration::from_secs(5), 1024, 1024).unwrap();
        let elapsed = started.elapsed();
        let escaped = join_test_process_monitor_v1(monitor);
        terminate_escaped_test_process_v1(&escaped);
        assert_eq!(supervised.reason, StopReason::Exited);
        assert!(
            elapsed < Duration::from_secs(4),
            "capture waited for escaped pipe holder for {elapsed:?}"
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn ownership_guard_fifo_substitution_is_rejected_without_blocking() {
        let (output, custody) = test_custody("guard-fifo");
        let guard = output.join(OWNERSHIP_FILE);
        replace_with_fifo(&guard);
        let writer = DelayedFifoWriter::start(guard.clone());
        let started = Instant::now();
        let result = custody.validate();
        assert_fifo_rejection_is_bounded(started, result, writer);
        fs::remove_file(guard).unwrap();
        fs::remove_dir_all(output).unwrap();
    }

    #[test]
    fn durable_record_fifo_substitution_is_rejected_without_blocking() {
        let (output, custody) = test_custody("record-fifo");
        custody
            .commit_record("record.bin", ".record.redo", b"record", 64)
            .unwrap();
        let record = output.join("record.bin");
        replace_with_fifo(&record);
        let writer = DelayedFifoWriter::start(record);
        let started = Instant::now();
        let result = custody.read_record("record.bin", 64).map(|_| ());
        assert_fifo_rejection_is_bounded(started, result, writer);
        custody.cleanup().unwrap();
    }

    #[test]
    fn retained_source_fifo_substitution_is_rejected_without_blocking() {
        let (output, custody) = test_custody("source-fifo");
        let source_path = output.join("dispatch.json");
        let bytes = b"{}\n";
        fs::write(&source_path, bytes).unwrap();
        let metadata = fs::symlink_metadata(&source_path).unwrap();
        let artifact = Artifact {
            relative: "dispatch.json".to_owned(),
            length: metadata.len(),
            digest: Sha256::digest(bytes).into(),
            identity: ObjectIdentity::from_metadata(&metadata),
        };
        let retained = RetainedDispatchSourceV1::open(&custody, &artifact).unwrap();
        replace_with_fifo(&source_path);
        let writer = DelayedFifoWriter::start(source_path);
        let started = Instant::now();
        let result = retained.revalidate(&custody);
        assert_fifo_rejection_is_bounded(started, result, writer);
        custody.cleanup().unwrap();
    }

    #[test]
    fn artifact_final_reopen_fifo_substitution_is_rejected_without_blocking() {
        let (output, custody) = test_custody("artifact-fifo");
        let artifact_path = output.join("artifact.bin");
        fs::write(&artifact_path, b"artifact").unwrap();
        let metadata = fs::symlink_metadata(&artifact_path).unwrap();
        let writer = std::cell::RefCell::new(None);
        let started = Instant::now();
        let result = hash_file_with_reopen_hook(&custody, "artifact.bin", &metadata, || {
            replace_with_fifo(&artifact_path);
            writer.replace(Some(DelayedFifoWriter::start(artifact_path.clone())));
        })
        .map(|_| ());
        let writer = writer.into_inner().expect("reopen hook ran");
        assert_fifo_rejection_is_bounded(started, result, writer);
        custody.cleanup().unwrap();
    }

    #[test]
    fn semantic_configuration_identity_binds_measurement_kind_and_target_records() {
        assert_eq!(
            canonical_configuration(ProfileKind::DispatchJson, &[]),
            canonical_configuration(ProfileKind::DispatchJson, &[])
        );
        assert_ne!(
            canonical_configuration(ProfileKind::DispatchJson, &[]),
            canonical_configuration(ProfileKind::DispatchCsv, &[])
        );
        assert_ne!(
            canonical_configuration(ProfileKind::DispatchJson, &[]),
            canonical_configuration(ProfileKind::Att, &[])
        );
        let device = |target| DeviceIdentity {
            node: 1,
            hardware: test_hardware(1, EXPECTED_AMD_VENDOR_ID, target, PRODUCTION_WAVE_WIDTH),
            bytes: b"stable-device".to_vec(),
            digest: [1; 32],
            target_profile: ObservedGpuTargetProfileRecordV1::from_direct_kfd_properties(
                EXPECTED_AMD_VENDOR_ID,
                target,
                PRODUCTION_WAVE_WIDTH,
            ),
        };
        assert_ne!(
            canonical_configuration(ProfileKind::DispatchJson, &[device(GFX942_TARGET_VERSION)]),
            canonical_configuration(ProfileKind::DispatchJson, &[device(GFX950_TARGET_VERSION)])
        );
    }
}
