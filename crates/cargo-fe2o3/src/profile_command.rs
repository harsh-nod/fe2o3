//! Bounded, explicitly authorized rocprofv3 orchestration.
//!
//! Planning opens and measures every executable input, the fixed collector configuration,
//! inherited environment, and direct-KFD topology records. It does not execute rocprofv3 or the
//! target. Collection requires the exact plan digest printed by that dry run. The presence of a
//! rocprofv3 option, a successful collector exit, or a file with a familiar suffix is not treated
//! as proof that a direct-KFD dispatch or ATT record was observed; Bundle V4 import is the separate
//! validation boundary.

use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::ffi::{OsStr, OsString};
use std::fmt::Write as _;
use std::fs::{self, File, Metadata, OpenOptions};
use std::io::{self, Read, Write};
use std::os::fd::AsRawFd;
use std::os::unix::fs::{MetadataExt as _, OpenOptionsExt as _, PermissionsExt as _};
use std::os::unix::process::CommandExt as _;
use std::path::{Component, Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
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
const MAX_DEVICES: usize = 64;
const MAX_ARTIFACTS: usize = 4096;
const MAX_ARTIFACT_DEPTH: usize = 8;
const MAX_PROFILER_IMPORT_SOURCE_BYTES: u64 = 8 * 1024 * 1024;
const MAX_ARGUMENTS: usize = 256;
const MAX_ARGUMENT_BYTES: usize = 4096;
const MAX_OBSERVED_GPU_TARGET_PROFILE_RECORD_BYTES_V1: usize = 512;
const POLL_INTERVAL: Duration = Duration::from_millis(5);
const OWNERSHIP_FILE: &str = ".fe2o3-profile-owned-v1";
const MANIFEST_FILE: &str = "fe2o3-profile-manifest-v1.txt";
const KFD_TOPOLOGY_ROOT: &str = "/sys/class/kfd/kfd/topology/nodes";
const EXPECTED_AMD_VENDOR_ID: u64 = 0x1002;
const GFX942_TARGET_VERSION: u64 = 90_402;
const GFX950_TARGET_VERSION: u64 = 90_500;
const PRODUCTION_WAVE_WIDTH: u64 = 64;

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
    canonical_path: PathBuf,
    identity: ObjectIdentity,
    digest: [u8; 32],
    prefix: Vec<u8>,
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
        Ok(Self {
            file,
            canonical_path,
            identity,
            digest: hasher.finalize().into(),
            prefix,
        })
    }

    fn validate(&self, label: &str) -> Result<(), String> {
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
    bytes: Vec<u8>,
    digest: [u8; 32],
    target_profile: ObservedGpuTargetProfileRecordV1,
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
}

struct CollectorLibraries {
    tool_route: PathBuf,
    tool: FilePin,
    core_route: PathBuf,
    core: FilePin,
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
    if supplied_authorization != Some(plan.authorization) {
        return Err(format!(
            "profile collection authorization does not match this exact plan; rerun without --collect and pass --authorize-collection {}",
            hex(&plan.authorization)
        ));
    }
    collect(plan)
}

fn usage() -> &'static str {
    "usage: cargo fe2o3 profile [--kind dispatch-json|dispatch-csv|att] [--tool /absolute/path/to/rocprofv3] [--python /absolute/path/to/python3] --output-dir /absolute/new/directory [--cwd /absolute/directory] [--timeout-ms N] [--stdout-limit N] [--stderr-limit N] [--storage-limit N] [--kir-sha256 HEX --kir-len N --wave-width 32|64] [--collect --authorize-collection HEX] -- <program> [arguments...]"
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
    let collector_tool_bytes =
        canonical_collector_tool(&tool, &interpreter, collector_libraries.as_ref());
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
    });
    Ok(Plan {
        options,
        working_directory,
        output_directory,
        tool,
        interpreter,
        collector_libraries,
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
    })
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
        let target_profile = ObservedGpuTargetProfileRecordV1::from_direct_kfd_properties(
            required_u64_property(&parsed, node, "vendor_id")?,
            required_u64_property(&parsed, node, "gfx_target_version")?,
            required_u64_property(&parsed, node, "wave_front_size")?,
        );
        let mut bytes = b"fe2o3-kfd-stable-device-v1\n".to_vec();
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
) -> Vec<u8> {
    let mut bytes = b"fe2o3-rocprofv3-toolchain-v1\0".to_vec();
    for pin in [script, interpreter] {
        append_field(&mut bytes, &pin.digest);
        append_field(&mut bytes, &pin.identity.size.to_le_bytes());
    }
    if let Some(libraries) = libraries {
        for pin in [&libraries.tool, &libraries.core] {
            append_field(&mut bytes, &pin.digest);
            append_field(&mut bytes, &pin.identity.size.to_le_bytes());
        }
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
    } = input;
    let mut hasher = Sha256::new();
    hash_field(&mut hasher, b"fe2o3-profile-authorization-v1");
    hash_field(&mut hasher, options.kind.name().as_bytes());
    hash_field(&mut hasher, &tool.digest);
    hash_field(&mut hasher, &tool.identity.size.to_le_bytes());
    hash_field(&mut hasher, &interpreter.digest);
    hash_field(&mut hasher, &interpreter.identity.size.to_le_bytes());
    hash_field(&mut hasher, &target.digest);
    hash_field(&mut hasher, &target.identity.size.to_le_bytes());
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
    let environment = content_identity(
        &plan.environment_digest,
        plan.environment_bytes.len() as u64,
    );
    let tool = content_identity(
        &plan.collector_tool_digest,
        plan.collector_tool_bytes.len() as u64,
    );
    let configuration =
        content_identity(&plan.configuration_digest, plan.configuration.len() as u64);
    if plan.devices.is_empty() {
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
        return;
    }
    match plan.options.kind {
        ProfileKind::DispatchJson | ProfileKind::DispatchCsv => {
            if !render_dispatch_import_plan(
                output,
                plan.options.kind,
                &plan.devices,
                plan.options.kir_binding.as_ref(),
                environment,
                tool,
                configuration,
            ) {
                return;
            }
        }
        ProfileKind::Att => {
            line(output, "next-import-program", "fe2o3-profiler-import");
            line(
                output,
                "next-import-status",
                "deferred-until-att-manifest-references-are-content-bound",
            );
            for (index, argument) in [
                "att-v4".to_owned(),
                "--environment".to_owned(),
                environment,
                "--tool".to_owned(),
                tool,
                "--config".to_owned(),
                configuration,
            ]
            .into_iter()
            .enumerate()
            {
                line_debug(output, &format!("next-import-arg[{index}]"), &argument);
            }
            line(output, "next-import-deferred-flag", "--att-artifact");
            line(
                output,
                "next-import-deferred-value-format",
                "manifest-relative-reference=raw:1:sha256:length",
            );
            line(output, "next-import-deferred-flag", "--att-agent-id");
            line(
                output,
                "next-import-deferred-value-format",
                "absolute-kfd-node-id-from-validated-att-output-directory",
            );
            line(output, "next-import-deferred-flag", "--device-binding");
            line(
                output,
                "next-import-deferred-value-format",
                "absolute-kfd-node-id=domain:1:stable-device-sha256:length",
            );
            for (index, device) in plan.devices.iter().enumerate() {
                line(
                    output,
                    &format!("next-import-att-device-candidate[{index}]"),
                    format!("{}={}", device.node, device.content_identity()),
                );
            }
        }
    }
    line(output, "next-query-program", "fe2o3-profiler-query");
    line(output, "next-query-arg[0]", "capabilities");
    line(output, "next-query-stdin", "imported-fe2o3prof4-bundle");
}

fn render_dispatch_import_plan(
    output: &mut String,
    kind: ProfileKind,
    devices: &[DeviceIdentity],
    kir: Option<&KirBinding>,
    environment: String,
    tool: String,
    configuration: String,
) -> bool {
    let Some(kir) = kir else {
        line(
            output,
            "next-import-status",
            "unavailable-missing-kir-identity-length-and-wave-width",
        );
        line(
            output,
            "next-query-status",
            "unavailable-until-bundle-v4-import",
        );
        return false;
    };
    if devices.iter().any(|device| {
        matches!(
            device.target_profile.status,
            ObservedGpuTargetProfileStatusV1::Unavailable(_)
        )
    }) {
        line(
            output,
            "next-import-status",
            "unavailable-observed-gpu-target-profile",
        );
        line(
            output,
            "next-import-unavailable-reason",
            "one-or-more-direct-kfd-target-profiles-unavailable",
        );
        for (index, (device, reason)) in devices
            .iter()
            .filter_map(|device| match device.target_profile.status {
                ObservedGpuTargetProfileStatusV1::Observed(_) => None,
                ObservedGpuTargetProfileStatusV1::Unavailable(reason) => Some((device, reason)),
            })
            .enumerate()
        {
            line(
                output,
                &format!("next-import-unavailable-device[{index}]"),
                format!("node={};reason={}", device.node, reason.name()),
            );
        }
        line(
            output,
            "next-query-status",
            "unavailable-until-bundle-v4-import",
        );
        return false;
    }
    if devices
        .iter()
        .any(|device| device.target_profile.wave_width != u64::from(kir.wave_width))
    {
        line(
            output,
            "next-import-status",
            "unavailable-kir-wave-width-mismatch",
        );
        line(
            output,
            "next-import-unavailable-reason",
            "caller-kir-wave-width-does-not-match-observed-direct-kfd-device",
        );
        for (index, device) in devices
            .iter()
            .filter(|device| device.target_profile.wave_width != u64::from(kir.wave_width))
            .enumerate()
        {
            line(
                output,
                &format!("next-import-wave-mismatch-device[{index}]"),
                format!(
                    "node={};observed-wave-width={};kir-wave-width={}",
                    device.node, device.target_profile.wave_width, kir.wave_width
                ),
            );
        }
        line(
            output,
            "next-query-status",
            "unavailable-until-bundle-v4-import",
        );
        return false;
    }

    line(output, "next-import-program", "fe2o3-profiler-import");
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
    let command = if kind == ProfileKind::DispatchJson {
        "dispatch-json-v4"
    } else {
        "dispatch-csv-v4"
    };
    for (index, argument) in [
        command.to_owned(),
        "--environment".to_owned(),
        environment,
        "--tool".to_owned(),
        tool,
        "--config".to_owned(),
        configuration,
        "--kir-sha256".to_owned(),
        hex(&kir.digest),
        "--kir-len".to_owned(),
        kir.length.to_string(),
        "--wave-width".to_owned(),
        kir.wave_width.to_string(),
    ]
    .into_iter()
    .chain(devices.iter().flat_map(|device| {
        [
            "--device-binding".to_owned(),
            format!("{}={}", device.node, device.content_identity()),
        ]
    }))
    .enumerate()
    {
        line_debug(output, &format!("next-import-arg[{index}]"), &argument);
    }
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
    plan.tool.validate("rocprofv3 script")?;
    plan.interpreter.validate("rocprofv3 Python interpreter")?;
    if let Some(libraries) = &plan.collector_libraries {
        libraries.validate()?;
    }
    plan.target.validate("profile target")?;
    validate_device_bindings(&plan.devices)?;
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
        .and_then(|()| validate_device_bindings(&plan.devices))
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
    let artifacts = match scan_artifacts(&plan.output_directory, plan.options.storage_limit) {
        Ok(artifacts) => artifacts,
        Err(error) => {
            custody.cleanup()?;
            return Err(format!("collector output rejected and cleaned: {error}"));
        }
    };
    let manifest = render_manifest(&plan, &artifacts);
    if manifest.len() as u64
        > plan
            .options
            .storage_limit
            .saturating_sub(artifacts.iter().map(|artifact| artifact.length).sum())
    {
        custody.cleanup()?;
        return Err(
            "collector manifest would exceed the storage limit; output was cleaned".to_owned(),
        );
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
    Ok(render_successful_collection(&plan, supervised, &artifacts))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum StopReason {
    Exited,
    Timeout,
    OutputOverflow,
    WaitFailure,
}

struct BoundedCapture {
    bytes: Vec<u8>,
    overflow: bool,
}

struct Supervised {
    status: Option<ExitStatus>,
    reason: StopReason,
    stdout: BoundedCapture,
    stderr: BoundedCapture,
    wait_error: Option<String>,
}

fn run_collector(plan: &Plan) -> Result<Supervised, String> {
    let mut command = Command::new(plan.interpreter.execution_path());
    command
        .arg0(&plan.interpreter.canonical_path)
        .arg(plan.tool.external_path())
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
    let mut child = crate::process_execution::spawn(&mut command)
        .map_err(|error| format!("failed to spawn pinned rocprofv3 collector: {error}"))?;
    supervise(
        &mut child,
        plan.options.timeout,
        plan.options.stdout_limit,
        plan.options.stderr_limit,
    )
}

fn supervise(
    child: &mut Child,
    timeout: Duration,
    stdout_limit: usize,
    stderr_limit: usize,
) -> Result<Supervised, String> {
    let stdout = child
        .stdout
        .take()
        .ok_or("collector stdout pipe was unavailable")?;
    let stderr = child
        .stderr
        .take()
        .ok_or("collector stderr pipe was unavailable")?;
    let overflow = Arc::new(AtomicBool::new(false));
    let stdout_thread = capture_thread(stdout, stdout_limit, Arc::clone(&overflow));
    let stderr_thread = capture_thread(stderr, stderr_limit, Arc::clone(&overflow));
    let started = Instant::now();
    let (status, reason, wait_error) = loop {
        if overflow.load(Ordering::Acquire) {
            terminate(child);
            break (child.wait().ok(), StopReason::OutputOverflow, None);
        }
        match child.try_wait() {
            Ok(Some(status)) => break (Some(status), StopReason::Exited, None),
            Ok(None) if started.elapsed() >= timeout => {
                terminate(child);
                break (child.wait().ok(), StopReason::Timeout, None);
            }
            Ok(None) => thread::sleep(POLL_INTERVAL),
            Err(error) => {
                terminate(child);
                let _ = child.wait();
                break (None, StopReason::WaitFailure, Some(error.to_string()));
            }
        }
    };
    terminate(child);
    let stdout = stdout_thread
        .join()
        .map_err(|_| "collector stdout capture thread panicked")?;
    let stderr = stderr_thread
        .join()
        .map_err(|_| "collector stderr capture thread panicked")?;
    Ok(Supervised {
        status,
        reason,
        stdout,
        stderr,
        wait_error,
    })
}

fn capture_thread(
    mut pipe: impl Read + Send + 'static,
    limit: usize,
    global_overflow: Arc<AtomicBool>,
) -> thread::JoinHandle<BoundedCapture> {
    thread::spawn(move || {
        let mut bytes = Vec::new();
        let mut overflow = false;
        let mut buffer = [0_u8; 8192];
        loop {
            match pipe.read(&mut buffer) {
                Ok(0) => break,
                Ok(read) => {
                    let remaining = limit.saturating_sub(bytes.len());
                    bytes.extend_from_slice(&buffer[..read.min(remaining)]);
                    if read > remaining {
                        overflow = true;
                        global_overflow.store(true, Ordering::Release);
                    }
                }
                Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
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

fn terminate(child: &mut Child) {
    let pid = i32::try_from(child.id()).unwrap_or(i32::MAX);
    // SAFETY: a negative, positive process-group id targets the fresh child group created above.
    let _ = unsafe { libc::kill(-pid, libc::SIGKILL) };
    let _ = child.kill();
}

struct OutputCustody {
    path: PathBuf,
    identity: ObjectIdentity,
    guard: Vec<u8>,
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
        Ok(Self {
            path: path.to_path_buf(),
            identity: ObjectIdentity::from_metadata(&metadata),
            guard,
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
        let guard = fs::read(self.path.join(OWNERSHIP_FILE))
            .map_err(|error| format!("output ownership guard unavailable: {error}"))?;
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
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(self.path.join(MANIFEST_FILE))
            .map_err(|error| format!("failed to create collection manifest: {error}"))?;
        file.write_all(bytes)
            .and_then(|()| file.sync_all())
            .map_err(|error| format!("failed to persist collection manifest: {error}"))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Artifact {
    relative: String,
    length: u64,
    digest: [u8; 32],
}

fn scan_artifacts(root: &Path, storage_limit: u64) -> Result<Vec<Artifact>, String> {
    let mut pending = vec![(root.to_path_buf(), 0_usize)];
    let mut artifacts = Vec::new();
    let mut total = 0_u64;
    while let Some((directory, depth)) = pending.pop() {
        if depth > MAX_ARTIFACT_DEPTH {
            return Err("collector output exceeds the directory-depth bound".to_owned());
        }
        let mut entries = fs::read_dir(&directory)
            .map_err(|error| format!("failed to enumerate collector output: {error}"))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| format!("failed to enumerate collector output: {error}"))?;
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
            if relative == OWNERSHIP_FILE || relative == MANIFEST_FILE {
                continue;
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
            let (digest, length) = hash_file(&path, &metadata)?;
            artifacts.push(Artifact {
                relative,
                length,
                digest,
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

fn hash_file(path: &Path, expected: &Metadata) -> Result<([u8; 32], u64), String> {
    let fd = rustix::fs::open(
        path,
        rustix::fs::OFlags::RDONLY
            | rustix::fs::OFlags::NOFOLLOW
            | rustix::fs::OFlags::NONBLOCK
            | rustix::fs::OFlags::CLOEXEC,
        rustix::fs::Mode::empty(),
    )
    .map_err(|error| format!("failed to retain collector artifact: {error}"))?;
    let mut file = File::from(fd);
    let expected_identity = ObjectIdentity::from_metadata(expected);
    if ObjectIdentity::from_metadata(
        &file
            .metadata()
            .map_err(|error| format!("failed to inspect collector artifact: {error}"))?,
    ) != expected_identity
    {
        return Err("collector artifact was substituted before hashing".to_owned());
    }
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
    if length != expected.len()
        || ObjectIdentity::from_metadata(
            &file
                .metadata()
                .map_err(|error| format!("failed to re-inspect collector artifact: {error}"))?,
        ) != expected_identity
        || ObjectIdentity::from_metadata(
            &fs::symlink_metadata(path)
                .map_err(|error| format!("collector artifact path changed: {error}"))?,
        ) != expected_identity
    {
        return Err("collector artifact changed while hashing".to_owned());
    }
    Ok((hasher.finalize().into(), length))
}

fn render_manifest(plan: &Plan, artifacts: &[Artifact]) -> String {
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
    for (index, artifact) in artifacts
        .iter()
        .filter(|artifact| match plan.options.kind {
            ProfileKind::DispatchJson => artifact.relative.ends_with(".json"),
            ProfileKind::DispatchCsv => artifact.relative.ends_with(".csv"),
            ProfileKind::Att => artifact.relative.ends_with(".json"),
        })
        .enumerate()
    {
        line(
            &mut output,
            &format!("import-source-candidate[{index}]"),
            format!(
                "path={:?};bytes={};status={}",
                artifact.relative,
                artifact.length,
                if artifact.length <= MAX_PROFILER_IMPORT_SOURCE_BYTES {
                    "size-eligible-requires-schema-validation"
                } else {
                    "unavailable-exceeds-import-source-byte-limit"
                }
            ),
        );
    }
    render_import_plan(&mut output, plan);
    line(&mut output, "dispatch-observation-origin", "unavailable");
    line(
        &mut output,
        "dispatch-observation-reason",
        "bundle-v4-import-not-run",
    );
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
    use std::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};

    static NEXT_TOPOLOGY_FIXTURE: AtomicU64 = AtomicU64::new(0);

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
            digest: Sha256::digest(&bytes).into(),
            bytes,
            target_profile: ObservedGpuTargetProfileRecordV1::from_direct_kfd_properties(
                vendor, target, wave,
            ),
        }
    }

    fn dispatch_import_output(devices: &[DeviceIdentity], kir_wave: Option<u8>) -> String {
        let kir = kir_wave.map(|wave_width| KirBinding {
            digest: [3; 32],
            length: 17,
            wave_width,
        });
        let mut output = String::new();
        let _ = render_dispatch_import_plan(
            &mut output,
            ProfileKind::DispatchJson,
            devices,
            kir.as_ref(),
            "environment".to_owned(),
            "tool".to_owned(),
            "configuration".to_owned(),
        );
        output
    }

    fn assert_no_dispatch_import_command(output: &str) {
        assert!(!output.contains("next-import-program:"));
        assert!(!output.contains("next-import-arg["));
        assert!(!output.contains("ready-after-collector"));
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
    fn dispatch_import_rejects_every_typed_unavailable_target_profile() {
        for (device, reason) in [
            (
                profile_device(1, EXPECTED_AMD_VENDOR_ID, 90_401, PRODUCTION_WAVE_WIDTH),
                "unknown-gfx-target-version",
            ),
            (
                profile_device(2, 0, GFX942_TARGET_VERSION, PRODUCTION_WAVE_WIDTH),
                "vendor-contradicts-amd-target",
            ),
            (
                profile_device(3, EXPECTED_AMD_VENDOR_ID, GFX950_TARGET_VERSION, 32),
                "wave-width-contradicts-target",
            ),
        ] {
            let output = dispatch_import_output(&[device], Some(64));
            assert!(output.contains("next-import-status: unavailable-observed-gpu-target-profile"));
            assert!(output.contains(&format!("reason={reason}")));
            assert_no_dispatch_import_command(&output);
        }
    }

    #[test]
    fn dispatch_import_rejects_caller_wave_mismatch_without_emitting_arguments() {
        let devices = [
            profile_device(
                1,
                EXPECTED_AMD_VENDOR_ID,
                GFX942_TARGET_VERSION,
                PRODUCTION_WAVE_WIDTH,
            ),
            profile_device(
                2,
                EXPECTED_AMD_VENDOR_ID,
                GFX950_TARGET_VERSION,
                PRODUCTION_WAVE_WIDTH,
            ),
        ];
        let output = dispatch_import_output(&devices, Some(32));
        assert!(output.contains("next-import-status: unavailable-kir-wave-width-mismatch"));
        assert!(output.contains("node=1;observed-wave-width=64;kir-wave-width=32"));
        assert!(output.contains("node=2;observed-wave-width=64;kir-wave-width=32"));
        assert_no_dispatch_import_command(&output);

        let missing = dispatch_import_output(&devices, None);
        assert!(missing.contains(
            "next-import-status: unavailable-missing-kir-identity-length-and-wave-width"
        ));
        assert_no_dispatch_import_command(&missing);
    }

    #[test]
    fn dispatch_import_is_ready_only_for_observed_wave_compatible_devices() {
        let devices = [
            profile_device(
                1,
                EXPECTED_AMD_VENDOR_ID,
                GFX942_TARGET_VERSION,
                PRODUCTION_WAVE_WIDTH,
            ),
            profile_device(
                2,
                EXPECTED_AMD_VENDOR_ID,
                GFX950_TARGET_VERSION,
                PRODUCTION_WAVE_WIDTH,
            ),
        ];
        let output = dispatch_import_output(&devices, Some(64));
        assert!(output.contains("next-import-program: fe2o3-profiler-import"));
        assert!(output.contains(
            "next-import-status: ready-after-collector-artifact-and-source-size-validation"
        ));
        assert!(output.contains("\"--device-binding\""));
        assert!(output.contains("\"--wave-width\""));
    }

    #[test]
    fn target_profile_record_is_canonical_bounded_and_authorized() {
        let mut device = DeviceIdentity {
            node: 7,
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
