//! Explicit cargo-side configuration for the narrow Worker V2 handoff flow.

use std::error::Error;
use std::ffi::OsStr;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use fe2o3_artifact_transaction::ConsumedCompilerModuleHandoffV1;
use fe2o3_hsaco_finalize::{
    ContentIdentityV1, FirstBuildWorkerV2Error, InertFirstBuildWorkerV2EvidenceV1, LinkOptionV1,
    LinkPlanError, MAX_LINK_INPUTS, PinnedWorkerV1, WorkerExecutionError, WorkerExecutionLimitsV1,
    WorkerInputKindV1, WorkerInputV1, WorkerMeasurementV1, WorkerOutputConstraintsV1,
    WorkerProtocolError, execute_reproducible_first_build_worker_v2,
};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

pub(crate) const CODEGEN_PIPELINE_ENV: &str = "FE2O3_CODEGEN_PIPELINE";
pub(crate) const WORKER_V2_CONFIG_ENV: &str = "FE2O3_WORKER_V2_CONFIG_V2";
pub(crate) const WORKER_V2_EXPECTED_ID_ENV: &str = "FE2O3_WORKER_V2_EXPECTED_ID_V1";
const WORKER_V2_PIPELINE: &str = "kernel-ir-worker-v2";
const CONFIG_FORMAT: &str = "fe2o3-worker-v2-config-v2";
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
pub(crate) struct WorkerV2ConfigIdentity([u8; 32]);

impl WorkerV2ConfigIdentity {
    pub(crate) const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    pub(crate) fn to_hex(self) -> String {
        hex(&self.0)
    }

    #[cfg(test)]
    pub(crate) const fn for_test(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct ConfiguredUnit {
    crate_name: String,
    source: String,
    working_directory: String,
}

pub(crate) struct PreparedWorkerV2Config {
    identity: WorkerV2ConfigIdentity,
    worker: PinnedWorkerV1,
    providers: Vec<WorkerInputV1>,
    link_options: Vec<LinkOptionV1>,
    candidate_output: WorkerOutputConstraintsV1,
    limits: WorkerExecutionLimitsV1,
    units: Vec<ConfiguredUnit>,
}

impl PreparedWorkerV2Config {
    pub(crate) fn from_environment() -> Result<Option<Self>, WorkerV2ConfigError> {
        Self::from_selection(
            std::env::var_os(CODEGEN_PIPELINE_ENV).as_deref(),
            std::env::var_os(WORKER_V2_CONFIG_ENV).as_deref(),
        )
    }

    fn from_selection(
        pipeline: Option<&OsStr>,
        config_path: Option<&OsStr>,
    ) -> Result<Option<Self>, WorkerV2ConfigError> {
        let selected = pipeline == Some(OsStr::new(WORKER_V2_PIPELINE));
        match (selected, config_path) {
            (false, None) => Ok(None),
            (false, Some(_)) => Err(WorkerV2ConfigError::UnexpectedConfiguration),
            (true, None) => Err(WorkerV2ConfigError::MissingConfiguration),
            (true, Some(path)) if path.is_empty() => Err(WorkerV2ConfigError::MissingConfiguration),
            (true, Some(path)) => Self::from_manifest(Path::new(path)).map(Some),
        }
    }

    fn from_manifest(path: &Path) -> Result<Self, WorkerV2ConfigError> {
        require_absolute_path(path, "configuration")?;
        let bytes = read_bounded(path, MAX_CONFIG_BYTES, "configuration")?;
        let value: Value = serde_json::from_slice(&bytes)
            .map_err(|error| WorkerV2ConfigError::Json(error.to_string()))?;
        let canonical = serde_json::to_vec(&value)
            .map_err(|error| WorkerV2ConfigError::Json(error.to_string()))?;
        if canonical != bytes {
            return Err(WorkerV2ConfigError::Invalid(
                "configuration must be compact canonical JSON with lexicographically ordered object keys"
                    .to_owned(),
            ));
        }

        let root = exact_object(&value, ROOT_KEYS, "configuration")?;
        if required_string(root, "format", "configuration")? != CONFIG_FORMAT {
            return Err(WorkerV2ConfigError::Invalid(format!(
                "configuration format must be exactly {CONFIG_FORMAT:?}"
            )));
        }

        let worker = prepare_worker(required_value(root, "worker", "configuration")?)?;
        let providers = prepare_providers(required_value(root, "providers", "configuration")?)?;
        let link_options =
            parse_link_options(required_value(root, "link_options", "configuration")?)?;
        let candidate_output = WorkerOutputConstraintsV1::new(required_u64(
            root,
            "candidate_output_max_bytes",
            "configuration",
        )?)
        .map_err(WorkerV2ConfigError::Protocol)?;
        let limits = parse_limits(required_value(root, "limits", "configuration")?)?;
        let units = parse_units(required_value(root, "units", "configuration")?)?;

        let identity = transitive_identity(&bytes, &worker, &providers);
        Ok(Self {
            identity,
            worker,
            providers,
            link_options,
            candidate_output,
            limits,
            units,
        })
    }

    pub(crate) const fn identity(&self) -> WorkerV2ConfigIdentity {
        self.identity
    }

    pub(crate) fn selects(
        &self,
        crate_name: &str,
        source: &Path,
        working_directory: &Path,
    ) -> bool {
        let (Some(source), Some(working_directory)) = (source.to_str(), working_directory.to_str())
        else {
            return false;
        };
        self.units
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

    pub(crate) fn execute(
        &self,
        consumed: ConsumedCompilerModuleHandoffV1,
    ) -> Result<InertFirstBuildWorkerV2EvidenceV1, FirstBuildWorkerV2Error> {
        execute_reproducible_first_build_worker_v2(
            consumed,
            &self.worker,
            self.providers.clone(),
            self.link_options.clone(),
            self.candidate_output.clone(),
            self.limits,
        )
    }
}

fn transitive_identity(
    manifest: &[u8],
    worker: &PinnedWorkerV1,
    providers: &[WorkerInputV1],
) -> WorkerV2ConfigIdentity {
    let mut hash = Sha256::new();
    update_identity(&mut hash, b"fe2o3-worker-v2-transitive-config-v1");
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
    WorkerV2ConfigIdentity(hash.finalize().into())
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
pub(crate) enum WorkerV2ConfigError {
    MissingConfiguration,
    UnexpectedConfiguration,
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

impl fmt::Display for WorkerV2ConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingConfiguration => write!(
                formatter,
                "{CODEGEN_PIPELINE_ENV}={WORKER_V2_PIPELINE} requires {WORKER_V2_CONFIG_ENV}"
            ),
            Self::UnexpectedConfiguration => write!(
                formatter,
                "{WORKER_V2_CONFIG_ENV} is valid only with {CODEGEN_PIPELINE_ENV}={WORKER_V2_PIPELINE}"
            ),
            Self::Io { kind, path, error } => {
                write!(
                    formatter,
                    "failed to read Worker V2 {kind} {}: {error}",
                    path.display()
                )
            }
            Self::Json(error) => write!(formatter, "invalid Worker V2 configuration JSON: {error}"),
            Self::Invalid(reason) => write!(formatter, "invalid Worker V2 configuration: {reason}"),
            Self::LinkPlan(error) => write!(formatter, "invalid Worker V2 link option: {error}"),
            Self::Protocol(error) => write!(formatter, "invalid Worker V2 protocol input: {error}"),
            Self::Worker(error) => write!(formatter, "invalid Worker V2 measurement: {error}"),
        }
    }
}

impl Error for WorkerV2ConfigError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io { error, .. } => Some(error),
            Self::LinkPlan(error) => Some(error),
            Self::Protocol(error) => Some(error),
            Self::Worker(error) => Some(error),
            Self::MissingConfiguration
            | Self::UnexpectedConfiguration
            | Self::Json(_)
            | Self::Invalid(_) => None,
        }
    }
}

fn prepare_worker(value: &Value) -> Result<PinnedWorkerV1, WorkerV2ConfigError> {
    let object = exact_object(value, WORKER_KEYS, "worker")?;
    let path = absolute_json_path(required_string(object, "path", "worker")?, "worker")?;
    let identity = declared_identity(object, "worker")?;
    let measurement = WorkerMeasurementV1::new(
        identity,
        required_string(object, "worker_build_identity", "worker")?,
        required_string(object, "llvm_build_identity", "worker")?,
    )
    .map_err(WorkerV2ConfigError::Worker)?;
    PinnedWorkerV1::open(path, measurement).map_err(WorkerV2ConfigError::Worker)
}

fn prepare_providers(value: &Value) -> Result<Vec<WorkerInputV1>, WorkerV2ConfigError> {
    let values = value
        .as_array()
        .ok_or_else(|| WorkerV2ConfigError::Invalid("providers must be an array".to_owned()))?;
    if values.len() >= MAX_LINK_INPUTS {
        return Err(WorkerV2ConfigError::Invalid(format!(
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
            return Err(WorkerV2ConfigError::Invalid(
                "providers must be strictly ordered by declared content identity".to_owned(),
            ));
        }
        previous = Some(identity);
        let kind = match required_string(object, "kind", &context)? {
            "llvm-bitcode" => WorkerInputKindV1::LlvmBitcode,
            "amdgpu-relocatable" => WorkerInputKindV1::AmdGpuRelocatable,
            "llvm-text-ir" => WorkerInputKindV1::LlvmTextIr,
            other => {
                return Err(WorkerV2ConfigError::Invalid(format!(
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
                .map_err(WorkerV2ConfigError::Protocol)?,
        );
    }
    Ok(providers)
}

fn parse_link_options(value: &Value) -> Result<Vec<LinkOptionV1>, WorkerV2ConfigError> {
    let values = value
        .as_array()
        .ok_or_else(|| WorkerV2ConfigError::Invalid("link_options must be an array".to_owned()))?;
    if values.len() != REQUIRED_OPTIONS.len() {
        return Err(WorkerV2ConfigError::Invalid(
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
            return Err(WorkerV2ConfigError::Invalid(format!(
                "{context} must be {expected_name:?} with one of {allowed_values:?}"
            )));
        }
        options.push(LinkOptionV1::new(name, option_value).map_err(WorkerV2ConfigError::LinkPlan)?);
    }
    Ok(options)
}

fn parse_limits(value: &Value) -> Result<WorkerExecutionLimitsV1, WorkerV2ConfigError> {
    let object = exact_object(value, LIMIT_KEYS, "limits")?;
    let timeout_ms = required_u64(object, "timeout_ms", "limits")?;
    let stdout_bytes = usize::try_from(required_u64(object, "stdout_bytes", "limits")?)
        .map_err(|_| WorkerV2ConfigError::Invalid("limits.stdout_bytes is too large".to_owned()))?;
    let stderr_bytes = usize::try_from(required_u64(object, "stderr_bytes", "limits")?)
        .map_err(|_| WorkerV2ConfigError::Invalid("limits.stderr_bytes is too large".to_owned()))?;
    WorkerExecutionLimitsV1::new(
        Duration::from_millis(timeout_ms),
        stdout_bytes,
        stderr_bytes,
    )
    .map_err(WorkerV2ConfigError::Worker)
}

fn parse_units(value: &Value) -> Result<Vec<ConfiguredUnit>, WorkerV2ConfigError> {
    let values = value
        .as_array()
        .ok_or_else(|| WorkerV2ConfigError::Invalid("units must be an array".to_owned()))?;
    if values.is_empty() || values.len() > 1024 {
        return Err(WorkerV2ConfigError::Invalid(
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
            return Err(WorkerV2ConfigError::Invalid(format!(
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
            return Err(WorkerV2ConfigError::Invalid(
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
) -> Result<ContentIdentityV1, WorkerV2ConfigError> {
    let sha256 = decode_sha256(required_string(object, "sha256", context)?, context)?;
    let byte_len = required_u64(object, "byte_len", context)?;
    if byte_len == 0 {
        return Err(WorkerV2ConfigError::Invalid(format!(
            "{context}.byte_len must be nonzero"
        )));
    }
    Ok(ContentIdentityV1::from_parts(sha256, byte_len))
}

fn exact_object<'a>(
    value: &'a Value,
    expected: &[&str],
    context: &str,
) -> Result<&'a Map<String, Value>, WorkerV2ConfigError> {
    let object = value
        .as_object()
        .ok_or_else(|| WorkerV2ConfigError::Invalid(format!("{context} must be an object")))?;
    let keys = object.keys().map(String::as_str).collect::<Vec<_>>();
    if keys != expected {
        return Err(WorkerV2ConfigError::Invalid(format!(
            "{context} must contain exactly the fields {expected:?}; found {keys:?}"
        )));
    }
    Ok(object)
}

fn required_value<'a>(
    object: &'a Map<String, Value>,
    name: &str,
    context: &str,
) -> Result<&'a Value, WorkerV2ConfigError> {
    object
        .get(name)
        .ok_or_else(|| WorkerV2ConfigError::Invalid(format!("{context} is missing {name:?}")))
}

fn required_string<'a>(
    object: &'a Map<String, Value>,
    name: &str,
    context: &str,
) -> Result<&'a str, WorkerV2ConfigError> {
    required_value(object, name, context)?
        .as_str()
        .ok_or_else(|| WorkerV2ConfigError::Invalid(format!("{context}.{name} must be a string")))
}

fn required_u64(
    object: &Map<String, Value>,
    name: &str,
    context: &str,
) -> Result<u64, WorkerV2ConfigError> {
    required_value(object, name, context)?
        .as_u64()
        .ok_or_else(|| {
            WorkerV2ConfigError::Invalid(format!(
                "{context}.{name} must be an unsigned 64-bit integer"
            ))
        })
}

fn absolute_json_path(value: &str, context: &str) -> Result<PathBuf, WorkerV2ConfigError> {
    if value.len() > MAX_CONFIG_PATH_BYTES || value.as_bytes().contains(&0) {
        return Err(WorkerV2ConfigError::Invalid(format!(
            "{context}.path is empty or exceeds the path bound"
        )));
    }
    let path = PathBuf::from(value);
    require_absolute_path(&path, context)?;
    Ok(path)
}

fn require_absolute_path(path: &Path, context: &str) -> Result<(), WorkerV2ConfigError> {
    if !path.is_absolute() || path.as_os_str().is_empty() {
        return Err(WorkerV2ConfigError::Invalid(format!(
            "{context} path must be absolute"
        )));
    }
    Ok(())
}

fn read_bounded(
    path: &Path,
    maximum: usize,
    kind: &'static str,
) -> Result<Vec<u8>, WorkerV2ConfigError> {
    let bytes = fs::read(path).map_err(|error| WorkerV2ConfigError::Io {
        kind,
        path: path.to_owned(),
        error,
    })?;
    if bytes.is_empty() || bytes.len() > maximum {
        return Err(WorkerV2ConfigError::Invalid(format!(
            "Worker V2 {kind} {} must contain 1..={maximum} bytes",
            path.display()
        )));
    }
    Ok(bytes)
}

fn decode_sha256(value: &str, context: &str) -> Result<[u8; 32], WorkerV2ConfigError> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(WorkerV2ConfigError::Invalid(format!(
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_TEST: AtomicU64 = AtomicU64::new(1);

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "cargo-fe2o3-worker-v2-config-{}-{}",
                std::process::id(),
                NEXT_TEST.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|byte| format!("{byte:02x}")).collect()
    }

    fn manifest(directory: &TestDirectory) -> PathBuf {
        let worker = std::env::current_exe().unwrap();
        let worker_bytes = fs::read(&worker).unwrap();
        let provider = directory.0.join("provider.o");
        fs::write(&provider, b"provider").unwrap();
        let provider_identity = ContentIdentityV1::calculate(b"provider");
        let worker_identity = ContentIdentityV1::calculate(&worker_bytes);
        let value = json!({
            "candidate_output_max_bytes": 4096,
            "format": CONFIG_FORMAT,
            "limits": {
                "stderr_bytes": 1024,
                "stdout_bytes": 16384,
                "timeout_ms": 2000
            },
            "link_options": [
                {"name": "code-object-version", "value": "6"},
                {"name": "opt-level", "value": "2"},
                {"name": "strip-debug", "value": "true"},
                {"name": "verify-each", "value": "true"}
            ],
            "providers": [{
                "byte_len": provider_identity.byte_len(),
                "kind": "amdgpu-relocatable",
                "path": provider,
                "sha256": hex(provider_identity.sha256())
            }],
            "units": [{
                "crate_name": "kernel",
                "source": "src/lib.rs",
                "working_directory": directory.0
            }],
            "worker": {
                "byte_len": worker_identity.byte_len(),
                "llvm_build_identity": "llvm-test-v1",
                "path": worker,
                "sha256": hex(worker_identity.sha256()),
                "worker_build_identity": "worker-test-v1"
            }
        });
        let path = directory.0.join("config.json");
        fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
        path
    }

    #[test]
    fn requires_configuration_exactly_for_the_worker_v2_pipeline() {
        assert!(
            PreparedWorkerV2Config::from_selection(None, None)
                .unwrap()
                .is_none()
        );
        assert!(matches!(
            PreparedWorkerV2Config::from_selection(Some(OsStr::new(WORKER_V2_PIPELINE)), None),
            Err(WorkerV2ConfigError::MissingConfiguration)
        ));
        assert!(matches!(
            PreparedWorkerV2Config::from_selection(None, Some(OsStr::new("/config"))),
            Err(WorkerV2ConfigError::UnexpectedConfiguration)
        ));
    }

    #[test]
    fn prepares_exact_measured_inputs_and_stable_manifest_identity() {
        let directory = TestDirectory::new();
        let path = manifest(&directory);
        let first = PreparedWorkerV2Config::from_manifest(&path).unwrap();
        let second = PreparedWorkerV2Config::from_manifest(&path).unwrap();
        assert_eq!(first.identity(), second.identity());
        assert_eq!(first.providers.len(), 1);
        assert_eq!(first.link_options.len(), 4);
        assert!(first.selects("kernel", Path::new("src/lib.rs"), &directory.0));
        assert!(!first.selects("host", Path::new("src/lib.rs"), &directory.0));
    }

    #[test]
    fn rejects_noncanonical_json_and_unknown_fields() {
        let directory = TestDirectory::new();
        let path = manifest(&directory);
        let mut bytes = fs::read(&path).unwrap();
        bytes.push(b'\n');
        fs::write(&path, bytes).unwrap();
        assert!(matches!(
            PreparedWorkerV2Config::from_manifest(&path),
            Err(WorkerV2ConfigError::Invalid(_))
        ));

        let path = manifest(&directory);
        let mut value: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        value["unknown"] = Value::Bool(true);
        fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
        assert!(matches!(
            PreparedWorkerV2Config::from_manifest(&path),
            Err(WorkerV2ConfigError::Invalid(_))
        ));
    }

    #[test]
    fn rejects_mismatched_worker_and_provider_identities_before_execution() {
        let directory = TestDirectory::new();
        let path = manifest(&directory);
        let mut value: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        value["worker"]["sha256"] = Value::String("00".repeat(32));
        fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
        assert!(matches!(
            PreparedWorkerV2Config::from_manifest(&path),
            Err(WorkerV2ConfigError::Worker(_))
        ));

        let path = manifest(&directory);
        let mut value: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        value["providers"][0]["sha256"] = Value::String("00".repeat(32));
        fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
        assert!(matches!(
            PreparedWorkerV2Config::from_manifest(&path),
            Err(WorkerV2ConfigError::Protocol(
                WorkerProtocolError::ContentIdentityMismatch
            ))
        ));
    }

    #[test]
    fn rejects_noncanonical_options_and_paths() {
        let directory = TestDirectory::new();
        let path = manifest(&directory);
        let mut value: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        value["link_options"][0]["name"] = Value::String("opt-level".to_owned());
        fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
        assert!(matches!(
            PreparedWorkerV2Config::from_manifest(&path),
            Err(WorkerV2ConfigError::Invalid(_))
        ));

        assert!(matches!(
            PreparedWorkerV2Config::from_manifest(Path::new("relative.json")),
            Err(WorkerV2ConfigError::Invalid(_))
        ));
    }
}
