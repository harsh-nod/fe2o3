//! Explicit cargo-side configuration for the narrow Worker V2 handoff flow.

use std::error::Error;
use std::ffi::OsStr;
use std::fmt;
#[cfg(test)]
use std::fs;
use std::fs::{File, OpenOptions};
use std::io::Read;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::production_release::{
    ExactRowSoftmaxV1CaseV1, RowSoftmaxV1MaskProfileV1, RowSoftmaxV1ReleaseWorkloadV1,
};
use crate::protected_compiler_handoff_v3::ParentConsumedCompilerModuleHandoffV3;
use fe2o3_artifact_transaction::{
    CompilerModuleHandoffReceiptV3, ConsumedCompilerModuleHandoffV1,
    ConsumedCompilerModuleHandoffV2,
};
use fe2o3_build_authority::CompilerClosureV2;
use fe2o3_compiler_ffi::InertSemanticCompilerModuleHandoffV3;
use fe2o3_hsaco_finalize::{
    ContentIdentityV1, FirstBuildWorkerV2Error, InertFirstBuildWorkerV2EvidenceV1,
    InertProtectedFirstBuildWorkerV2EvidenceV1, InertProtectedFirstBuildWorkerV3EvidenceV1,
    LinkOptionV1, LinkPlanError, MAX_LINK_INPUTS, PinnedWorkerV1,
    PreparedProtectedFirstBuildWorkerV3PreflightV1, ProtectedFirstBuildWorkerV2Error,
    ProtectedFirstBuildWorkerV3Error, ROW_SOFTMAX_V1_PROVIDER_ITEM_COUNT,
    RowSoftmaxV1DirectWorkerPinsV1, RowSoftmaxV1OcmlProviderPinsV1, RowSoftmaxV1ProviderManifestV1,
    WorkerExecutionError, WorkerExecutionLimitsV1, WorkerInputKindV1, WorkerInputV1,
    WorkerMeasurementV1, WorkerOutputConstraintsV1, WorkerProtocolError,
    execute_preflighted_protected_reproducible_first_build_worker_v3,
    execute_protected_reproducible_first_build_worker_v2,
    execute_protected_reproducible_first_build_worker_v3,
    execute_reproducible_first_build_worker_v2,
    preflight_protected_reproducible_first_build_worker_v3,
};
use fe2o3_verifier::{
    GENERAL_GEMM_RUNTIME_CLOSURE_V2_MANIFEST_SHA256, MAX_GENERAL_GEMM_PROOF_TIMEOUT_SECONDS_V1,
};
use fe2o3_worker_v2_bundle::{MAX_WORKER_V2_ENVELOPE_INPUTS_BYTES, WorkerV2EnvelopeInputsV1};
use rustix::fs::{FileType, Mode, OFlags, fstat, open};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

pub(crate) use crate::worker_v2_restart::WorkerV2EnvelopeModeV1;

pub(crate) const QUALIFICATION_ORACLE_ENV: &str = "FE2O3_QUALIFICATION_ORACLE_V1";
const OBSOLETE_CODEGEN_PIPELINE_ENV: &str = "FE2O3_CODEGEN_PIPELINE";
pub(crate) const WORKER_V2_CONFIG_ENV: &str = "FE2O3_WORKER_V2_CONFIG_V2";
pub(crate) const WORKER_V2_EXPECTED_ID_ENV: &str = "FE2O3_WORKER_V2_EXPECTED_ID_V1";
pub(crate) const GENERAL_GEMM_RUNTIME_CLOSURE_V2_ROOT_ENV: &str =
    "FE2O3_GENERAL_GEMM_RUNTIME_CLOSURE_V2_ROOT";
pub(crate) const GENERAL_GEMM_RUNTIME_CLOSURE_V2_MANIFEST_SHA256_ENV: &str =
    "FE2O3_GENERAL_GEMM_RUNTIME_CLOSURE_V2_MANIFEST_SHA256";
pub(crate) const WORKER_V2_SOURCE_DEBUG_PROFILE_ENV: &str =
    "FE2O3_WORKER_V2_SOURCE_DEBUG_PROFILE_V1";
const WORKER_V2_PIPELINE: &str = "kernel-ir-worker-v2";
const PRODUCTION_CONFIG_PROFILE_ID_V1: &str = "production-v1";
pub(crate) const OBSOLETE_PRODUCTION_SELECTOR: &str = PRODUCTION_CONFIG_PROFILE_ID_V1;
const SCALAR_GEMM_V1_PIPELINE: &str = "collected-scalar-gemm-v1";
const ROW_SOFTMAX_V1_PIPELINE: &str = "collected-row-softmax-v1";
pub(crate) const GENERAL_GEMM_V1_PIPELINE: &str = "collected-general-gemm-v1";
const CONFIG_FORMAT: &str = "fe2o3-worker-v2-config-v2";
const S09_ALPHA_DEBUG_PROFILE: &str = "s09-alpha-gfx942-o0-v1";
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
const ROOT_KEYS_WITH_ENVELOPE: &[&str] = &[
    "candidate_output_max_bytes",
    "format",
    "limits",
    "link_options",
    "load_envelope",
    "load_envelope_inputs",
    "providers",
    "units",
    "worker",
];
const ROOT_KEYS_WITH_ENVELOPE_MODE: &[&str] = &[
    "candidate_output_max_bytes",
    "format",
    "limits",
    "link_options",
    "load_envelope",
    "providers",
    "units",
    "worker",
];
const ROOT_KEYS_WITH_ENVELOPE_INPUTS: &[&str] = &[
    "candidate_output_max_bytes",
    "format",
    "limits",
    "link_options",
    "load_envelope_inputs",
    "providers",
    "units",
    "worker",
];
const ENVELOPE_INPUT_KEYS: &[&str] = &["byte_len", "path", "sha256"];
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
const ROW_SOFTMAX_V1_KEYS: &[&str] = &[
    "case",
    "comparison_policy",
    "mask",
    "ocml_file_sha256",
    "ocml_manifest_sha256",
    "provider_crate_hash",
    "provider_definition_identities",
    "provider_source_identities",
    "provider_stable_crate_id",
    "row_elements",
];
const GENERAL_GEMM_V1_KEYS: &[&str] = &[
    "profile",
    "proof_timeout_seconds",
    "runtime_closure_v2_manifest_sha256",
    "runtime_closure_v2_root",
];
const GENERAL_GEMM_QUALIFICATION_PAIR_PROFILE_V1: &str = "qualification-pair-v1";
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
    manifest_path: PathBuf,
    identity: WorkerV2ConfigIdentity,
    profile: WorkerConfigProfile,
    envelope_mode: WorkerV2EnvelopeModeV1,
    envelope_inputs: Option<ConfiguredEnvelopeInputs>,
    worker: PinnedWorkerV1,
    providers: Vec<WorkerInputV1>,
    link_options: Vec<LinkOptionV1>,
    candidate_output: WorkerOutputConstraintsV1,
    limits: WorkerExecutionLimitsV1,
    source_debug_profile: Option<WorkerV2SourceDebugProfileV1>,
    row_softmax_v1: Option<PreparedRowSoftmaxV1Config>,
    general_gemm_v1: Option<PreparedGeneralGemmV1Config>,
    units: Vec<ConfiguredUnit>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PreparedGeneralGemmV1Config {
    runtime_closure_v2_root: PathBuf,
    runtime_closure_v2_manifest_sha256: [u8; 32],
    proof_timeout_seconds: u32,
}

impl PreparedGeneralGemmV1Config {
    pub(crate) fn runtime_closure_v2_root(&self) -> &Path {
        &self.runtime_closure_v2_root
    }

    pub(crate) const fn runtime_closure_v2_manifest_sha256(&self) -> [u8; 32] {
        self.runtime_closure_v2_manifest_sha256
    }

    pub(crate) const fn proof_timeout_seconds(&self) -> u32 {
        self.proof_timeout_seconds
    }
}

pub(crate) struct PreparedRowSoftmaxV1Config {
    provider: RowSoftmaxV1ProviderManifestV1,
    ocml: RowSoftmaxV1OcmlProviderPinsV1,
    case: ExactRowSoftmaxV1CaseV1,
    row_elements: u32,
    mask: RowSoftmaxV1MaskProfileV1,
    comparison_policy: String,
}

impl PreparedRowSoftmaxV1Config {
    pub(crate) const fn provider(&self) -> RowSoftmaxV1ProviderManifestV1 {
        self.provider
    }

    pub(crate) fn workload(&self) -> RowSoftmaxV1ReleaseWorkloadV1<'_> {
        RowSoftmaxV1ReleaseWorkloadV1 {
            case: self.case,
            row_elements: self.row_elements,
            mask: self.mask,
            comparison_policy: &self.comparison_policy,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct WorkerV2BuildObservation<'a> {
    pub(crate) config_identity: WorkerV2ConfigIdentity,
    pub(crate) executable_sha256: [u8; 32],
    pub(crate) worker_build_identity: &'a str,
    pub(crate) llvm_build_identity: &'a str,
    pub(crate) prepared_rustc_command_sha256: [u8; 32],
    pub(crate) cargo_fe2o3_executable_sha256: [u8; 32],
    pub(crate) declared_cargo_executable_sha256: [u8; 32],
    pub(crate) pinned_cargo_image_sha256: [u8; 32],
    pub(crate) observed_parent_pid: u64,
    pub(crate) observed_parent_start_time_ticks: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum WorkerV2SourceDebugProfileV1 {
    S09AlphaGfx942O0,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum WorkerConfigProfile {
    General,
    Production,
    ScalarGemmV1,
    RowSoftmaxV1,
    GeneralGemmV1,
}

impl WorkerConfigProfile {
    fn from_environment_value(value: &OsStr) -> Option<Self> {
        if value == WORKER_V2_PIPELINE {
            Some(Self::General)
        } else if value == SCALAR_GEMM_V1_PIPELINE {
            Some(Self::ScalarGemmV1)
        } else if value == ROW_SOFTMAX_V1_PIPELINE {
            Some(Self::RowSoftmaxV1)
        } else if value == GENERAL_GEMM_V1_PIPELINE {
            Some(Self::GeneralGemmV1)
        } else {
            None
        }
    }

    const fn environment_value(self) -> &'static str {
        match self {
            Self::General => WORKER_V2_PIPELINE,
            Self::Production => PRODUCTION_CONFIG_PROFILE_ID_V1,
            Self::ScalarGemmV1 => SCALAR_GEMM_V1_PIPELINE,
            Self::RowSoftmaxV1 => ROW_SOFTMAX_V1_PIPELINE,
            Self::GeneralGemmV1 => GENERAL_GEMM_V1_PIPELINE,
        }
    }
}

/// Matches the backend's closed route rule: only unset means production.
pub(crate) fn production_compilation_selected(profile: Option<&OsStr>) -> bool {
    profile.is_none()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum WorkerV2CompileEnvironmentProfileV1 {
    ProductionGfx942,
    S09AlphaGfx942O0,
    ScalarGemmV1Gfx942,
    RowSoftmaxV1Gfx942,
    GeneralGemmV1Gfx942,
}

impl WorkerV2SourceDebugProfileV1 {
    pub(crate) const fn env_value(self) -> &'static str {
        match self {
            Self::S09AlphaGfx942O0 => S09_ALPHA_DEBUG_PROFILE,
        }
    }
}

#[derive(Clone, Debug)]
struct ConfiguredEnvelopeInputs {
    path: PathBuf,
    expected: ContentIdentityV1,
    pinned: Option<Box<WorkerV2EnvelopeInputsV1>>,
}

impl PreparedWorkerV2Config {
    pub(crate) fn from_environment() -> Result<Option<Self>, WorkerV2ConfigError> {
        Self::from_environment_values(
            std::env::var_os(OBSOLETE_CODEGEN_PIPELINE_ENV).as_deref(),
            std::env::var_os(QUALIFICATION_ORACLE_ENV).as_deref(),
            std::env::var_os(WORKER_V2_CONFIG_ENV).as_deref(),
        )
    }

    fn from_environment_values(
        obsolete_pipeline: Option<&OsStr>,
        qualification_oracle: Option<&OsStr>,
        config_path: Option<&OsStr>,
    ) -> Result<Option<Self>, WorkerV2ConfigError> {
        if let Some(value) = obsolete_pipeline {
            return Err(WorkerV2ConfigError::Invalid(format!(
                "{OBSOLETE_CODEGEN_PIPELINE_ENV} has been removed; production compilation has no selector and temporary test oracles use {QUALIFICATION_ORACLE_ENV}; found {value:?}"
            )));
        }
        Self::from_selection(qualification_oracle, config_path)
    }

    pub(crate) fn from_environment_for_cargo_setup() -> Result<Option<Self>, WorkerV2ConfigError> {
        let mut prepared = Self::from_environment()?;
        if let Some(config) = prepared.as_mut() {
            config.pin_envelope_inputs()?;
        }
        Ok(prepared)
    }

    fn from_selection(
        profile: Option<&OsStr>,
        config_path: Option<&OsStr>,
    ) -> Result<Option<Self>, WorkerV2ConfigError> {
        if profile == Some(OsStr::new(OBSOLETE_PRODUCTION_SELECTOR)) {
            return Err(WorkerV2ConfigError::Invalid(format!(
                "{QUALIFICATION_ORACLE_ENV} must be unset for production compilation; explicit `{OBSOLETE_PRODUCTION_SELECTOR}` selection has been removed"
            )));
        }
        let selected = if production_compilation_selected(profile) {
            Some(WorkerConfigProfile::Production)
        } else {
            profile.and_then(WorkerConfigProfile::from_environment_value)
        };
        match (selected, config_path) {
            (None, None) => Ok(None),
            (Some(WorkerConfigProfile::RowSoftmaxV1), None) => Ok(None),
            (None, Some(_)) => Err(WorkerV2ConfigError::UnexpectedConfiguration),
            (Some(_), None) => Err(WorkerV2ConfigError::MissingConfiguration),
            (Some(_), Some(path)) if path.is_empty() => {
                Err(WorkerV2ConfigError::MissingConfiguration)
            }
            (Some(profile), Some(path)) => {
                Self::from_manifest_for_profile(Path::new(path), profile).map(Some)
            }
        }
    }

    #[cfg(test)]
    fn from_manifest(path: &Path) -> Result<Self, WorkerV2ConfigError> {
        Self::from_manifest_for_profile(path, WorkerConfigProfile::General)
    }

    fn from_manifest_for_profile(
        path: &Path,
        profile: WorkerConfigProfile,
    ) -> Result<Self, WorkerV2ConfigError> {
        require_absolute_path(path, "configuration")?;
        if profile == WorkerConfigProfile::GeneralGemmV1 {
            require_closed_child_manifest_path(path, "configuration")?;
        }
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

        let root = exact_root_object(&value)?;
        if required_string(root, "format", "configuration")? != CONFIG_FORMAT {
            return Err(WorkerV2ConfigError::Invalid(format!(
                "configuration format must be exactly {CONFIG_FORMAT:?}"
            )));
        }

        let worker = prepare_worker(required_value(root, "worker", "configuration")?)?;
        let providers = prepare_providers(required_value(root, "providers", "configuration")?)?;
        let link_options =
            parse_link_options(required_value(root, "link_options", "configuration")?)?;
        let source_debug_profile = parse_source_debug_profile(root, &link_options)?;
        let candidate_output = WorkerOutputConstraintsV1::new(required_u64(
            root,
            "candidate_output_max_bytes",
            "configuration",
        )?)
        .map_err(WorkerV2ConfigError::Protocol)?;
        let limits = parse_limits(required_value(root, "limits", "configuration")?)?;
        let units = parse_units(required_value(root, "units", "configuration")?)?;
        let (envelope_mode, envelope_inputs) = parse_envelope_inputs(root)?;
        let row_softmax_v1 = parse_row_softmax_v1(
            root,
            profile,
            &providers,
            &link_options,
            source_debug_profile,
            envelope_mode,
            &candidate_output,
        )?;
        let general_gemm_v1 = parse_general_gemm_v1(
            root,
            profile,
            &providers,
            &link_options,
            source_debug_profile,
            envelope_mode,
            &candidate_output,
        )?;

        let identity = transitive_identity(
            profile,
            &bytes,
            &worker,
            &providers,
            envelope_inputs.as_ref(),
        );
        Ok(Self {
            manifest_path: path.to_path_buf(),
            identity,
            profile,
            envelope_mode,
            envelope_inputs,
            worker,
            providers,
            link_options,
            candidate_output,
            limits,
            source_debug_profile,
            row_softmax_v1,
            general_gemm_v1,
            units,
        })
    }

    pub(crate) const fn identity(&self) -> WorkerV2ConfigIdentity {
        self.identity
    }

    pub(crate) fn manifest_path(&self) -> &Path {
        &self.manifest_path
    }

    pub(crate) const fn envelope_mode(&self) -> WorkerV2EnvelopeModeV1 {
        self.envelope_mode
    }

    pub(crate) const fn source_debug_profile(&self) -> Option<WorkerV2SourceDebugProfileV1> {
        self.source_debug_profile
    }

    pub(crate) const fn requires_expected_identity(&self) -> bool {
        self.source_debug_profile.is_some()
            || matches!(
                self.profile,
                WorkerConfigProfile::ScalarGemmV1
                    | WorkerConfigProfile::RowSoftmaxV1
                    | WorkerConfigProfile::GeneralGemmV1
            )
    }

    pub(crate) const fn row_softmax_v1(&self) -> Option<&PreparedRowSoftmaxV1Config> {
        self.row_softmax_v1.as_ref()
    }

    pub(crate) const fn general_gemm_v1(&self) -> Option<&PreparedGeneralGemmV1Config> {
        self.general_gemm_v1.as_ref()
    }

    pub(crate) const fn executes_worker_in_rustc(&self) -> bool {
        matches!(self.profile, WorkerConfigProfile::GeneralGemmV1)
    }

    pub(crate) const fn is_production_compilation(&self) -> bool {
        matches!(self.profile, WorkerConfigProfile::Production)
    }

    pub(crate) fn row_softmax_v1_worker_pins(
        &self,
    ) -> Result<RowSoftmaxV1DirectWorkerPinsV1, WorkerV2ConfigError> {
        let row = self.row_softmax_v1.as_ref().ok_or_else(|| {
            WorkerV2ConfigError::Invalid(
                "row-softmax worker pins requested from a different profile".to_owned(),
            )
        })?;
        let measurement = self.worker.measurement();
        RowSoftmaxV1DirectWorkerPinsV1::new(
            measurement.executable(),
            measurement.worker_build_identity(),
            measurement.llvm_build_identity(),
            row.ocml,
        )
        .map_err(|error| WorkerV2ConfigError::Invalid(error.to_string()))
    }

    pub(crate) fn compile_environment_profile(
        &self,
        crate_name: &str,
        source: &Path,
        working_directory: &Path,
    ) -> Option<WorkerV2CompileEnvironmentProfileV1> {
        if !self.selects(crate_name, source, working_directory) {
            return None;
        }
        if self.profile == WorkerConfigProfile::Production {
            Some(WorkerV2CompileEnvironmentProfileV1::ProductionGfx942)
        } else if self.source_debug_profile.is_some() {
            Some(WorkerV2CompileEnvironmentProfileV1::S09AlphaGfx942O0)
        } else if self.profile == WorkerConfigProfile::ScalarGemmV1 {
            Some(WorkerV2CompileEnvironmentProfileV1::ScalarGemmV1Gfx942)
        } else if self.profile == WorkerConfigProfile::RowSoftmaxV1 {
            Some(WorkerV2CompileEnvironmentProfileV1::RowSoftmaxV1Gfx942)
        } else if self.profile == WorkerConfigProfile::GeneralGemmV1 {
            Some(WorkerV2CompileEnvironmentProfileV1::GeneralGemmV1Gfx942)
        } else {
            None
        }
    }

    pub(crate) fn build_observation(
        &self,
        prepared_rustc_command_sha256: [u8; 32],
        cargo_fe2o3_executable_sha256: [u8; 32],
        declared_cargo_executable_sha256: [u8; 32],
        pinned_cargo_image_sha256: [u8; 32],
        observed_parent_pid: u64,
        observed_parent_start_time_ticks: u64,
    ) -> WorkerV2BuildObservation<'_> {
        let measurement = self.worker.measurement();
        WorkerV2BuildObservation {
            config_identity: self.identity,
            executable_sha256: *measurement.executable().sha256(),
            worker_build_identity: measurement.worker_build_identity(),
            llvm_build_identity: measurement.llvm_build_identity(),
            prepared_rustc_command_sha256,
            cargo_fe2o3_executable_sha256,
            declared_cargo_executable_sha256,
            pinned_cargo_image_sha256,
            observed_parent_pid,
            observed_parent_start_time_ticks,
        }
    }

    pub(crate) fn load_envelope_inputs(
        &self,
    ) -> Result<Option<WorkerV2EnvelopeInputsV1>, WorkerV2ConfigError> {
        self.envelope_inputs
            .as_ref()
            .map(ConfiguredEnvelopeInputs::load)
            .transpose()
    }

    fn pin_envelope_inputs(&mut self) -> Result<(), WorkerV2ConfigError> {
        let Some(configured) = self.envelope_inputs.as_mut() else {
            return Ok(());
        };
        if configured.pinned.is_none() {
            configured.pinned = Some(Box::new(configured.read_exact()?));
        }
        Ok(())
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

    pub(crate) fn execute_protected(
        &self,
        consumed: ConsumedCompilerModuleHandoffV2,
    ) -> Result<InertProtectedFirstBuildWorkerV2EvidenceV1, ProtectedFirstBuildWorkerV2Error> {
        execute_protected_reproducible_first_build_worker_v2(
            consumed,
            &self.worker,
            self.providers.clone(),
            self.link_options.clone(),
            self.candidate_output.clone(),
            self.limits,
        )
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn execute_protected_v3(
        &self,
        parent_consumed: ParentConsumedCompilerModuleHandoffV3,
    ) -> Result<InertProtectedFirstBuildWorkerV3EvidenceV1, ProtectedFirstBuildWorkerV3Error> {
        let (receipt, consumed, expected_compiler_closure) = parent_consumed.into_parts();
        execute_protected_reproducible_first_build_worker_v3(
            consumed,
            receipt,
            expected_compiler_closure,
            &self.worker,
            self.providers.clone(),
            self.link_options.clone(),
            self.candidate_output.clone(),
            self.limits,
        )
    }

    pub(crate) fn preflight_protected_v3(
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
            &self.worker,
            self.providers.clone(),
            self.link_options.clone(),
            self.candidate_output.clone(),
            self.limits,
        )
    }

    pub(crate) fn execute_preflighted_protected_v3(
        &self,
        parent_consumed: ParentConsumedCompilerModuleHandoffV3,
        preflight: PreparedProtectedFirstBuildWorkerV3PreflightV1,
    ) -> Result<InertProtectedFirstBuildWorkerV3EvidenceV1, ProtectedFirstBuildWorkerV3Error> {
        let (_, consumed, _) = parent_consumed.into_parts();
        execute_preflighted_protected_reproducible_first_build_worker_v3(
            consumed,
            preflight,
            &self.worker,
        )
    }
}

fn parse_general_gemm_v1(
    root: &Map<String, Value>,
    profile: WorkerConfigProfile,
    providers: &[WorkerInputV1],
    options: &[LinkOptionV1],
    source_debug_profile: Option<WorkerV2SourceDebugProfileV1>,
    envelope_mode: WorkerV2EnvelopeModeV1,
    candidate_output: &WorkerOutputConstraintsV1,
) -> Result<Option<PreparedGeneralGemmV1Config>, WorkerV2ConfigError> {
    let Some(value) = root.get("general_gemm_v1") else {
        if profile == WorkerConfigProfile::GeneralGemmV1 {
            return Err(WorkerV2ConfigError::Invalid(
                "general-GEMM Worker V2 configuration requires general_gemm_v1 qualification-pair pins"
                    .to_owned(),
            ));
        }
        return Ok(None);
    };
    if profile != WorkerConfigProfile::GeneralGemmV1 {
        return Err(WorkerV2ConfigError::Invalid(
            "general_gemm_v1 pins are valid only for collected-general-gemm-v1".to_owned(),
        ));
    }
    if !providers.is_empty() {
        return Err(WorkerV2ConfigError::Invalid(
            "general-GEMM synchronous worker rejects request-side link providers".to_owned(),
        ));
    }
    if source_debug_profile.is_some() || envelope_mode != WorkerV2EnvelopeModeV1::NonAuthoritative {
        return Err(WorkerV2ConfigError::Invalid(
            "general-GEMM synchronous worker rejects source-debug and load-envelope fields"
                .to_owned(),
        ));
    }
    let option = |name: &str| {
        options
            .iter()
            .find(|option| option.name() == name)
            .map(LinkOptionV1::value)
    };
    if option("code-object-version") != Some("6")
        || option("opt-level") != Some("2")
        || option("strip-debug") != Some("true")
        || option("verify-each") != Some("true")
    {
        return Err(WorkerV2ConfigError::Invalid(
            "general-GEMM production policy requires COV6, O2, stripped debug, and verify-each"
                .to_owned(),
        ));
    }
    if candidate_output.max_bytes() != fe2o3_hsaco::MAX_HSACO_BYTES as u64 {
        return Err(WorkerV2ConfigError::Invalid(format!(
            "general-GEMM candidate_output_max_bytes must be exactly {}",
            fe2o3_hsaco::MAX_HSACO_BYTES
        )));
    }
    let object = exact_object(value, GENERAL_GEMM_V1_KEYS, "general_gemm_v1")?;
    let profile = required_string(object, "profile", "general_gemm_v1")?;
    if profile != GENERAL_GEMM_QUALIFICATION_PAIR_PROFILE_V1 {
        return Err(WorkerV2ConfigError::Invalid(format!(
            "general_gemm_v1.profile has unsupported value {profile:?}"
        )));
    }
    let proof_timeout_seconds = required_u64(object, "proof_timeout_seconds", "general_gemm_v1")?;
    if proof_timeout_seconds == 0
        || proof_timeout_seconds > u64::from(MAX_GENERAL_GEMM_PROOF_TIMEOUT_SECONDS_V1)
    {
        return Err(WorkerV2ConfigError::Invalid(format!(
            "general_gemm_v1.proof_timeout_seconds must be in 1..={MAX_GENERAL_GEMM_PROOF_TIMEOUT_SECONDS_V1}"
        )));
    }
    let runtime_closure_v2_root = absolute_json_path(
        required_string(object, "runtime_closure_v2_root", "general_gemm_v1")?,
        "general_gemm_v1.runtime_closure_v2_root",
    )?;
    require_closed_child_manifest_path(
        &runtime_closure_v2_root,
        "general_gemm_v1.runtime_closure_v2_root",
    )?;
    let runtime_closure_v2_manifest_sha256 = decode_sha256(
        required_string(
            object,
            "runtime_closure_v2_manifest_sha256",
            "general_gemm_v1",
        )?,
        "general_gemm_v1.runtime_closure_v2_manifest_sha256",
    )?;
    if runtime_closure_v2_manifest_sha256 != GENERAL_GEMM_RUNTIME_CLOSURE_V2_MANIFEST_SHA256 {
        return Err(WorkerV2ConfigError::Invalid(
            "general_gemm_v1.runtime_closure_v2_manifest_sha256 differs from the compiled-in reviewed manifest"
                .to_owned(),
        ));
    }
    Ok(Some(PreparedGeneralGemmV1Config {
        runtime_closure_v2_root,
        runtime_closure_v2_manifest_sha256,
        proof_timeout_seconds: proof_timeout_seconds as u32,
    }))
}

fn parse_row_softmax_v1(
    root: &Map<String, Value>,
    profile: WorkerConfigProfile,
    providers: &[WorkerInputV1],
    options: &[LinkOptionV1],
    source_debug_profile: Option<WorkerV2SourceDebugProfileV1>,
    envelope_mode: WorkerV2EnvelopeModeV1,
    candidate_output: &WorkerOutputConstraintsV1,
) -> Result<Option<PreparedRowSoftmaxV1Config>, WorkerV2ConfigError> {
    let Some(value) = root.get("row_softmax_v1") else {
        if profile == WorkerConfigProfile::RowSoftmaxV1 {
            return Err(WorkerV2ConfigError::Invalid(
                "row-softmax Worker V2 configuration requires row_softmax_v1 policy pins"
                    .to_owned(),
            ));
        }
        return Ok(None);
    };
    if profile != WorkerConfigProfile::RowSoftmaxV1 {
        return Err(WorkerV2ConfigError::Invalid(
            "row_softmax_v1 policy pins are valid only for collected-row-softmax-v1".to_owned(),
        ));
    }
    if !providers.is_empty() {
        return Err(WorkerV2ConfigError::Invalid(
            "row-softmax direct worker rejects request-side link providers".to_owned(),
        ));
    }
    if source_debug_profile.is_some() || envelope_mode != WorkerV2EnvelopeModeV1::NonAuthoritative {
        return Err(WorkerV2ConfigError::Invalid(
            "row-softmax production policy rejects source-debug and generic load envelopes"
                .to_owned(),
        ));
    }
    let required_option = |name: &str, value: &str| {
        options
            .iter()
            .any(|option| option.name() == name && option.value() == value)
    };
    if !required_option("code-object-version", "6")
        || !required_option("opt-level", "0")
        || !required_option("strip-debug", "true")
        || !required_option("verify-each", "true")
    {
        return Err(WorkerV2ConfigError::Invalid(
            "row-softmax production policy requires COV6, O0, stripped debug, and verify-each"
                .to_owned(),
        ));
    }
    if candidate_output.max_bytes() != fe2o3_hsaco::MAX_HSACO_BYTES as u64 {
        return Err(WorkerV2ConfigError::Invalid(format!(
            "row-softmax candidate_output_max_bytes must be exactly {}",
            fe2o3_hsaco::MAX_HSACO_BYTES
        )));
    }

    let object = exact_object(value, ROW_SOFTMAX_V1_KEYS, "row_softmax_v1")?;
    let case = match required_string(object, "case", "row_softmax_v1")? {
        "normal" => ExactRowSoftmaxV1CaseV1::Normal,
        "equal" => ExactRowSoftmaxV1CaseV1::Equal,
        "dominant" => ExactRowSoftmaxV1CaseV1::Dominant,
        "exceptional" => ExactRowSoftmaxV1CaseV1::Exceptional,
        other => {
            return Err(WorkerV2ConfigError::Invalid(format!(
                "row_softmax_v1.case has unsupported value {other:?}"
            )));
        }
    };
    let mask = match required_string(object, "mask", "row_softmax_v1")? {
        "unmasked" => RowSoftmaxV1MaskProfileV1::Unmasked,
        "alternating" => RowSoftmaxV1MaskProfileV1::Alternating,
        other => {
            return Err(WorkerV2ConfigError::Invalid(format!(
                "row_softmax_v1.mask has unsupported value {other:?}"
            )));
        }
    };
    let row_elements = u32::try_from(required_u64(object, "row_elements", "row_softmax_v1")?)
        .map_err(|_| {
            WorkerV2ConfigError::Invalid("row_softmax_v1.row_elements exceeds u32".to_owned())
        })?;
    let comparison_policy = required_string(object, "comparison_policy", "row_softmax_v1")?;
    if comparison_policy.is_empty()
        || comparison_policy.len() > 128
        || !comparison_policy.is_ascii()
    {
        return Err(WorkerV2ConfigError::Invalid(
            "row_softmax_v1.comparison_policy is not bounded ASCII".to_owned(),
        ));
    }
    let provider = RowSoftmaxV1ProviderManifestV1::new(
        required_u64(object, "provider_stable_crate_id", "row_softmax_v1")?,
        decode_fixed_hex(
            required_string(object, "provider_crate_hash", "row_softmax_v1")?,
            "row_softmax_v1.provider_crate_hash",
        )?,
        decode_identity_array::<ROW_SOFTMAX_V1_PROVIDER_ITEM_COUNT, 16>(
            required_value(object, "provider_definition_identities", "row_softmax_v1")?,
            "row_softmax_v1.provider_definition_identities",
        )?,
        decode_identity_array::<ROW_SOFTMAX_V1_PROVIDER_ITEM_COUNT, 32>(
            required_value(object, "provider_source_identities", "row_softmax_v1")?,
            "row_softmax_v1.provider_source_identities",
        )?,
    )
    .map_err(|error| WorkerV2ConfigError::Invalid(error.to_string()))?;
    let ocml_files = decode_identity_array::<4, 32>(
        required_value(object, "ocml_file_sha256", "row_softmax_v1")?,
        "row_softmax_v1.ocml_file_sha256",
    )?;
    let ocml = RowSoftmaxV1OcmlProviderPinsV1::new(
        ocml_files,
        decode_fixed_hex(
            required_string(object, "ocml_manifest_sha256", "row_softmax_v1")?,
            "row_softmax_v1.ocml_manifest_sha256",
        )?,
    )
    .map_err(|error| WorkerV2ConfigError::Invalid(error.to_string()))?;
    Ok(Some(PreparedRowSoftmaxV1Config {
        provider,
        ocml,
        case,
        row_elements,
        mask,
        comparison_policy: comparison_policy.to_owned(),
    }))
}

fn parse_envelope_inputs(
    root: &Map<String, Value>,
) -> Result<(WorkerV2EnvelopeModeV1, Option<ConfiguredEnvelopeInputs>), WorkerV2ConfigError> {
    let mode = match root.get("load_envelope") {
        None => WorkerV2EnvelopeModeV1::NonAuthoritative,
        Some(Value::String(value)) if value == "required" => WorkerV2EnvelopeModeV1::Required,
        Some(_) => Err(WorkerV2ConfigError::Invalid(
            "configuration.load_envelope must be exactly \"required\" when present".to_owned(),
        ))?,
    };
    match (mode, root.get("load_envelope_inputs")) {
        (WorkerV2EnvelopeModeV1::NonAuthoritative, None) => Ok((mode, None)),
        (WorkerV2EnvelopeModeV1::NonAuthoritative, Some(_)) => Err(WorkerV2ConfigError::Invalid(
            "configuration.load_envelope_inputs is valid only when load_envelope is \"required\""
                .to_owned(),
        )),
        (WorkerV2EnvelopeModeV1::Required, None) => Err(WorkerV2ConfigError::Invalid(
            "configuration.load_envelope=\"required\" requires load_envelope_inputs".to_owned(),
        )),
        (WorkerV2EnvelopeModeV1::Required, Some(value)) => {
            let object = exact_object(value, ENVELOPE_INPUT_KEYS, "load_envelope_inputs")?;
            let path = absolute_json_path(
                required_string(object, "path", "load_envelope_inputs")?,
                "load_envelope_inputs",
            )?;
            let expected = declared_identity(object, "load_envelope_inputs")?;
            let declared_len = usize::try_from(expected.byte_len()).map_err(|_| {
                WorkerV2ConfigError::Invalid(
                    "load_envelope_inputs.byte_len exceeds the platform bound".to_owned(),
                )
            })?;
            if declared_len > MAX_WORKER_V2_ENVELOPE_INPUTS_BYTES {
                return Err(WorkerV2ConfigError::Invalid(format!(
                    "load_envelope_inputs.byte_len exceeds {MAX_WORKER_V2_ENVELOPE_INPUTS_BYTES} bytes"
                )));
            }
            Ok((
                mode,
                Some(ConfiguredEnvelopeInputs {
                    path,
                    expected,
                    pinned: None,
                }),
            ))
        }
    }
}

impl ConfiguredEnvelopeInputs {
    fn load(&self) -> Result<WorkerV2EnvelopeInputsV1, WorkerV2ConfigError> {
        if let Some(inputs) = &self.pinned {
            return Ok(inputs.as_ref().clone());
        }
        self.read_exact()
    }

    fn read_exact(&self) -> Result<WorkerV2EnvelopeInputsV1, WorkerV2ConfigError> {
        let declared_len = usize::try_from(self.expected.byte_len()).map_err(|_| {
            WorkerV2ConfigError::Invalid(
                "load_envelope_inputs.byte_len exceeds the platform bound".to_owned(),
            )
        })?;
        let bytes = read_measured_private(&self.path, declared_len, "envelope input capsule")?;
        let measured: [u8; 32] = Sha256::digest(&bytes).into();
        if self.expected.sha256() != &measured {
            return Err(WorkerV2ConfigError::Invalid(
                "load_envelope_inputs.sha256 does not match the exact capsule bytes".to_owned(),
            ));
        }
        let inputs = WorkerV2EnvelopeInputsV1::from_bytes(&bytes).map_err(|error| {
            WorkerV2ConfigError::Invalid(format!(
                "load_envelope_inputs is not a canonical capsule: {error}"
            ))
        })?;
        if inputs.to_bytes() != bytes {
            return Err(WorkerV2ConfigError::Invalid(
                "load_envelope_inputs capsule encoding is not canonical".to_owned(),
            ));
        }
        Ok(inputs)
    }
}

fn transitive_identity(
    profile: WorkerConfigProfile,
    manifest: &[u8],
    worker: &PinnedWorkerV1,
    providers: &[WorkerInputV1],
    envelope_inputs: Option<&ConfiguredEnvelopeInputs>,
) -> WorkerV2ConfigIdentity {
    let mut hash = Sha256::new();
    update_identity(&mut hash, b"fe2o3-worker-v2-transitive-config-v2");
    update_identity(&mut hash, profile.environment_value().as_bytes());
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
    if let Some(inputs) = envelope_inputs {
        update_identity(&mut hash, &[1]);
        update_identity(&mut hash, inputs.expected.sha256());
        update_identity(&mut hash, &inputs.expected.byte_len().to_le_bytes());
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
                "a Worker V2 codegen profile requires {WORKER_V2_CONFIG_ENV}"
            ),
            Self::UnexpectedConfiguration => write!(
                formatter,
                "{WORKER_V2_CONFIG_ENV} is valid only when {QUALIFICATION_ORACLE_ENV} is unset for production compilation or exactly {WORKER_V2_PIPELINE}, {SCALAR_GEMM_V1_PIPELINE}, {ROW_SOFTMAX_V1_PIPELINE}, or {GENERAL_GEMM_V1_PIPELINE}"
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

fn parse_source_debug_profile(
    root: &Map<String, Value>,
    options: &[LinkOptionV1],
) -> Result<Option<WorkerV2SourceDebugProfileV1>, WorkerV2ConfigError> {
    let Some(value) = root.get("source_debug_profile") else {
        return Ok(None);
    };
    if value.as_str() != Some(S09_ALPHA_DEBUG_PROFILE) {
        return Err(WorkerV2ConfigError::Invalid(format!(
            "configuration.source_debug_profile must be exactly {S09_ALPHA_DEBUG_PROFILE:?}"
        )));
    }
    let option = |name: &str| {
        options
            .iter()
            .find(|option| option.name() == name)
            .map(LinkOptionV1::value)
    };
    if option("code-object-version") != Some("6")
        || option("opt-level") != Some("0")
        || option("strip-debug") != Some("false")
        || option("verify-each") != Some("true")
    {
        return Err(WorkerV2ConfigError::Invalid(
            "the S09 alpha source-debug profile requires code-object-version=6, opt-level=0, strip-debug=false, and verify-each=true"
                .to_owned(),
        ));
    }
    Ok(Some(WorkerV2SourceDebugProfileV1::S09AlphaGfx942O0))
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

fn exact_root_object(value: &Value) -> Result<&Map<String, Value>, WorkerV2ConfigError> {
    let object = value.as_object().ok_or_else(|| {
        WorkerV2ConfigError::Invalid("configuration must be an object".to_owned())
    })?;
    let keys = object.keys().map(String::as_str).collect::<Vec<_>>();
    let profile_neutral_keys = keys
        .iter()
        .copied()
        .filter(|key| {
            !matches!(
                *key,
                "source_debug_profile" | "row_softmax_v1" | "general_gemm_v1"
            )
        })
        .collect::<Vec<_>>();
    if profile_neutral_keys != ROOT_KEYS
        && profile_neutral_keys != ROOT_KEYS_WITH_ENVELOPE_MODE
        && profile_neutral_keys != ROOT_KEYS_WITH_ENVELOPE_INPUTS
        && profile_neutral_keys != ROOT_KEYS_WITH_ENVELOPE
    {
        return Err(WorkerV2ConfigError::Invalid(format!(
            "configuration contains unknown or duplicate configuration fields; found {keys:?}"
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
    let bytes = path.as_os_str().as_encoded_bytes();
    if !path.is_absolute()
        || bytes.is_empty()
        || bytes.len() > MAX_CONFIG_PATH_BYTES
        || bytes.contains(&0)
    {
        return Err(WorkerV2ConfigError::Invalid(format!(
            "{context} path must be a bounded absolute path"
        )));
    }
    Ok(())
}

fn require_closed_child_manifest_path(
    path: &Path,
    context: &str,
) -> Result<(), WorkerV2ConfigError> {
    let Some(value) = path.to_str() else {
        return Err(WorkerV2ConfigError::Invalid(format!(
            "{context} path must be canonical absolute UTF-8 for the reviewed child"
        )));
    };
    if value != "/"
        && value[1..]
            .split('/')
            .any(|component| component.is_empty() || matches!(component, "." | ".."))
    {
        return Err(WorkerV2ConfigError::Invalid(format!(
            "{context} path must be canonical absolute UTF-8 for the reviewed child"
        )));
    }
    Ok(())
}

fn read_bounded(
    path: &Path,
    maximum: usize,
    kind: &'static str,
) -> Result<Vec<u8>, WorkerV2ConfigError> {
    let mut file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_CLOEXEC | libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)
        .map_err(|error| WorkerV2ConfigError::Io {
            kind,
            path: path.to_owned(),
            error,
        })?;
    let initial = file.metadata().map_err(|error| WorkerV2ConfigError::Io {
        kind,
        path: path.to_owned(),
        error,
    })?;
    let initial_len = usize::try_from(initial.len()).ok();
    if !initial.file_type().is_file()
        || initial_len.is_none_or(|length| length == 0 || length > maximum)
    {
        return Err(WorkerV2ConfigError::Invalid(format!(
            "Worker V2 {kind} {} must be a regular file containing 1..={maximum} bytes",
            path.display()
        )));
    }
    let mut bytes = Vec::with_capacity(initial_len.expect("validated bounded length"));
    Read::by_ref(&mut file)
        .take((maximum + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|error| WorkerV2ConfigError::Io {
            kind,
            path: path.to_owned(),
            error,
        })?;
    let final_metadata = file.metadata().map_err(|error| WorkerV2ConfigError::Io {
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
        return Err(WorkerV2ConfigError::Invalid(format!(
            "Worker V2 {kind} {} changed while it was read",
            path.display()
        )));
    }
    Ok(bytes)
}

fn read_measured_private(
    path: &Path,
    exact_len: usize,
    kind: &'static str,
) -> Result<Vec<u8>, WorkerV2ConfigError> {
    if exact_len == 0 || exact_len > MAX_WORKER_V2_ENVELOPE_INPUTS_BYTES {
        return Err(WorkerV2ConfigError::Invalid(format!(
            "Worker V2 {kind} {} has an invalid declared length",
            path.display()
        )));
    }
    let descriptor = open(
        path,
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC | OFlags::NONBLOCK,
        Mode::empty(),
    )
    .map_err(|error| WorkerV2ConfigError::Io {
        kind,
        path: path.to_owned(),
        error: std::io::Error::from(error),
    })?;
    let initial = fstat(&descriptor).map_err(|error| WorkerV2ConfigError::Io {
        kind,
        path: path.to_owned(),
        error: std::io::Error::from(error),
    })?;
    if FileType::from_raw_mode(initial.st_mode) != FileType::RegularFile
        || initial.st_nlink != 1
        || initial.st_mode & 0o077 != 0
        || usize::try_from(initial.st_size).ok() != Some(exact_len)
    {
        return Err(WorkerV2ConfigError::Invalid(format!(
            "Worker V2 {kind} {} must be one private, single-link regular file of the declared size",
            path.display()
        )));
    }
    let mut file = File::from(descriptor);
    let mut bytes = Vec::with_capacity(exact_len.saturating_add(1));
    Read::by_ref(&mut file)
        .take((exact_len + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|error| WorkerV2ConfigError::Io {
            kind,
            path: path.to_owned(),
            error,
        })?;
    let final_stat = fstat(&file).map_err(|error| WorkerV2ConfigError::Io {
        kind,
        path: path.to_owned(),
        error: std::io::Error::from(error),
    })?;
    if bytes.len() != exact_len
        || final_stat.st_dev != initial.st_dev
        || final_stat.st_ino != initial.st_ino
        || final_stat.st_mode != initial.st_mode
        || final_stat.st_nlink != 1
        || final_stat.st_mtime != initial.st_mtime
        || final_stat.st_mtime_nsec != initial.st_mtime_nsec
        || final_stat.st_ctime != initial.st_ctime
        || final_stat.st_ctime_nsec != initial.st_ctime_nsec
        || final_stat.st_mode & 0o077 != 0
        || usize::try_from(final_stat.st_size).ok() != Some(exact_len)
    {
        return Err(WorkerV2ConfigError::Invalid(format!(
            "Worker V2 {kind} {} changed while it was measured",
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

fn decode_fixed_hex<const N: usize>(
    value: &str,
    context: &str,
) -> Result<[u8; N], WorkerV2ConfigError> {
    if value.len() != N * 2
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(WorkerV2ConfigError::Invalid(format!(
            "{context} must be exactly {} lowercase hexadecimal digits",
            N * 2
        )));
    }
    let mut bytes = [0_u8; N];
    for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
        bytes[index] = (hex_value(pair[0]) << 4) | hex_value(pair[1]);
    }
    Ok(bytes)
}

fn decode_identity_array<const COUNT: usize, const WIDTH: usize>(
    value: &Value,
    context: &str,
) -> Result<[[u8; WIDTH]; COUNT], WorkerV2ConfigError> {
    let values = value
        .as_array()
        .filter(|values| values.len() == COUNT)
        .ok_or_else(|| {
            WorkerV2ConfigError::Invalid(format!(
                "{context} must contain exactly {COUNT} hexadecimal identities"
            ))
        })?;
    let mut result = [[0_u8; WIDTH]; COUNT];
    for (index, value) in values.iter().enumerate() {
        let value = value.as_str().ok_or_else(|| {
            WorkerV2ConfigError::Invalid(format!("{context}[{index}] must be a string"))
        })?;
        result[index] = decode_fixed_hex(value, &format!("{context}[{index}]"))?;
    }
    Ok(result)
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
    use fe2o3_hsaco_finalize::ROW_SOFTMAX_V1_UPSTREAM_LLVM_BUILD_IDENTITY_V1;
    use serde_json::json;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_TEST: AtomicU64 = AtomicU64::new(1);

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            fe2o3_artifact_transaction::enable_same_mount_namespace_artifact_path_guard_v1();
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
    fn v1_execute_path_preserves_its_handoff_and_evidence_types() {
        let _execute: fn(
            &PreparedWorkerV2Config,
            ConsumedCompilerModuleHandoffV1,
        )
            -> Result<InertFirstBuildWorkerV2EvidenceV1, FirstBuildWorkerV2Error> =
            PreparedWorkerV2Config::execute;
    }

    #[test]
    fn protected_execute_path_consumes_v2_and_returns_protected_evidence() {
        let _execute: fn(
            &PreparedWorkerV2Config,
            ConsumedCompilerModuleHandoffV2,
        ) -> Result<
            InertProtectedFirstBuildWorkerV2EvidenceV1,
            ProtectedFirstBuildWorkerV2Error,
        > = PreparedWorkerV2Config::execute_protected;
    }

    #[test]
    fn protected_v3_execute_path_retains_parent_receipt_and_native_evidence() {
        let _execute: fn(
            &PreparedWorkerV2Config,
            ParentConsumedCompilerModuleHandoffV3,
        ) -> Result<
            InertProtectedFirstBuildWorkerV3EvidenceV1,
            ProtectedFirstBuildWorkerV3Error,
        > = PreparedWorkerV2Config::execute_protected_v3;

        let _preflight: fn(
            &PreparedWorkerV2Config,
            &InertSemanticCompilerModuleHandoffV3,
            CompilerModuleHandoffReceiptV3,
            CompilerClosureV2,
        ) -> Result<
            PreparedProtectedFirstBuildWorkerV3PreflightV1,
            ProtectedFirstBuildWorkerV3Error,
        > = PreparedWorkerV2Config::preflight_protected_v3;

        let _execute_preflighted: fn(
            &PreparedWorkerV2Config,
            ParentConsumedCompilerModuleHandoffV3,
            PreparedProtectedFirstBuildWorkerV3PreflightV1,
        ) -> Result<
            InertProtectedFirstBuildWorkerV3EvidenceV1,
            ProtectedFirstBuildWorkerV3Error,
        > = PreparedWorkerV2Config::execute_preflighted_protected_v3;
    }

    fn row_softmax_manifest(directory: &TestDirectory) -> PathBuf {
        let generic = manifest(directory);
        let mut value: Value = serde_json::from_slice(&fs::read(&generic).unwrap()).unwrap();
        value["candidate_output_max_bytes"] = json!(fe2o3_hsaco::MAX_HSACO_BYTES);
        value["link_options"][1]["value"] = json!("0");
        value["providers"] = json!([]);
        value["worker"]["llvm_build_identity"] =
            json!(ROW_SOFTMAX_V1_UPSTREAM_LLVM_BUILD_IDENTITY_V1);
        let definitions = (1_u8..=ROW_SOFTMAX_V1_PROVIDER_ITEM_COUNT as u8)
            .map(|value| hex(&[value; 16]))
            .collect::<Vec<_>>();
        let sources = [1_u8, 2, 2, 2, 1, 3, 3, 3].map(|value| hex(&[value; 32]));
        value["row_softmax_v1"] = json!({
            "case": "normal",
            "comparison_policy": crate::production_release::ROW_SOFTMAX_V1_PRODUCTION_POLICY,
            "mask": "unmasked",
            "ocml_file_sha256": [
                hex(&[0x31; 32]),
                hex(&[0x32; 32]),
                hex(&[0x33; 32]),
                hex(&[0x34; 32])
            ],
            "ocml_manifest_sha256": hex(&[0x35; 32]),
            "provider_crate_hash": hex(&[0x21; 16]),
            "provider_definition_identities": definitions,
            "provider_source_identities": sources,
            "provider_stable_crate_id": 7,
            "row_elements": 64
        });
        let path = directory.0.join("row-softmax-config.json");
        fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
        path
    }

    fn general_gemm_manifest(directory: &TestDirectory) -> PathBuf {
        let generic = manifest(directory);
        let mut value: Value = serde_json::from_slice(&fs::read(&generic).unwrap()).unwrap();
        value["candidate_output_max_bytes"] = json!(fe2o3_hsaco::MAX_HSACO_BYTES);
        value["providers"] = json!([]);
        value["general_gemm_v1"] = json!({
            "profile": GENERAL_GEMM_QUALIFICATION_PAIR_PROFILE_V1,
            "proof_timeout_seconds": 120,
            "runtime_closure_v2_manifest_sha256": hex(&GENERAL_GEMM_RUNTIME_CLOSURE_V2_MANIFEST_SHA256),
            "runtime_closure_v2_root": "/opt/fe2o3/verus-runtime-v2/0.2026.08.02"
        });
        let path = directory.0.join("general-gemm-qualification-pair.json");
        fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
        path
    }

    #[test]
    fn requires_configuration_for_production_and_qualification_profiles() {
        assert!(matches!(
            PreparedWorkerV2Config::from_environment_values(
                Some(OsStr::new("production-v1")),
                None,
                None,
            ),
            Err(WorkerV2ConfigError::Invalid(reason))
                if reason.contains("FE2O3_CODEGEN_PIPELINE has been removed")
        ));
        assert!(matches!(
            PreparedWorkerV2Config::from_selection(None, None),
            Err(WorkerV2ConfigError::MissingConfiguration)
        ));
        assert!(matches!(
            PreparedWorkerV2Config::from_selection(Some(OsStr::new(WORKER_V2_PIPELINE)), None),
            Err(WorkerV2ConfigError::MissingConfiguration)
        ));
        assert!(matches!(
            PreparedWorkerV2Config::from_selection(
                Some(OsStr::new(OBSOLETE_PRODUCTION_SELECTOR)),
                None
            ),
            Err(WorkerV2ConfigError::Invalid(reason))
                if reason.contains("must be unset for production compilation")
        ));
        assert!(matches!(
            PreparedWorkerV2Config::from_selection(Some(OsStr::new(SCALAR_GEMM_V1_PIPELINE)), None),
            Err(WorkerV2ConfigError::MissingConfiguration)
        ));
        assert!(matches!(
            PreparedWorkerV2Config::from_selection(
                Some(OsStr::new(GENERAL_GEMM_V1_PIPELINE)),
                None
            ),
            Err(WorkerV2ConfigError::MissingConfiguration)
        ));
        assert!(
            PreparedWorkerV2Config::from_selection(Some(OsStr::new(ROW_SOFTMAX_V1_PIPELINE)), None)
                .unwrap()
                .is_none()
        );
        assert!(matches!(
            PreparedWorkerV2Config::from_selection(
                Some(OsStr::new("legacy-v1")),
                Some(OsStr::new("/config"))
            ),
            Err(WorkerV2ConfigError::UnexpectedConfiguration)
        ));
    }

    #[test]
    fn general_gemm_manifest_selects_only_the_closed_qualification_pair() {
        let directory = TestDirectory::new();
        let path = general_gemm_manifest(&directory);
        let config = PreparedWorkerV2Config::from_manifest_for_profile(
            &path,
            WorkerConfigProfile::GeneralGemmV1,
        )
        .unwrap();
        let pair = config.general_gemm_v1().unwrap();
        assert_eq!(config.manifest_path(), path);
        assert_eq!(
            pair.runtime_closure_v2_root(),
            Path::new("/opt/fe2o3/verus-runtime-v2/0.2026.08.02")
        );
        assert_eq!(
            pair.runtime_closure_v2_manifest_sha256(),
            GENERAL_GEMM_RUNTIME_CLOSURE_V2_MANIFEST_SHA256
        );
        assert_eq!(pair.proof_timeout_seconds(), 120);
        assert!(config.executes_worker_in_rustc());
        assert!(config.requires_expected_identity());
        assert!(config.providers.is_empty());

        let mut value: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        value["general_gemm_v1"]["profile"] = json!("single-schedule-v1");
        fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
        assert!(matches!(
            PreparedWorkerV2Config::from_manifest_for_profile(
                &path,
                WorkerConfigProfile::GeneralGemmV1,
            ),
            Err(WorkerV2ConfigError::Invalid(reason))
                if reason.contains("unsupported value")
        ));
    }

    #[test]
    fn general_gemm_manifest_rejects_profile_and_field_substitution() {
        let directory = TestDirectory::new();
        let path = general_gemm_manifest(&directory);
        let mut value: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        let reference_identity = PreparedWorkerV2Config::from_manifest_for_profile(
            &path,
            WorkerConfigProfile::GeneralGemmV1,
        )
        .unwrap()
        .identity();
        value["general_gemm_v1"]["proof_timeout_seconds"] = json!(121);
        fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
        let substituted = PreparedWorkerV2Config::from_manifest_for_profile(
            &path,
            WorkerConfigProfile::GeneralGemmV1,
        )
        .unwrap();
        assert_eq!(
            substituted
                .general_gemm_v1()
                .map(PreparedGeneralGemmV1Config::proof_timeout_seconds),
            Some(121)
        );
        assert_ne!(substituted.identity(), reference_identity);

        value["general_gemm_v1"]["proof_timeout_seconds"] = json!(120);
        value["general_gemm_v1"]["runtime_closure_v2_root"] =
            json!("/opt/fe2o3/verus-runtime-v2/0.2026.08.02-substituted");
        fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
        let substituted = PreparedWorkerV2Config::from_manifest_for_profile(
            &path,
            WorkerConfigProfile::GeneralGemmV1,
        )
        .unwrap();
        assert_eq!(
            substituted
                .general_gemm_v1()
                .map(PreparedGeneralGemmV1Config::runtime_closure_v2_root),
            Some(Path::new(
                "/opt/fe2o3/verus-runtime-v2/0.2026.08.02-substituted"
            ))
        );
        assert_ne!(substituted.identity(), reference_identity);

        value["general_gemm_v1"]["custom"] = json!(true);
        fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
        assert!(matches!(
            PreparedWorkerV2Config::from_manifest_for_profile(
                &path,
                WorkerConfigProfile::GeneralGemmV1,
            ),
            Err(WorkerV2ConfigError::Invalid(reason))
                if reason.contains("must contain exactly")
        ));

        value["general_gemm_v1"]
            .as_object_mut()
            .unwrap()
            .remove("custom");
        value["general_gemm_v1"]["proof_timeout_seconds"] = json!(0);
        fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
        assert!(matches!(
            PreparedWorkerV2Config::from_manifest_for_profile(
                &path,
                WorkerConfigProfile::GeneralGemmV1,
            ),
            Err(WorkerV2ConfigError::Invalid(reason))
                if reason.contains("proof_timeout_seconds")
        ));

        value["general_gemm_v1"]["proof_timeout_seconds"] = json!(120);
        value["general_gemm_v1"]["runtime_closure_v2_root"] = json!("relative/runtime");
        fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
        assert!(matches!(
            PreparedWorkerV2Config::from_manifest_for_profile(
                &path,
                WorkerConfigProfile::GeneralGemmV1,
            ),
            Err(WorkerV2ConfigError::Invalid(reason))
                if reason.contains("absolute")
        ));

        value["general_gemm_v1"]["runtime_closure_v2_root"] =
            json!("/opt/fe2o3/verus-runtime-v2/0.2026.08.02");
        value["general_gemm_v1"]["runtime_closure_v2_manifest_sha256"] = json!("77".repeat(32));
        fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
        assert!(matches!(
            PreparedWorkerV2Config::from_manifest_for_profile(
                &path,
                WorkerConfigProfile::GeneralGemmV1,
            ),
            Err(WorkerV2ConfigError::Invalid(reason))
                if reason.contains("compiled-in reviewed manifest")
        ));
    }

    #[test]
    fn general_gemm_manifest_rejects_lexical_aliases_and_unbounded_objects() {
        use std::os::unix::fs::symlink;

        let directory = TestDirectory::new();
        let path = general_gemm_manifest(&directory);
        let lexical_alias = path
            .parent()
            .unwrap()
            .join(".")
            .join(path.file_name().unwrap());
        assert!(matches!(
            PreparedWorkerV2Config::from_manifest_for_profile(
                &lexical_alias,
                WorkerConfigProfile::GeneralGemmV1,
            ),
            Err(WorkerV2ConfigError::Invalid(reason)) if reason.contains("canonical absolute UTF-8")
        ));

        let symlink_path = directory.0.join("manifest-link.json");
        symlink(&path, &symlink_path).unwrap();
        assert!(read_bounded(&symlink_path, MAX_CONFIG_BYTES, "configuration").is_err());

        let oversized = directory.0.join("oversized.json");
        File::create(&oversized)
            .unwrap()
            .set_len((MAX_CONFIG_BYTES + 1) as u64)
            .unwrap();
        assert!(read_bounded(&oversized, MAX_CONFIG_BYTES, "configuration").is_err());
    }

    #[test]
    fn row_softmax_manifest_binds_exact_provider_ocml_and_workload_policy() {
        let directory = TestDirectory::new();
        let path = row_softmax_manifest(&directory);
        let config = PreparedWorkerV2Config::from_manifest_for_profile(
            &path,
            WorkerConfigProfile::RowSoftmaxV1,
        )
        .unwrap();
        assert!(config.requires_expected_identity());
        assert!(config.providers.is_empty());
        assert!(config.row_softmax_v1().is_some());
        assert!(config.row_softmax_v1_worker_pins().is_ok());
        let workload = config.row_softmax_v1().unwrap().workload();
        assert_eq!(workload.row_elements, 64);
        assert_eq!(workload.mask, RowSoftmaxV1MaskProfileV1::Unmasked);
        assert_eq!(
            workload.comparison_policy,
            crate::production_release::ROW_SOFTMAX_V1_PRODUCTION_POLICY
        );

        let first_identity = config.identity();
        let mut value: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        value["row_softmax_v1"]["ocml_manifest_sha256"] = json!(hex(&[0x36; 32]));
        fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
        let changed = PreparedWorkerV2Config::from_manifest_for_profile(
            &path,
            WorkerConfigProfile::RowSoftmaxV1,
        )
        .unwrap();
        assert_ne!(first_identity, changed.identity());
    }

    #[test]
    fn row_softmax_manifest_rejects_request_side_provider_and_wrong_finalizer_options() {
        let directory = TestDirectory::new();
        let path = row_softmax_manifest(&directory);
        let mut value: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        value["providers"] = json!([{
            "byte_len": 8,
            "kind": "llvm-bitcode",
            "path": directory.0.join("provider.o"),
            "sha256": hex(ContentIdentityV1::calculate(b"provider").sha256())
        }]);
        fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
        assert!(matches!(
            PreparedWorkerV2Config::from_manifest_for_profile(
                &path,
                WorkerConfigProfile::RowSoftmaxV1
            ),
            Err(WorkerV2ConfigError::Invalid(reason))
                if reason.contains("request-side link providers")
        ));

        let path = row_softmax_manifest(&directory);
        let mut value: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        value["link_options"][0]["value"] = json!("5");
        fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
        assert!(matches!(
            PreparedWorkerV2Config::from_manifest_for_profile(
                &path,
                WorkerConfigProfile::RowSoftmaxV1
            ),
            Err(WorkerV2ConfigError::Invalid(reason))
                if reason.contains("requires COV6")
        ));
    }

    #[test]
    fn prepares_exact_measured_inputs_and_stable_manifest_identity() {
        let directory = TestDirectory::new();
        let path = manifest(&directory);
        let first = PreparedWorkerV2Config::from_manifest(&path).unwrap();
        let second = PreparedWorkerV2Config::from_manifest(&path).unwrap();
        assert_eq!(first.identity(), second.identity());
        assert_eq!(
            first.envelope_mode(),
            WorkerV2EnvelopeModeV1::NonAuthoritative
        );
        assert!(!first.envelope_mode().grants_load_authority());
        assert!(!first.envelope_mode().grants_launch_authority());
        assert_eq!(first.providers.len(), 1);
        assert_eq!(first.link_options.len(), 4);
        assert!(first.selects("kernel", Path::new("src/lib.rs"), &directory.0));
        assert!(!first.selects("host", Path::new("src/lib.rs"), &directory.0));
    }

    #[test]
    fn compile_environment_profile_requires_exact_profile_and_unit_selection() {
        let directory = TestDirectory::new();
        let path = manifest(&directory);
        let general = PreparedWorkerV2Config::from_selection(
            Some(OsStr::new(WORKER_V2_PIPELINE)),
            Some(path.as_os_str()),
        )
        .unwrap()
        .unwrap();
        let production = PreparedWorkerV2Config::from_selection(None, Some(path.as_os_str()))
            .unwrap()
            .unwrap();
        let scalar = PreparedWorkerV2Config::from_selection(
            Some(OsStr::new(SCALAR_GEMM_V1_PIPELINE)),
            Some(path.as_os_str()),
        )
        .unwrap()
        .unwrap();

        assert_ne!(general.identity(), scalar.identity());
        assert_ne!(general.identity(), production.identity());
        assert_ne!(production.identity(), scalar.identity());
        assert_eq!(production.profile, WorkerConfigProfile::Production);
        assert!(!general.requires_expected_identity());
        assert!(!production.requires_expected_identity());
        assert!(scalar.requires_expected_identity());
        assert_eq!(
            general.compile_environment_profile("kernel", Path::new("src/lib.rs"), &directory.0),
            None
        );
        assert_eq!(
            production
                .compile_environment_profile("kernel", Path::new("src/lib.rs"), &directory.0,),
            Some(WorkerV2CompileEnvironmentProfileV1::ProductionGfx942)
        );
        assert_eq!(
            scalar.compile_environment_profile("kernel", Path::new("src/lib.rs"), &directory.0),
            Some(WorkerV2CompileEnvironmentProfileV1::ScalarGemmV1Gfx942)
        );
        for (crate_name, source, working_directory) in [
            ("host", Path::new("src/lib.rs"), directory.0.as_path()),
            ("kernel", Path::new("src/other.rs"), directory.0.as_path()),
            ("kernel", Path::new("src/lib.rs"), Path::new("/other")),
        ] {
            assert_eq!(
                scalar.compile_environment_profile(crate_name, source, working_directory),
                None
            );
        }
    }

    #[test]
    fn admits_only_the_exact_s09_o0_debug_profile() {
        let directory = TestDirectory::new();
        let path = manifest(&directory);
        let mut value: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        value["source_debug_profile"] = Value::String(S09_ALPHA_DEBUG_PROFILE.to_owned());
        value["link_options"][1]["value"] = Value::String("0".to_owned());
        value["link_options"][2]["value"] = Value::String("false".to_owned());
        fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();

        let prepared = PreparedWorkerV2Config::from_manifest(&path).unwrap();
        assert_eq!(
            prepared.source_debug_profile(),
            Some(WorkerV2SourceDebugProfileV1::S09AlphaGfx942O0)
        );
        assert!(prepared.requires_expected_identity());
        assert_eq!(
            prepared.compile_environment_profile("kernel", Path::new("src/lib.rs"), &directory.0),
            Some(WorkerV2CompileEnvironmentProfileV1::S09AlphaGfx942O0)
        );

        value["link_options"][1]["value"] = Value::String("1".to_owned());
        fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
        assert!(matches!(
            PreparedWorkerV2Config::from_manifest(&path),
            Err(WorkerV2ConfigError::Invalid(message))
                if message.contains("requires code-object-version=6, opt-level=0")
        ));

        value["link_options"][1]["value"] = Value::String("0".to_owned());
        value["source_debug_profile"] = Value::String("custom-gdb-command".to_owned());
        fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
        assert!(matches!(
            PreparedWorkerV2Config::from_manifest(&path),
            Err(WorkerV2ConfigError::Invalid(message))
                if message.contains("must be exactly")
        ));
    }

    #[test]
    fn load_envelope_requirement_needs_an_exact_measured_capsule() {
        let directory = TestDirectory::new();
        let path = manifest(&directory);
        PreparedWorkerV2Config::from_manifest(&path).unwrap();
        let mut value: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        value["load_envelope"] = Value::String("required".to_owned());
        fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
        assert!(matches!(
            PreparedWorkerV2Config::from_manifest(&path),
            Err(WorkerV2ConfigError::Invalid(_))
        ));

        value["load_envelope_inputs"]["sha256"] = Value::String("00".repeat(32));
        fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
        assert!(matches!(
            PreparedWorkerV2Config::from_manifest(&path),
            Err(WorkerV2ConfigError::Invalid(_))
        ));

        let capsule = directory.0.join("envelope-inputs.capsule");
        fs::write(&capsule, b"not-a-canonical-capsule").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&capsule, fs::Permissions::from_mode(0o600)).unwrap();
        }
        value["load_envelope_inputs"] = json!({
            "byte_len": 23,
            "path": capsule,
            "sha256": hex(&Sha256::digest(b"not-a-canonical-capsule"))
        });
        fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
        let mut prepared = PreparedWorkerV2Config::from_manifest(&path).unwrap();
        assert!(matches!(
            prepared.pin_envelope_inputs(),
            Err(WorkerV2ConfigError::Invalid(message))
                if message.contains("not a canonical capsule")
        ));

        value["load_envelope"] = Value::String("optional".to_owned());
        fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
        assert!(matches!(
            PreparedWorkerV2Config::from_manifest(&path),
            Err(WorkerV2ConfigError::Invalid(_))
        ));
    }

    #[test]
    #[cfg(unix)]
    fn required_envelope_input_rejects_symlink_truncation_substitution_and_oversize() {
        use std::os::unix::fs::{PermissionsExt, symlink};

        let directory = TestDirectory::new();
        let path = manifest(&directory);
        let target = directory.0.join("capsule-target");
        fs::write(&target, b"capsule").unwrap();
        fs::set_permissions(&target, fs::Permissions::from_mode(0o600)).unwrap();
        let link = directory.0.join("capsule-link");
        symlink(&target, &link).unwrap();
        let mut value: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        value["load_envelope"] = Value::String("required".to_owned());
        value["load_envelope_inputs"] = json!({
            "byte_len": 7,
            "path": link,
            "sha256": hex(&Sha256::digest(b"capsule"))
        });
        fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
        let mut prepared = PreparedWorkerV2Config::from_manifest(&path).unwrap();
        assert!(matches!(
            prepared.pin_envelope_inputs(),
            Err(WorkerV2ConfigError::Io { .. })
        ));

        value["load_envelope_inputs"]["path"] =
            Value::String(target.to_str().expect("temporary path is UTF-8").to_owned());
        value["load_envelope_inputs"]["byte_len"] = Value::from(8);
        fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
        let mut prepared = PreparedWorkerV2Config::from_manifest(&path).unwrap();
        assert!(matches!(
            prepared.pin_envelope_inputs(),
            Err(WorkerV2ConfigError::Invalid(message))
                if message.contains("declared size")
        ));

        value["load_envelope_inputs"]["byte_len"] = Value::from(7);
        fs::write(&target, b"capsulE").unwrap();
        fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
        let mut prepared = PreparedWorkerV2Config::from_manifest(&path).unwrap();
        assert!(matches!(
            prepared.pin_envelope_inputs(),
            Err(WorkerV2ConfigError::Invalid(message))
                if message.contains("sha256 does not match")
        ));

        value["load_envelope_inputs"]["byte_len"] =
            Value::from((MAX_WORKER_V2_ENVELOPE_INPUTS_BYTES as u64) + 1);
        fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
        assert!(matches!(
            PreparedWorkerV2Config::from_manifest(&path),
            Err(WorkerV2ConfigError::Invalid(_))
        ));
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
