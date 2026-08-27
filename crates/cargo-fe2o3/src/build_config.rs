//! Pinned build recipe for the production compiler transaction.
//!
//! The executable exposes one dedicated production parser and type. Legacy
//! qualification manifests are rejected before the production manifest is read.

use std::error::Error;
use std::fmt;
use std::fs::OpenOptions;
use std::io::Read;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::protected_compiler_handoff_v3::ParentConsumedProductionHandoff;
use fe2o3_artifact_transaction::CompilerModuleHandoffReceiptV3;
use fe2o3_build_authority::CompilerClosureV2;
use fe2o3_compiler_ffi::InertSemanticCompilerModuleHandoffV3;
use fe2o3_hsaco_finalize::{
    ContentIdentityV1, InertProtectedFirstBuildWorkerV3EvidenceV1, LinkOptionV1, LinkPlanError,
    MAX_LINK_INPUTS, PinnedWorkerV1, PreparedProtectedFirstBuildWorkerV3PreflightV1,
    ProtectedFirstBuildWorkerV3Error, WorkerExecutionError, WorkerExecutionLimitsV1,
    WorkerInputKindV1, WorkerInputV1, WorkerMeasurementV1, WorkerOutputConstraintsV1,
    WorkerProtocolError, execute_preflighted_protected_reproducible_first_build_worker_v3,
    preflight_protected_reproducible_first_build_worker_v3,
};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

pub(crate) const QUALIFICATION_ORACLE_ENV: &str = "FE2O3_QUALIFICATION_ORACLE_V1";
const OBSOLETE_CODEGEN_PIPELINE_ENV: &str = "FE2O3_CODEGEN_PIPELINE";
pub(crate) const PRODUCTION_BUILD_CONFIG_ENV: &str = "FE2O3_PRODUCTION_BUILD_CONFIG_V1";
pub(crate) const PRODUCTION_BUILD_EXPECTED_ID_ENV: &str = "FE2O3_PRODUCTION_BUILD_EXPECTED_ID_V1";
pub(crate) const WORKER_V2_CONFIG_ENV: &str = "FE2O3_WORKER_V2_CONFIG_V2";
pub(crate) const WORKER_V2_EXPECTED_ID_ENV: &str = "FE2O3_WORKER_V2_EXPECTED_ID_V1";
pub(crate) const WORKER_V2_SOURCE_DEBUG_PROFILE_ENV: &str =
    "FE2O3_WORKER_V2_SOURCE_DEBUG_PROFILE_V1";
const PRODUCTION_CONFIG_PROFILE_ID_V1: &str = "production-v1";
const PRODUCTION_BUILD_CONFIG_FORMAT: &str = "fe2o3-production-build-config-v1";
const MAX_CONFIG_BYTES: usize = 1024 * 1024;
const MAX_CONFIG_PATH_BYTES: usize = 4096;

const ROOT_KEYS: &[&str] = &[
    "candidate_output_max_bytes",
    "format",
    "limits",
    "link_options",
    "providers",
    "units",
    "worker",
];
const WORKER_KEYS: &[&str] = &[
    "byte_len",
    "llvm_build_identity",
    "path",
    "sha256",
    "worker_build_identity",
];
const PROVIDER_KEYS: &[&str] = &["byte_len", "kind", "path", "sha256"];
const OPTION_KEYS: &[&str] = &["name", "value"];
const LIMIT_KEYS: &[&str] = &["stderr_bytes", "stdout_bytes", "timeout_ms"];
const UNIT_KEYS: &[&str] = &["crate_name", "source", "working_directory"];
const REQUIRED_OPTIONS: &[(&str, &[&str])] = &[
    ("code-object-version", &["4", "5", "6"]),
    ("opt-level", &["0", "1", "2", "3"]),
    ("strip-debug", &["false", "true"]),
    ("verify-each", &["false", "true"]),
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct BuildConfigIdentity([u8; 32]);

impl BuildConfigIdentity {
    pub(crate) const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    pub(crate) fn to_hex(self) -> String {
        hex(&self.0)
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct ConfiguredUnit {
    crate_name: String,
    source: String,
    working_directory: String,
}

struct PreparedLinkBuildConfig {
    identity: BuildConfigIdentity,
    worker: PinnedWorkerV1,
    providers: Vec<WorkerInputV1>,
    link_options: Vec<LinkOptionV1>,
    candidate_output: WorkerOutputConstraintsV1,
    limits: WorkerExecutionLimitsV1,
    units: Vec<ConfiguredUnit>,
}

/// Production-only ownership of one pinned, workload-neutral link recipe.
pub(crate) struct PreparedProductionBuildConfig {
    link: PreparedLinkBuildConfig,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum BuildCompileEnvironmentProfileV1 {
    ProductionAmd,
}

impl PreparedProductionBuildConfig {
    pub(crate) fn from_environment() -> Result<Option<Self>, BuildConfigError> {
        if let Some(value) = std::env::var_os(OBSOLETE_CODEGEN_PIPELINE_ENV) {
            return Err(BuildConfigError::Invalid(format!(
                "{OBSOLETE_CODEGEN_PIPELINE_ENV} has been removed; production compilation has no selector; found {value:?}"
            )));
        }
        if let Some(value) = std::env::var_os(QUALIFICATION_ORACLE_ENV) {
            return Err(BuildConfigError::Invalid(format!(
                "{QUALIFICATION_ORACLE_ENV} is unavailable; production compilation has no selector; found {value:?}"
            )));
        }
        if std::env::var_os(WORKER_V2_CONFIG_ENV).is_some() {
            return Err(BuildConfigError::Invalid(format!(
                "{WORKER_V2_CONFIG_ENV} is qualification-only; production requires {PRODUCTION_BUILD_CONFIG_ENV}"
            )));
        }
        let Some(path) = std::env::var_os(PRODUCTION_BUILD_CONFIG_ENV) else {
            return Err(BuildConfigError::MissingConfiguration);
        };
        if path.is_empty() {
            return Err(BuildConfigError::MissingConfiguration);
        }
        Self::from_manifest(Path::new(&path)).map(Some)
    }

    pub(crate) fn from_environment_for_cargo_setup() -> Result<Option<Self>, BuildConfigError> {
        Self::from_environment()
    }

    fn from_manifest(path: &Path) -> Result<Self, BuildConfigError> {
        prepare_production_manifest(path)
    }

    pub(crate) const fn identity(&self) -> BuildConfigIdentity {
        self.link.identity
    }

    pub(crate) fn compile_environment_profile(
        &self,
        crate_name: &str,
        source: &Path,
        working_directory: &Path,
    ) -> Option<BuildCompileEnvironmentProfileV1> {
        self.selects(crate_name, source, working_directory)
            .then_some(BuildCompileEnvironmentProfileV1::ProductionAmd)
    }

    pub(crate) fn selects(
        &self,
        crate_name: &str,
        source: &Path,
        working_directory: &Path,
    ) -> bool {
        let Some(source) = source.to_str() else {
            return false;
        };
        let Some(working_directory) = working_directory.to_str() else {
            return false;
        };
        self.link
            .units
            .binary_search_by(|unit| {
                (
                    unit.crate_name.as_str(),
                    unit.source.as_str(),
                    unit.working_directory.as_str(),
                )
                    .cmp(&(crate_name, source, working_directory))
            })
            .is_ok()
    }
}

fn prepare_production_manifest(
    path: &Path,
) -> Result<PreparedProductionBuildConfig, BuildConfigError> {
    require_absolute_path(path, "configuration")?;
    let bytes = read_bounded(path, MAX_CONFIG_BYTES, "configuration")?;
    let value: Value = serde_json::from_slice(&bytes)
        .map_err(|error| BuildConfigError::Json(error.to_string()))?;
    let canonical =
        serde_json::to_vec(&value).map_err(|error| BuildConfigError::Json(error.to_string()))?;
    if canonical != bytes {
        return Err(BuildConfigError::Invalid(
            "configuration must be compact canonical JSON with lexicographically ordered object keys"
                .to_owned(),
        ));
    }

    let root = exact_production_root_object(&value)?;
    if required_string(root, "format", "configuration")? != PRODUCTION_BUILD_CONFIG_FORMAT {
        return Err(BuildConfigError::Invalid(format!(
            "configuration format must be exactly {PRODUCTION_BUILD_CONFIG_FORMAT:?}"
        )));
    }
    let worker = prepare_worker(required_value(root, "worker", "configuration")?)?;
    let providers = prepare_providers(required_value(root, "providers", "configuration")?)?;
    let link_options = parse_link_options(required_value(root, "link_options", "configuration")?)?;
    let candidate_output = WorkerOutputConstraintsV1::new(required_u64(
        root,
        "candidate_output_max_bytes",
        "configuration",
    )?)
    .map_err(BuildConfigError::Protocol)?;
    let limits = parse_limits(required_value(root, "limits", "configuration")?)?;
    let units = parse_units(required_value(root, "units", "configuration")?)?;
    let identity =
        transitive_identity(PRODUCTION_CONFIG_PROFILE_ID_V1, &bytes, &worker, &providers);
    Ok(PreparedProductionBuildConfig {
        link: PreparedLinkBuildConfig {
            identity,
            worker,
            providers,
            link_options,
            candidate_output,
            limits,
            units,
        },
    })
}

impl PreparedProductionBuildConfig {
    pub(crate) fn preflight_production(
        &self,
        handoff: &InertSemanticCompilerModuleHandoffV3,
        receipt: CompilerModuleHandoffReceiptV3,
        compiler_closure: CompilerClosureV2,
    ) -> Result<PreparedProtectedFirstBuildWorkerV3PreflightV1, ProtectedFirstBuildWorkerV3Error>
    {
        preflight_protected_reproducible_first_build_worker_v3(
            handoff,
            receipt,
            compiler_closure,
            &self.link.worker,
            self.link.providers.clone(),
            self.link.link_options.clone(),
            self.link.candidate_output.clone(),
            self.link.limits,
        )
    }

    pub(crate) fn execute_preflighted_production(
        &self,
        parent_consumed: ParentConsumedProductionHandoff,
        preflight: PreparedProtectedFirstBuildWorkerV3PreflightV1,
    ) -> Result<InertProtectedFirstBuildWorkerV3EvidenceV1, ProtectedFirstBuildWorkerV3Error> {
        let (_, consumed, _) = parent_consumed.into_parts();
        execute_preflighted_protected_reproducible_first_build_worker_v3(
            consumed,
            preflight,
            &self.link.worker,
        )
    }
}

fn transitive_identity(
    profile: &str,
    manifest: &[u8],
    worker: &PinnedWorkerV1,
    providers: &[WorkerInputV1],
) -> BuildConfigIdentity {
    let mut hash = Sha256::new();
    update_identity(&mut hash, b"fe2o3-build-config-transitive-v1");
    update_identity(&mut hash, profile.as_bytes());
    update_identity(&mut hash, manifest);
    let measurement = worker.measurement();
    update_identity(&mut hash, measurement.executable().sha256());
    update_identity(
        &mut hash,
        &measurement.executable().byte_len().to_le_bytes(),
    );
    update_identity(&mut hash, measurement.worker_build_identity().as_bytes());
    update_identity(&mut hash, measurement.llvm_build_identity().as_bytes());
    update_identity(&mut hash, &(providers.len() as u64).to_le_bytes());
    for provider in providers {
        update_identity(&mut hash, &[provider.kind() as u8]);
        update_identity(&mut hash, provider.identity().sha256());
        update_identity(&mut hash, &provider.identity().byte_len().to_le_bytes());
        update_identity(&mut hash, provider.bytes());
    }
    BuildConfigIdentity(hash.finalize().into())
}

fn update_identity(hash: &mut Sha256, bytes: &[u8]) {
    hash.update((bytes.len() as u64).to_le_bytes());
    hash.update(bytes);
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(DIGITS[(byte >> 4) as usize] as char);
        encoded.push(DIGITS[(byte & 0x0f) as usize] as char);
    }
    encoded
}

#[derive(Debug)]
pub(crate) enum BuildConfigError {
    MissingConfiguration,
    Io {
        kind: &'static str,
        path: PathBuf,
        error: std::io::Error,
    },
    Json(String),
    Invalid(String),
    LinkPlan(LinkPlanError),
    Protocol(WorkerProtocolError),
    Worker(WorkerExecutionError),
}

impl fmt::Display for BuildConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingConfiguration => {
                write!(
                    formatter,
                    "production requires {PRODUCTION_BUILD_CONFIG_ENV}"
                )
            }
            Self::Io { kind, path, error } => {
                write!(
                    formatter,
                    "failed to read build configuration {kind} {}: {error}",
                    path.display()
                )
            }
            Self::Json(error) => write!(formatter, "invalid build configuration JSON: {error}"),
            Self::Invalid(reason) => write!(formatter, "invalid build configuration: {reason}"),
            Self::LinkPlan(error) => write!(formatter, "invalid build link option: {error}"),
            Self::Protocol(error) => write!(formatter, "invalid build input: {error}"),
            Self::Worker(error) => write!(formatter, "invalid build worker measurement: {error}"),
        }
    }
}

impl Error for BuildConfigError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io { error, .. } => Some(error),
            Self::LinkPlan(error) => Some(error),
            Self::Protocol(error) => Some(error),
            Self::Worker(error) => Some(error),
            Self::MissingConfiguration | Self::Json(_) | Self::Invalid(_) => None,
        }
    }
}

fn prepare_worker(value: &Value) -> Result<PinnedWorkerV1, BuildConfigError> {
    let object = exact_object(value, WORKER_KEYS, "worker")?;
    let path = absolute_json_path(required_string(object, "path", "worker")?, "worker")?;
    let identity = declared_identity(object, "worker")?;
    let measurement = WorkerMeasurementV1::new(
        identity,
        required_string(object, "worker_build_identity", "worker")?,
        required_string(object, "llvm_build_identity", "worker")?,
    )
    .map_err(BuildConfigError::Worker)?;
    PinnedWorkerV1::open(path, measurement).map_err(BuildConfigError::Worker)
}

fn prepare_providers(value: &Value) -> Result<Vec<WorkerInputV1>, BuildConfigError> {
    let values = value
        .as_array()
        .ok_or_else(|| BuildConfigError::Invalid("providers must be an array".to_owned()))?;
    if values.len() >= MAX_LINK_INPUTS {
        return Err(BuildConfigError::Invalid(format!(
            "providers must contain fewer than {MAX_LINK_INPUTS} entries"
        )));
    }

    let mut providers = Vec::with_capacity(values.len());
    let mut previous = None;
    for (index, value) in values.iter().enumerate() {
        let context = format!("providers[{index}]");
        let object = exact_object(value, PROVIDER_KEYS, &context)?;
        let path = absolute_json_path(required_string(object, "path", &context)?, &context)?;
        let identity = declared_identity(object, &context)?;
        if previous.is_some_and(|previous| previous >= identity) {
            return Err(BuildConfigError::Invalid(
                "providers must be strictly ordered by declared content identity".to_owned(),
            ));
        }
        previous = Some(identity);
        let kind = match required_string(object, "kind", &context)? {
            "llvm-bitcode" => WorkerInputKindV1::LlvmBitcode,
            "amdgpu-relocatable" => WorkerInputKindV1::AmdGpuRelocatable,
            "llvm-text-ir" => WorkerInputKindV1::LlvmTextIr,
            other => {
                return Err(BuildConfigError::Invalid(format!(
                    "{context}.kind has unsupported value {other:?}"
                )));
            }
        };
        let bytes = read_bounded(
            &path,
            fe2o3_hsaco_finalize::MAX_WORKER_TOTAL_INPUT_BYTES,
            "provider",
        )?;
        providers.push(
            WorkerInputV1::from_declared(kind, identity, bytes)
                .map_err(BuildConfigError::Protocol)?,
        );
    }
    Ok(providers)
}

fn parse_link_options(value: &Value) -> Result<Vec<LinkOptionV1>, BuildConfigError> {
    let values = value
        .as_array()
        .ok_or_else(|| BuildConfigError::Invalid("link_options must be an array".to_owned()))?;
    if values.len() != REQUIRED_OPTIONS.len() {
        return Err(BuildConfigError::Invalid(
            "link_options must explicitly contain code-object-version, opt-level, strip-debug, and verify-each"
                .to_owned(),
        ));
    }

    let mut options = Vec::with_capacity(values.len());
    for (index, ((expected_name, allowed_values), value)) in
        REQUIRED_OPTIONS.iter().zip(values).enumerate()
    {
        let context = format!("link_options[{index}]");
        let object = exact_object(value, OPTION_KEYS, &context)?;
        let name = required_string(object, "name", &context)?;
        let option_value = required_string(object, "value", &context)?;
        if name != *expected_name || !allowed_values.contains(&option_value) {
            return Err(BuildConfigError::Invalid(format!(
                "{context} must be {expected_name:?} with one of {allowed_values:?}"
            )));
        }
        options.push(LinkOptionV1::new(name, option_value).map_err(BuildConfigError::LinkPlan)?);
    }
    Ok(options)
}

fn parse_limits(value: &Value) -> Result<WorkerExecutionLimitsV1, BuildConfigError> {
    let object = exact_object(value, LIMIT_KEYS, "limits")?;
    let timeout_ms = required_u64(object, "timeout_ms", "limits")?;
    let stdout_bytes = usize::try_from(required_u64(object, "stdout_bytes", "limits")?)
        .map_err(|_| BuildConfigError::Invalid("limits.stdout_bytes is too large".to_owned()))?;
    let stderr_bytes = usize::try_from(required_u64(object, "stderr_bytes", "limits")?)
        .map_err(|_| BuildConfigError::Invalid("limits.stderr_bytes is too large".to_owned()))?;
    WorkerExecutionLimitsV1::new(
        Duration::from_millis(timeout_ms),
        stdout_bytes,
        stderr_bytes,
    )
    .map_err(BuildConfigError::Worker)
}

fn parse_units(value: &Value) -> Result<Vec<ConfiguredUnit>, BuildConfigError> {
    let values = value
        .as_array()
        .ok_or_else(|| BuildConfigError::Invalid("units must be an array".to_owned()))?;
    if values.is_empty() || values.len() > 1024 {
        return Err(BuildConfigError::Invalid(
            "units must contain 1..=1024 exact compilation-unit selectors".to_owned(),
        ));
    }
    let mut units = Vec::with_capacity(values.len());
    for (index, value) in values.iter().enumerate() {
        let context = format!("units[{index}]");
        let object = exact_object(value, UNIT_KEYS, &context)?;
        let crate_name = required_string(object, "crate_name", &context)?;
        let source = required_string(object, "source", &context)?;
        let working_directory = required_string(object, "working_directory", &context)?;
        if !valid_selector_text(crate_name) || !valid_selector_text(source) {
            return Err(BuildConfigError::Invalid(format!(
                "{context} contains an empty, non-ASCII, control-bearing, or oversized selector"
            )));
        }
        let working_directory =
            absolute_json_path(working_directory, &format!("{context}.working_directory"))?;
        let unit = ConfiguredUnit {
            crate_name: crate_name.to_owned(),
            source: source.to_owned(),
            working_directory: working_directory
                .to_str()
                .expect("JSON paths are UTF-8")
                .to_owned(),
        };
        if units.last().is_some_and(|previous| previous >= &unit) {
            return Err(BuildConfigError::Invalid(
                "units must be strictly ordered by crate_name, source, and working_directory"
                    .to_owned(),
            ));
        }
        units.push(unit);
    }
    Ok(units)
}

fn declared_identity(
    object: &Map<String, Value>,
    context: &str,
) -> Result<ContentIdentityV1, BuildConfigError> {
    let sha256 = decode_sha256(required_string(object, "sha256", context)?, context)?;
    let byte_len = required_u64(object, "byte_len", context)?;
    if byte_len == 0 {
        return Err(BuildConfigError::Invalid(format!(
            "{context}.byte_len must be nonzero"
        )));
    }
    Ok(ContentIdentityV1::from_parts(sha256, byte_len))
}

fn exact_object<'a>(
    value: &'a Value,
    expected: &[&str],
    context: &str,
) -> Result<&'a Map<String, Value>, BuildConfigError> {
    let object = value
        .as_object()
        .ok_or_else(|| BuildConfigError::Invalid(format!("{context} must be an object")))?;
    let keys = object.keys().map(String::as_str).collect::<Vec<_>>();
    if keys != expected {
        return Err(BuildConfigError::Invalid(format!(
            "{context} must contain exactly the fields {expected:?}; found {keys:?}"
        )));
    }
    Ok(object)
}

fn exact_production_root_object(value: &Value) -> Result<&Map<String, Value>, BuildConfigError> {
    let object = value
        .as_object()
        .ok_or_else(|| BuildConfigError::Invalid("configuration must be an object".to_owned()))?;
    let keys = object.keys().map(String::as_str).collect::<Vec<_>>();
    if keys != ROOT_KEYS {
        return Err(BuildConfigError::Invalid(format!(
            "production configuration must contain exactly the fields {ROOT_KEYS:?}; found {keys:?}"
        )));
    }
    Ok(object)
}

fn required_value<'a>(
    object: &'a Map<String, Value>,
    name: &str,
    context: &str,
) -> Result<&'a Value, BuildConfigError> {
    object
        .get(name)
        .ok_or_else(|| BuildConfigError::Invalid(format!("{context} is missing {name:?}")))
}

fn required_string<'a>(
    object: &'a Map<String, Value>,
    name: &str,
    context: &str,
) -> Result<&'a str, BuildConfigError> {
    required_value(object, name, context)?
        .as_str()
        .ok_or_else(|| BuildConfigError::Invalid(format!("{context}.{name} must be a string")))
}

fn required_u64(
    object: &Map<String, Value>,
    name: &str,
    context: &str,
) -> Result<u64, BuildConfigError> {
    required_value(object, name, context)?
        .as_u64()
        .ok_or_else(|| {
            BuildConfigError::Invalid(format!(
                "{context}.{name} must be an unsigned 64-bit integer"
            ))
        })
}

fn absolute_json_path(value: &str, context: &str) -> Result<PathBuf, BuildConfigError> {
    if value.len() > MAX_CONFIG_PATH_BYTES || value.as_bytes().contains(&0) {
        return Err(BuildConfigError::Invalid(format!(
            "{context}.path is empty or exceeds the path bound"
        )));
    }
    let path = PathBuf::from(value);
    require_absolute_path(&path, context)?;
    Ok(path)
}

fn require_absolute_path(path: &Path, context: &str) -> Result<(), BuildConfigError> {
    let bytes = path.as_os_str().as_encoded_bytes();
    if !path.is_absolute()
        || bytes.is_empty()
        || bytes.len() > MAX_CONFIG_PATH_BYTES
        || bytes.contains(&0)
    {
        return Err(BuildConfigError::Invalid(format!(
            "{context} path must be a bounded absolute path"
        )));
    }
    Ok(())
}

fn read_bounded(
    path: &Path,
    maximum: usize,
    kind: &'static str,
) -> Result<Vec<u8>, BuildConfigError> {
    let mut file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_CLOEXEC | libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)
        .map_err(|error| BuildConfigError::Io {
            kind,
            path: path.to_owned(),
            error,
        })?;
    let initial = file.metadata().map_err(|error| BuildConfigError::Io {
        kind,
        path: path.to_owned(),
        error,
    })?;
    let initial_len = usize::try_from(initial.len()).ok();
    if !initial.file_type().is_file()
        || initial_len.is_none_or(|length| length == 0 || length > maximum)
    {
        return Err(BuildConfigError::Invalid(format!(
            "Worker V2 {kind} {} must be a regular file containing 1..={maximum} bytes",
            path.display()
        )));
    }
    let mut bytes = Vec::with_capacity(initial_len.expect("validated bounded length"));
    Read::by_ref(&mut file)
        .take((maximum + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|error| BuildConfigError::Io {
            kind,
            path: path.to_owned(),
            error,
        })?;
    let final_metadata = file.metadata().map_err(|error| BuildConfigError::Io {
        kind,
        path: path.to_owned(),
        error,
    })?;
    if Some(bytes.len()) != initial_len
        || final_metadata.dev() != initial.dev()
        || final_metadata.ino() != initial.ino()
        || final_metadata.mode() != initial.mode()
        || final_metadata.nlink() != initial.nlink()
        || final_metadata.len() != initial.len()
        || final_metadata.mtime() != initial.mtime()
        || final_metadata.mtime_nsec() != initial.mtime_nsec()
        || final_metadata.ctime() != initial.ctime()
        || final_metadata.ctime_nsec() != initial.ctime_nsec()
    {
        return Err(BuildConfigError::Invalid(format!(
            "Worker V2 {kind} {} changed while it was read",
            path.display()
        )));
    }
    Ok(bytes)
}

fn decode_sha256(value: &str, context: &str) -> Result<[u8; 32], BuildConfigError> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(BuildConfigError::Invalid(format!(
            "{context}.sha256 must be exactly 64 lowercase hexadecimal digits"
        )));
    }
    let mut bytes = [0_u8; 32];
    for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
        bytes[index] = (hex_value(pair[0]) << 4) | hex_value(pair[1]);
    }
    Ok(bytes)
}

fn hex_value(byte: u8) -> u8 {
    match byte {
        b'0'..=b'9' => byte - b'0',
        b'a'..=b'f' => byte - b'a' + 10,
        _ => unreachable!("validated lowercase hexadecimal digit"),
    }
}

fn valid_selector_text(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_CONFIG_PATH_BYTES
        && value.is_ascii()
        && !value.bytes().any(|byte| byte.is_ascii_control())
}
