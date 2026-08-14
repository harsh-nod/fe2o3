use std::collections::BTreeMap;
use std::error::Error;
use std::ffi::{OsStr, OsString};
use std::fmt;
use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom, Write};
use std::os::fd::{AsRawFd, BorrowedFd, IntoRawFd};
use std::os::unix::fs::MetadataExt;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus};

use fe2o3_artifact_transaction::{
    AttemptScopedHsacoPublicationErrorV1, BackendPublicationReceiptV1,
    BrokeredInvocationCapabilityClaimV1, BuildAttempt, BuildInvocation, BuildSession, EmitError,
    PersistedBackendReceiptV1, ProducerIdentity, RecoveredWorkerV2PublicationIntentV1,
    WorkerV2PublicationIntentErrorV1, begin_build_attempt, clear_worker_v2_publication_intent_v1,
    consume_compiler_module_handoff_v1, fail_build_attempt, finish_build_attempt,
    publish_exact_hsaco_evidence_for_attempt_v1, read_backend_publication_receipt_v1,
    recover_published_hsaco_claim_for_attempt_v1,
};
use fe2o3_hsaco_finalize::inspect_worker_v2_raw_hsaco_v1;
use fe2o3_process_identity::{
    LinuxObjectIdentityV3, ParentPreparedProcessConsistencyV3, PinnedWorkingDirectoryV3,
    parent_prepared_process_consistency_digest_v3,
};
use fe2o3_rustc_invocation::{
    RustcArgsErrorV2, RustcCompileInvocationV2, RustcInvocationV2, classify_rustc_invocation_v2,
    is_rustc_codegen_backend_selector_v2, is_rustc_option_terminator_v2,
};
use fe2o3_worker_v2_bundle::WorkerV2EnvelopeInputsV1;
use reserved_fe2o3_symbols::{
    CRATE_BINDING_ID_ENV_V1, CrateBindingIdV1, derive_crate_binding_id_v1,
};
use sha2::{Digest, Sha256};

use crate::capability_broker;
use crate::inert_rustc_invocation_capture::InertRustcInvocationCaptureV2;
use crate::pinned_codegen_backend::PinnedCodegenBackend;
use crate::pinned_executable::{PinExecutableError, PinnedExecutable};
use crate::project::PinnedDirectory;
use crate::worker_v2::{
    PreparedWorkerV2Config, WORKER_V2_EXPECTED_ID_ENV, WORKER_V2_SOURCE_DEBUG_PROFILE_ENV,
    WorkerV2BuildObservation, WorkerV2CompileEnvironmentProfileV1, WorkerV2ConfigError,
    WorkerV2ConfigIdentity, WorkerV2SourceDebugProfileV1,
};
use crate::worker_v2_artifact_container::assemble_recovered_worker_v2_load_envelope_v1;
#[cfg(feature = "worker-v2-fault-injection-test-only")]
use crate::worker_v2_restart::injected_fault_point_v1;
use crate::worker_v2_restart::{
    PersistedAdmittedWorkerV2IntentV1, RestartIntentErrorV1, ResumeMarkerErrorV1,
    ResumeMarkerStateV1, WorkerV2PublicationKindV1, WorkerV2ResumeStoreV1,
    persist_admitted_worker_v2_intent_v1, recover_worker_v2_intent_v1,
    restart_admission_commitment_with_inputs_v1,
};
use crate::{
    ARTIFACT_CHILD_FD, BACKEND_CHILD_FD, MANAGED_RUSTC_ARGS_ENV, RUSTC_CHILD_FD,
    RUSTC_LIBRARY_CHILD_FD,
};

const HSACO_DIR_ENV: &str = "FE2O3_HSACO_DIR";
const TARGET_ENV: &str = "FE2O3_TARGET";
const VERIFY_KERNEL_IR_ENV: &str = "FE2O3_VERIFY_KERNEL_IR";
const SCALAR_GEMM_V1_PIPELINE: &str = "collected-scalar-gemm-v1";
const ROW_SOFTMAX_V1_PIPELINE: &str = "collected-row-softmax-v1";
const NON_PRODUCTION_REPRODUCTION_RECORD_ENV: &str =
    "FE2O3_NON_PRODUCTION_COMPILER_REPRODUCTION_RECORD_V1";
const BUILD_SESSION_ENV: &str = "FE2O3_BUILD_SESSION_V1";
const BUILD_ATTEMPT_ENV: &str = "FE2O3_BUILD_ATTEMPT_V1";
const CARGO_METADATA_BUILD_OBSERVATION_ENV_V2: &str = "FE2O3_CARGO_METADATA_BUILD_OBSERVATION_V2";
const CODEGEN_BACKEND_BUILD_OBSERVATION_ENV_V2: &str = "FE2O3_CODEGEN_BACKEND_BUILD_OBSERVATION_V2";
const WORKER_CONFIG_BUILD_OBSERVATION_ENV_V2: &str = "FE2O3_WORKER_CONFIG_BUILD_OBSERVATION_V2";
const WORKER_EXECUTABLE_BUILD_OBSERVATION_ENV_V2: &str =
    "FE2O3_WORKER_EXECUTABLE_BUILD_OBSERVATION_V2";
const WORKER_BUILD_IDENTITY_OBSERVATION_ENV_V2: &str = "FE2O3_WORKER_BUILD_IDENTITY_OBSERVATION_V2";
const LLVM_BUILD_IDENTITY_OBSERVATION_ENV_V2: &str = "FE2O3_LLVM_BUILD_IDENTITY_OBSERVATION_V2";
const CARGO_FE2O3_EXECUTABLE_BUILD_OBSERVATION_ENV_V2: &str =
    "FE2O3_CARGO_FE2O3_EXECUTABLE_BUILD_OBSERVATION_V2";
const DECLARED_CARGO_EXECUTABLE_BUILD_OBSERVATION_ENV_V2: &str =
    "FE2O3_DECLARED_CARGO_EXECUTABLE_BUILD_OBSERVATION_V2";
const PINNED_CARGO_IMAGE_BUILD_OBSERVATION_ENV_V2: &str =
    "FE2O3_PINNED_CARGO_IMAGE_BUILD_OBSERVATION_V2";
const OBSERVED_PARENT_PID_BUILD_OBSERVATION_ENV_V2: &str =
    "FE2O3_OBSERVED_PARENT_PID_BUILD_OBSERVATION_V2";
const OBSERVED_PARENT_START_TIME_BUILD_OBSERVATION_ENV_V2: &str =
    "FE2O3_OBSERVED_PARENT_START_TIME_BUILD_OBSERVATION_V2";
const MAX_BUILD_EXECUTABLE_BYTES: u64 = 512 * 1024 * 1024;
const MAX_PROC_STAT_BYTES: usize = 4096;
const PROCESS_CONSISTENCY_EXPECTATION_FD_V3: std::os::fd::RawFd =
    fe2o3_process_identity::S09_PROCESS_CONSISTENCY_EXPECTATION_FD_V3;
const BUILD_ATTEMPT_INPUT_DOMAIN: &[u8] = b"FE2O3/BUILD-ATTEMPT-INPUT/V2\0";
const ROW_SOFTMAX_EFFECTIVE_RUSTC_ARGV_DOMAIN_V1: &[u8] =
    b"FE2O3/ROW-SOFTMAX/EFFECTIVE-RUSTC-ARGV/V1\0";
const CARGO_METADATA_BUILD_OBSERVATION_DOMAIN_V2: &[u8] =
    b"FE2O3/CARGO-METADATA-BUILD-OBSERVATION/V2\0";
const WORKER_V2_CONFIG_ID_DOMAIN: &[u8] = b"FE2O3/WORKER-V2-CONFIG-ID/V1\0";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CompileBuildObservationV2 {
    crate_binding: CrateBindingIdV1,
    cargo_metadata_digest: [u8; 32],
}

impl CompileBuildObservationV2 {
    fn from_ordered_metadata(
        crate_name: &str,
        metadata: &[String],
    ) -> Result<Self, BindingWrapperError> {
        if metadata.is_empty() {
            return Err(BindingWrapperError::MissingMetadata {
                crate_name: crate_name.to_owned(),
            });
        }

        let crate_binding =
            derive_crate_binding_id_v1(crate_name, metadata.iter().map(String::as_str));
        let mut digest = Sha256::new();
        digest.update(CARGO_METADATA_BUILD_OBSERVATION_DOMAIN_V2);
        digest.update((metadata.len() as u64).to_le_bytes());
        for value in metadata {
            digest.update((value.len() as u64).to_le_bytes());
            digest.update(value.as_bytes());
        }

        Ok(Self {
            crate_binding,
            cargo_metadata_digest: digest.finalize().into(),
        })
    }

    fn cargo_metadata_digest_hex(self) -> String {
        const HEX: &[u8; 16] = b"0123456789abcdef";
        let mut encoded = String::with_capacity(self.cargo_metadata_digest.len() * 2);
        for byte in self.cargo_metadata_digest {
            encoded.push(HEX[usize::from(byte >> 4)] as char);
            encoded.push(HEX[usize::from(byte & 0x0f)] as char);
        }
        encoded
    }
}

#[derive(Debug)]
pub(crate) enum BindingWrapperError {
    Arguments(RustcArgsErrorV2),
    MissingMetadata {
        crate_name: String,
    },
    InvalidCodegenOption {
        argument_index: usize,
    },
    EmptyMetadata {
        argument_index: usize,
    },
    MissingManagedEnvironment(&'static str),
    InvalidBuildSession,
    InvalidManagedRustcArguments(&'static str),
    PinnedExecutable(PinExecutableError),
    CapabilityBroker(String),
    ChildCapability(String),
    UninspectableRustcResponseFile {
        argument_index: usize,
    },
    PreexistingCodegenBackend {
        argument_index: usize,
    },
    AuthorityLinkerOverride {
        argument_index: usize,
    },
    OptionTerminatorBeforeManagedArguments {
        argument_index: usize,
    },
    CurrentDirectory(std::io::Error),
    BuildObservation(String),
    WorkerV2Configuration(WorkerV2ConfigError),
    WorkerV2Restart(ResumeMarkerErrorV1),
    Artifact(EmitError),
    ManagedCompletion {
        primary: String,
        cleanup: Option<EmitError>,
    },
    AttemptTermination {
        rustc_status: ExitStatus,
        cleanup: EmitError,
    },
    UnsupportedInvocation,
    Spawn(std::io::Error),
}

impl fmt::Display for BindingWrapperError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Arguments(error) => write!(formatter, "invalid rustc invocation: {error}"),
            Self::MissingMetadata { crate_name } => write!(
                formatter,
                "rustc compile for crate `{crate_name}` has no explicit -C metadata value"
            ),
            Self::InvalidCodegenOption { argument_index } => write!(
                formatter,
                "rustc codegen option at argv[{argument_index}] is not valid UTF-8"
            ),
            Self::EmptyMetadata { argument_index } => write!(
                formatter,
                "rustc metadata value at argv[{argument_index}] is empty"
            ),
            Self::MissingManagedEnvironment(name) => {
                write!(formatter, "managed rustc invocation is missing {name}")
            }
            Self::InvalidBuildSession => formatter
                .write_str("managed rustc invocation has a noncanonical or reserved build session"),
            Self::InvalidManagedRustcArguments(reason) => {
                write!(formatter, "invalid {MANAGED_RUSTC_ARGS_ENV}: {reason}")
            }
            Self::PinnedExecutable(error) => {
                write!(formatter, "failed to pin rustc executable: {error}")
            }
            Self::CapabilityBroker(error) => {
                write!(formatter, "failed to receive managed capabilities: {error}")
            }
            Self::ChildCapability(error) => {
                write!(
                    formatter,
                    "failed to install managed rustc capabilities: {error}"
                )
            }
            Self::UninspectableRustcResponseFile { argument_index } => write!(
                formatter,
                "managed rustc argv[{argument_index}] is an uninspectable response file"
            ),
            Self::PreexistingCodegenBackend { argument_index } => write!(
                formatter,
                "managed rustc argv[{argument_index}] contains a preexisting codegen-backend selector"
            ),
            Self::AuthorityLinkerOverride { argument_index } => write!(
                formatter,
                "authority rustc argv[{argument_index}] contains an unmanaged linker option"
            ),
            Self::OptionTerminatorBeforeManagedArguments { argument_index } => write!(
                formatter,
                "managed rustc argv[{argument_index}] is an option terminator; appended compiler options would be positional inputs"
            ),
            Self::CurrentDirectory(error) => {
                write!(
                    formatter,
                    "failed to resolve rustc working directory: {error}"
                )
            }
            Self::BuildObservation(error) => {
                write!(formatter, "failed to collect build observation: {error}")
            }
            Self::WorkerV2Configuration(error) => {
                write!(formatter, "Worker V2 setup failed: {error}")
            }
            Self::WorkerV2Restart(error) => {
                write!(formatter, "Worker V2 restart setup failed: {error}")
            }
            Self::Artifact(error) => write!(formatter, "artifact build attempt failed: {error}"),
            Self::ManagedCompletion { primary, cleanup } => {
                write!(formatter, "managed build completion failed: {primary}")?;
                if let Some(cleanup) = cleanup {
                    write!(
                        formatter,
                        "; build-attempt invalidation also failed: {cleanup}"
                    )?;
                }
                Ok(())
            }
            Self::AttemptTermination {
                rustc_status,
                cleanup,
            } => write!(
                formatter,
                "rustc exited with {rustc_status}, and build-attempt invalidation failed: {cleanup}"
            ),
            Self::UnsupportedInvocation => {
                formatter.write_str("unsupported future rustc invocation classification")
            }
            Self::Spawn(error) => write!(formatter, "failed to execute rustc: {error}"),
        }
    }
}

impl Error for BindingWrapperError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Arguments(error) => Some(error),
            Self::Spawn(error) => Some(error),
            Self::CurrentDirectory(error) => Some(error),
            Self::WorkerV2Configuration(error) => Some(error),
            Self::WorkerV2Restart(error) => Some(error),
            Self::Artifact(error) => Some(error),
            Self::PinnedExecutable(error) => Some(error),
            Self::ManagedCompletion { cleanup, .. } => cleanup
                .as_ref()
                .map(|error| error as &(dyn Error + 'static)),
            Self::AttemptTermination { cleanup, .. } => Some(cleanup),
            Self::MissingMetadata { .. }
            | Self::InvalidCodegenOption { .. }
            | Self::EmptyMetadata { .. }
            | Self::MissingManagedEnvironment(_)
            | Self::InvalidBuildSession
            | Self::InvalidManagedRustcArguments(_)
            | Self::CapabilityBroker(_)
            | Self::ChildCapability(_)
            | Self::UninspectableRustcResponseFile { .. }
            | Self::PreexistingCodegenBackend { .. }
            | Self::AuthorityLinkerOverride { .. }
            | Self::OptionTerminatorBeforeManagedArguments { .. }
            | Self::UnsupportedInvocation => None,
            Self::BuildObservation(_) => None,
        }
    }
}

impl From<RustcArgsErrorV2> for BindingWrapperError {
    fn from(value: RustcArgsErrorV2) -> Self {
        Self::Arguments(value)
    }
}

impl From<PinExecutableError> for BindingWrapperError {
    fn from(value: PinExecutableError) -> Self {
        Self::PinnedExecutable(value)
    }
}

pub(crate) fn run(mut argv: Vec<OsString>) -> Result<ExitStatus, BindingWrapperError> {
    reject_dynamic_loader_environment()?;
    let expected_rustc_sha256 = expected_rustc_sha256()?;
    reject_uninspectable_rustc_args(&argv)?;
    if crate::non_production_reproduction::enabled() {
        canonicalize_rustc_metadata(&mut argv);
    }
    let invocation = match classify_rustc_invocation_v2(&argv) {
        Ok(invocation) => invocation,
        Err(_) if is_cargo_stdin_probe(&argv) => {
            let pinned = pin_parent_rustc_descriptor(&argv[0], expected_rustc_sha256)?;
            let mut command = pinned.command()?;
            configure_managed_rustc_loader(command.as_command_mut());
            command.args(&argv[1..]);
            configure_build_observation_environment(command.as_command_mut(), None);
            return command.status().map_err(BindingWrapperError::Spawn);
        }
        Err(error) => return Err(error.into()),
    };
    let pinned_rustc = pin_parent_rustc_descriptor(invocation.executable(), expected_rustc_sha256)?;
    let (
        build_observation,
        managed_attempt,
        managed_rustc_args,
        compiler_capabilities,
        rustc_working_directory,
    ) = match invocation {
        RustcInvocationV2::Compile(compile) => {
            let managed_rustc_args = managed_rustc_args_from_environment()?;
            let metadata = ordered_metadata_values(compile.argv())?;
            let build_observation =
                CompileBuildObservationV2::from_ordered_metadata(compile.crate_name(), &metadata)?;
            let worker_v2 = PreparedWorkerV2Config::from_environment()
                .map_err(BindingWrapperError::WorkerV2Configuration)?;
            validate_expected_worker_v2_identity(worker_v2.as_ref())?;
            let capability_profile = if worker_v2
                .as_ref()
                .and_then(PreparedWorkerV2Config::source_debug_profile)
                .is_some()
            {
                capability_broker::CapabilityProfileV1::S09
            } else {
                capability_broker::CapabilityProfileV1::Ordinary
            };
            if capability_profile == capability_broker::CapabilityProfileV1::S09
                || std::env::var_os("FE2O3_CODEGEN_PIPELINE").as_deref()
                    == Some(OsStr::new(ROW_SOFTMAX_V1_PIPELINE))
            {
                reject_authority_linker_arguments(compile.argv())?;
            }
            let capability_binding =
                capability_broker::CapabilityBindingV2::from_environment_for_client(
                    capability_profile,
                    worker_v2
                        .as_ref()
                        .map(|config| *config.identity().as_bytes()),
                )
                .map_err(BindingWrapperError::CapabilityBroker)?;
            authenticate_pinned_rustc(&pinned_rustc, capability_binding.rustc_executable_sha256())?;
            validate_rustc_lib_tree_descriptor(capability_binding)?;
            let compiler_capabilities = CompilerCapabilities::from_environment(capability_binding)?;
            let current_dir =
                std::env::current_dir().map_err(BindingWrapperError::CurrentDirectory)?;
            let managed = if worker_v2.as_ref().is_some_and(|config| {
                !config.selects(compile.crate_name(), compile.source_path(), &current_dir)
            }) {
                None
            } else {
                Some(prepare_managed_attempt(
                    compile,
                    worker_v2,
                    &current_dir,
                    compiler_capabilities.output_dir(),
                    &managed_rustc_args,
                )?)
            };
            (
                Some(build_observation),
                managed,
                managed_rustc_args,
                Some(compiler_capabilities),
                Some(current_dir),
            )
        }
        RustcInvocationV2::Terminal(_) | RustcInvocationV2::Query(_) => {
            (None, None, Vec::new(), None, None)
        }
        _ => return Err(BindingWrapperError::UnsupportedInvocation),
    };

    if managed_attempt
        .as_ref()
        .is_some_and(ManagedAttempt::is_worker_v2_recovery)
    {
        complete_managed_attempt(managed_attempt.expect("managed recovery exists"))?;
        return Ok(success_exit_status());
    }

    let execution_directory = match &rustc_working_directory {
        Some(directory) => directory.clone(),
        None => std::env::current_dir().map_err(BindingWrapperError::CurrentDirectory)?,
    };
    let compile_environment_profile = managed_attempt
        .as_ref()
        .and_then(ManagedAttempt::compile_environment_profile);
    let pinned_execution_directory = PinnedWorkingDirectoryV3::open(&execution_directory)
        .map_err(|error| BindingWrapperError::BuildObservation(error.to_string()))?;
    let mut command = pinned_rustc.command()?;
    configure_managed_rustc_loader(command.as_command_mut());
    append_prepared_rustc_arguments(
        command.as_command_mut(),
        invocation.forwarded_args(),
        &managed_rustc_args,
    )?;
    pinned_execution_directory.configure_child_fchdir(command.as_command_mut());
    if let Some(capabilities) = &compiler_capabilities {
        capabilities.prepare_command(command.as_command_mut())?;
    }
    configure_build_observation_environment(command.as_command_mut(), build_observation);
    if let Some(managed) = &managed_attempt {
        command
            .as_command_mut()
            .env(BUILD_ATTEMPT_ENV, managed.attempt.to_env_value());
    } else {
        command.as_command_mut().env_remove(BUILD_ATTEMPT_ENV);
    }
    match managed_attempt
        .as_ref()
        .and_then(ManagedAttempt::source_debug_profile)
    {
        Some(profile) => {
            command
                .as_command_mut()
                .env(WORKER_V2_SOURCE_DEBUG_PROFILE_ENV, profile.env_value());
        }
        None => {
            command
                .as_command_mut()
                .env_remove(WORKER_V2_SOURCE_DEBUG_PROFILE_ENV);
        }
    }
    let mut worker_build_observation = match managed_attempt.as_ref() {
        Some(managed) if managed.source_debug_profile().is_some() => {
            let pinned_cargo_image_sha256 = compiler_capabilities
                .as_ref()
                .and_then(CompilerCapabilities::pinned_cargo_image_sha256)
                .ok_or_else(|| {
                    BindingWrapperError::BuildObservation(
                        "S09 build has no brokered pinned Cargo image observation".to_owned(),
                    )
                })?;
            managed.worker_build_observation(pinned_cargo_image_sha256)?
        }
        Some(_) | None => None,
    };
    if compile_environment_profile.is_some() {
        let managed = managed_attempt.as_ref().ok_or_else(|| {
            BindingWrapperError::BuildObservation(
                "reviewed compile environment has no managed build attempt".to_owned(),
            )
        })?;
        let capabilities = compiler_capabilities.as_ref().ok_or_else(|| {
            BindingWrapperError::BuildObservation(
                "reviewed compile environment has no compiler capabilities".to_owned(),
            )
        })?;
        let private_tmpdir = capabilities.create_reviewed_private_tmpdir(managed.attempt)?;
        command
            .as_command_mut()
            .env("LANG", "C.UTF-8")
            .env("PATH", "/usr/bin")
            .env("TMPDIR", private_tmpdir);
    }
    configure_worker_build_observation_environment(
        command.as_command_mut(),
        worker_build_observation,
    );
    let complete_reviewed_environment = materialize_reviewed_child_environment(
        compile_environment_profile,
        command.as_command_mut(),
        std::env::vars_os(),
    )?;
    let inert_rustc_invocation = complete_reviewed_environment
        .as_ref()
        .map(|environment| {
            let capabilities = compiler_capabilities.as_ref().ok_or_else(|| {
                BindingWrapperError::BuildObservation(
                    "reviewed invocation capture has no compiler capabilities".to_owned(),
                )
            })?;
            let capture = InertRustcInvocationCaptureV2::capture(
                command.as_command(),
                command.configured_argv0(),
                &execution_directory,
                &environment.entries,
                *pinned_rustc.sha256(),
                capabilities.backend_sha256(),
            )
            .map_err(|error| {
                BindingWrapperError::BuildObservation(format!(
                    "cannot capture inert prepared rustc invocation: {error}"
                ))
            })?;
            debug_assert_eq!(capture.descriptor().amd_target(), "gfx942:xnack-");
            Ok::<_, BindingWrapperError>((capture.digest(), capture))
        })
        .transpose()?;
    if let Some((digest, _)) = inert_rustc_invocation.as_ref()
        && std::env::var_os("FE2O3_VERBOSE").as_deref() == Some(OsStr::new("1"))
    {
        eprintln!(
            "[cargo-fe2o3] inert prepared RustcInvocationDescriptorV2 observation sha256={digest}; no execution or authority claim"
        );
    }
    let protected_source_tree_sha256 = if worker_build_observation.is_some() {
        let source = managed_attempt
            .as_ref()
            .and_then(ManagedAttempt::protected_source_path)
            .ok_or_else(|| {
                BindingWrapperError::BuildObservation(
                    "S09 process consistency has no protected source path".to_owned(),
                )
            })?;
        Some(
            pinned_execution_directory
                .measure_protected_source_tree(source)
                .map_err(|error| BindingWrapperError::BuildObservation(error.to_string()))?
                .identity_sha256(),
        )
    } else {
        None
    };
    let mut prepared_consistency_expectation = if worker_build_observation.is_some() {
        Some(PreparedRustcConsistencyExpectation::attach(
            command.as_command_mut(),
        )?)
    } else {
        None
    };
    if let Some(observation) = worker_build_observation.as_mut() {
        observation.prepared_rustc_command_sha256 = prepared_rustc_command_sha256(
            command.as_command(),
            command.configured_argv0(),
            pinned_rustc.object_identity(),
            *pinned_rustc.sha256(),
            pinned_execution_directory.object_identity(),
            protected_source_tree_sha256.expect("S09 protected source-tree observation exists"),
            complete_reviewed_environment
                .as_ref()
                .expect("S09 complete child environment exists"),
        )?;
        prepared_consistency_expectation
            .as_mut()
            .expect("S09 process-consistency expectation exists")
            .finalize(observation.prepared_rustc_command_sha256)?;
    }
    if std::env::var_os("FE2O3_CODEGEN_PIPELINE").as_deref()
        == Some(OsStr::new(ROW_SOFTMAX_V1_PIPELINE))
        && let Some(managed) = managed_attempt.as_ref()
    {
        let mut effective_argv = Vec::with_capacity(command.as_command().get_args().len() + 1);
        effective_argv.push(command.configured_argv0().to_owned());
        effective_argv.extend(command.as_command().get_args().map(OsString::from));
        let observed = row_softmax_effective_rustc_argv_identity(&effective_argv);
        if managed.attempt.invocation() != observed {
            return Err(BindingWrapperError::BuildObservation(
                "row-softmax build attempt does not bind the exact prepared rustc argv".to_owned(),
            ));
        }
        let claim = BrokeredInvocationCapabilityClaimV1::new(managed.attempt, *observed.as_bytes())
            .map_err(|error| BindingWrapperError::CapabilityBroker(error.to_string()))?;
        compiler_capabilities
            .as_ref()
            .ok_or_else(|| {
                BindingWrapperError::CapabilityBroker(
                    "row-softmax invocation has no brokered compiler capabilities".to_owned(),
                )
            })?
            .prepare_invocation_authority(claim)?;
    }
    let status = command.status();
    // Keep the in-memory descriptor alive across the exact spawn it describes.
    drop(inert_rustc_invocation);
    let status = match status {
        Ok(status) => status,
        Err(error) => {
            if let Some(managed) = managed_attempt {
                fail_build_attempt(&managed.output_dir, &managed.producer, managed.attempt)
                    .map_err(BindingWrapperError::Artifact)?;
            }
            return Err(BindingWrapperError::Spawn(error));
        }
    };
    if let Some(managed) = managed_attempt {
        if status.success() {
            #[cfg(feature = "compiler-handoff-observation-test-only")]
            if let Some(request) = managed.compiler_handoff_observation.as_ref() {
                let observation = if managed.worker_v2.is_some() {
                    Err(
                        "test-only compiler-handoff observation cannot replace a configured Worker V2 consumer"
                            .to_owned(),
                    )
                } else {
                    crate::compiler_handoff_observation::publish_and_wait_for_consumption(
                        request,
                        &managed.output_dir,
                        &managed.producer,
                        managed.attempt,
                    )
                };
                if let Err(primary) = observation {
                    let cleanup =
                        fail_build_attempt(&managed.output_dir, &managed.producer, managed.attempt)
                            .err();
                    return Err(BindingWrapperError::ManagedCompletion { primary, cleanup });
                }
            }
            complete_managed_attempt(managed)?;
        } else if let Err(cleanup) =
            fail_build_attempt(&managed.output_dir, &managed.producer, managed.attempt)
        {
            return Err(BindingWrapperError::AttemptTermination {
                rustc_status: status,
                cleanup,
            });
        }
    }
    Ok(status)
}

fn configure_build_observation_environment(
    command: &mut Command,
    observation: Option<CompileBuildObservationV2>,
) {
    if let Some(observation) = observation {
        command.env(CRATE_BINDING_ID_ENV_V1, observation.crate_binding.to_hex());
        // This digest is an exact build observation, not a semantic admission identity.
        command.env(
            CARGO_METADATA_BUILD_OBSERVATION_ENV_V2,
            observation.cargo_metadata_digest_hex(),
        );
    } else {
        command.env_remove(CRATE_BINDING_ID_ENV_V1);
        command.env_remove(CARGO_METADATA_BUILD_OBSERVATION_ENV_V2);
    }
}

fn reject_dynamic_loader_environment() -> Result<(), BindingWrapperError> {
    let authority_sensitive = std::env::var_os("FE2O3_CODEGEN_PIPELINE").as_deref()
        == Some(OsStr::new(ROW_SOFTMAX_V1_PIPELINE))
        || std::env::var_os(crate::worker_v2::WORKER_V2_CONFIG_ENV).is_some();
    for (name, value) in std::env::vars_os() {
        if crate::is_dynamic_loader_injection_environment_name(&name) {
            // Cargo itself adds LD_LIBRARY_PATH for ordinary rustc wrappers. It is not forwarded:
            // configure_managed_rustc_loader replaces it with the retained fd 193 path. Authority
            // profiles reject it because their protected launcher contract is fail-closed.
            if name == OsStr::new("LD_LIBRARY_PATH") && !authority_sensitive {
                continue;
            }
            return Err(BindingWrapperError::BuildObservation(format!(
                "binding wrapper rejects dynamic-loader injection variable {name:?}={value:?}"
            )));
        }
    }
    Ok(())
}

fn configure_managed_rustc_loader(command: &mut Command) {
    crate::remove_dynamic_loader_environment(command);
    command.env(
        "LD_LIBRARY_PATH",
        format!("/proc/self/fd/{RUSTC_LIBRARY_CHILD_FD}"),
    );
}

fn expected_rustc_sha256() -> Result<[u8; 32], BindingWrapperError> {
    expected_sha256(crate::EXPECTED_RUSTC_SHA256_ENV)
}

fn expected_sha256(name: &'static str) -> Result<[u8; 32], BindingWrapperError> {
    let value = std::env::var_os(name).ok_or_else(|| {
        BindingWrapperError::BuildObservation(format!("binding wrapper is missing {name}"))
    })?;
    let encoded = os_bytes(&value);
    if encoded.len() != 64 {
        return Err(BindingWrapperError::BuildObservation(format!(
            "{name} is not a canonical SHA-256 digest"
        )));
    }
    let mut digest = [0_u8; 32];
    for (output, pair) in digest.iter_mut().zip(encoded.chunks_exact(2)) {
        let nibble = |byte: u8| match byte {
            b'0'..=b'9' => Some(byte - b'0'),
            b'a'..=b'f' => Some(byte - b'a' + 10),
            _ => None,
        };
        let high = nibble(pair[0]).ok_or_else(|| {
            BindingWrapperError::BuildObservation(format!(
                "{name} is not a canonical SHA-256 digest"
            ))
        })?;
        let low = nibble(pair[1]).ok_or_else(|| {
            BindingWrapperError::BuildObservation(format!(
                "{name} is not a canonical SHA-256 digest"
            ))
        })?;
        *output = (high << 4) | low;
    }
    Ok(digest)
}

fn authenticate_pinned_rustc(
    rustc: &PinnedExecutable,
    expected_sha256: [u8; 32],
) -> Result<(), BindingWrapperError> {
    if rustc.sha256() != &expected_sha256 {
        return Err(BindingWrapperError::BuildObservation(
            "Cargo selected a rustc executable that does not match the parent-pinned compiler"
                .to_owned(),
        ));
    }
    Ok(())
}

fn validate_rustc_lib_tree_descriptor(
    binding: capability_broker::CapabilityBindingV2,
) -> Result<(), BindingWrapperError> {
    // SAFETY: Cargo must inherit this fixed descriptor from the parent. fstat/fcntl do not take
    // ownership, and the descriptor remains live through the rustc transition.
    let descriptor = unsafe { BorrowedFd::borrow_raw(RUSTC_LIBRARY_CHILD_FD) };
    let stat = rustix::fs::fstat(descriptor).map_err(|error| {
        BindingWrapperError::BuildObservation(format!(
            "binding wrapper cannot inspect inherited rustc lib-tree fd {RUSTC_LIBRARY_CHILD_FD}: {error}"
        ))
    })?;
    if stat.st_mode & libc::S_IFMT != libc::S_IFDIR {
        return Err(BindingWrapperError::BuildObservation(format!(
            "binding wrapper inherited rustc lib-tree fd {RUSTC_LIBRARY_CHILD_FD} is not a directory"
        )));
    }
    let status = rustix::fs::fcntl_getfl(descriptor).map_err(|error| {
        BindingWrapperError::BuildObservation(format!(
            "binding wrapper cannot inspect inherited rustc lib-tree fd flags: {error}"
        ))
    })?;
    if status & rustix::fs::OFlags::ACCMODE != rustix::fs::OFlags::RDONLY {
        return Err(BindingWrapperError::BuildObservation(
            "binding wrapper inherited rustc lib-tree descriptor is writable".to_owned(),
        ));
    }
    let observed = crate::compiler_toolchain::retained_object_binding_sha256_v1(
        &binding.compiler_closure_sha256(),
        stat.st_dev,
        stat.st_ino,
        stat.st_mode,
    );
    if observed != binding.retained_object_binding_sha256() {
        return Err(BindingWrapperError::BuildObservation(
            "binding wrapper inherited rustc lib-tree descriptor does not match the broker-authenticated retained object"
                .to_owned(),
        ));
    }
    Ok(())
}

fn pin_parent_rustc_descriptor(
    declared: &OsStr,
    expected_sha256: [u8; 32],
) -> Result<PinnedExecutable, BindingWrapperError> {
    let descriptor_path = PathBuf::from(format!("/proc/self/fd/{RUSTC_CHILD_FD}"));
    if declared != descriptor_path.as_os_str() {
        return Err(BindingWrapperError::BuildObservation(
            "Cargo selected a rustc executable that does not match the parent-pinned compiler descriptor"
                .to_owned(),
        ));
    }
    let pinned = PinnedExecutable::open(&descriptor_path)?;
    authenticate_pinned_rustc(&pinned, expected_sha256)?;
    Ok(pinned)
}

fn configure_worker_build_observation_environment(
    command: &mut Command,
    observation: Option<WorkerV2BuildObservation<'_>>,
) {
    if let Some(observation) = observation {
        command.env(
            WORKER_CONFIG_BUILD_OBSERVATION_ENV_V2,
            observation.config_identity.to_hex(),
        );
        command.env(
            WORKER_EXECUTABLE_BUILD_OBSERVATION_ENV_V2,
            hex(&observation.executable_sha256),
        );
        command.env(
            WORKER_BUILD_IDENTITY_OBSERVATION_ENV_V2,
            observation.worker_build_identity,
        );
        command.env(
            LLVM_BUILD_IDENTITY_OBSERVATION_ENV_V2,
            observation.llvm_build_identity,
        );
        command.env(
            CARGO_FE2O3_EXECUTABLE_BUILD_OBSERVATION_ENV_V2,
            hex(&observation.cargo_fe2o3_executable_sha256),
        );
        command.env(
            DECLARED_CARGO_EXECUTABLE_BUILD_OBSERVATION_ENV_V2,
            hex(&observation.declared_cargo_executable_sha256),
        );
        command.env(
            PINNED_CARGO_IMAGE_BUILD_OBSERVATION_ENV_V2,
            hex(&observation.pinned_cargo_image_sha256),
        );
        command.env(
            OBSERVED_PARENT_PID_BUILD_OBSERVATION_ENV_V2,
            observation.observed_parent_pid.to_string(),
        );
        command.env(
            OBSERVED_PARENT_START_TIME_BUILD_OBSERVATION_ENV_V2,
            observation.observed_parent_start_time_ticks.to_string(),
        );
    } else {
        command.env_remove(WORKER_CONFIG_BUILD_OBSERVATION_ENV_V2);
        command.env_remove(WORKER_EXECUTABLE_BUILD_OBSERVATION_ENV_V2);
        command.env_remove(WORKER_BUILD_IDENTITY_OBSERVATION_ENV_V2);
        command.env_remove(LLVM_BUILD_IDENTITY_OBSERVATION_ENV_V2);
        command.env_remove(CARGO_FE2O3_EXECUTABLE_BUILD_OBSERVATION_ENV_V2);
        command.env_remove(DECLARED_CARGO_EXECUTABLE_BUILD_OBSERVATION_ENV_V2);
        command.env_remove(PINNED_CARGO_IMAGE_BUILD_OBSERVATION_ENV_V2);
        command.env_remove(OBSERVED_PARENT_PID_BUILD_OBSERVATION_ENV_V2);
        command.env_remove(OBSERVED_PARENT_START_TIME_BUILD_OBSERVATION_ENV_V2);
    }
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(DIGITS[usize::from(byte >> 4)] as char);
        encoded.push(DIGITS[usize::from(byte & 0x0f)] as char);
    }
    encoded
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct BuildExecutableSnapshot {
    device: u64,
    inode: u64,
    mode: u32,
    size: u64,
    modified_seconds: i64,
    modified_nanoseconds: i64,
    changed_seconds: i64,
    changed_nanoseconds: i64,
}

impl BuildExecutableSnapshot {
    fn from_metadata(metadata: &std::fs::Metadata) -> Self {
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

fn measure_build_executable(
    path: impl AsRef<Path>,
    label: &str,
) -> Result<[u8; 32], BindingWrapperError> {
    let path = path.as_ref();
    let file = File::open(path).map_err(|error| {
        BindingWrapperError::BuildObservation(format!(
            "{label} {}: cannot be opened: {error}",
            path.display()
        ))
    })?;
    measure_open_build_executable(file, path, label)
}

fn measure_open_build_executable(
    mut file: File,
    path: &Path,
    label: &str,
) -> Result<[u8; 32], BindingWrapperError> {
    let fail = |reason: String| {
        BindingWrapperError::BuildObservation(format!("{label} {}: {reason}", path.display()))
    };
    let initial_metadata = file
        .metadata()
        .map_err(|error| fail(format!("cannot be inspected: {error}")))?;
    let initial = BuildExecutableSnapshot::from_metadata(&initial_metadata);
    if !initial_metadata.is_file() || initial.mode & 0o111 == 0 {
        return Err(fail("is not a regular executable file".to_owned()));
    }
    if initial.size == 0 || initial.size > MAX_BUILD_EXECUTABLE_BYTES {
        return Err(fail(format!(
            "must contain 1 through {MAX_BUILD_EXECUTABLE_BYTES} bytes; found {}",
            initial.size
        )));
    }

    let mut digest = Sha256::new();
    let mut remaining = initial.size;
    let mut buffer = [0_u8; 64 * 1024];
    while remaining != 0 {
        let wanted = usize::try_from(remaining.min(buffer.len() as u64))
            .expect("bounded executable read size");
        let read = file
            .read(&mut buffer[..wanted])
            .map_err(|error| fail(format!("cannot be hashed: {error}")))?;
        if read == 0 {
            return Err(fail(format!(
                "was truncated while hashing with {remaining} bytes remaining"
            )));
        }
        digest.update(&buffer[..read]);
        remaining -= read as u64;
    }
    if file
        .read(&mut buffer[..1])
        .map_err(|error| fail(format!("cannot be checked for growth: {error}")))?
        != 0
    {
        return Err(fail("grew while hashing".to_owned()));
    }
    let final_metadata = file
        .metadata()
        .map_err(|error| fail(format!("cannot be reinspected: {error}")))?;
    if BuildExecutableSnapshot::from_metadata(&final_metadata) != initial {
        return Err(fail("changed while hashing".to_owned()));
    }
    Ok(digest.finalize().into())
}

fn resolve_declared_cargo_executable(current_dir: &Path) -> Result<PathBuf, BindingWrapperError> {
    let value = std::env::var_os("CARGO").ok_or_else(|| {
        BindingWrapperError::BuildObservation(
            "CARGO is missing from the rustc wrapper environment".to_owned(),
        )
    })?;
    if value.is_empty() {
        return Err(BindingWrapperError::BuildObservation(
            "CARGO is empty in the rustc wrapper environment".to_owned(),
        ));
    }
    resolve_command_executable(&value, current_dir)
}

pub(crate) fn resolve_command_executable(
    value: &OsStr,
    current_dir: &Path,
) -> Result<PathBuf, BindingWrapperError> {
    let search = std::env::var_os("PATH");
    resolve_command_executable_with_path(value, current_dir, search.as_deref())
}

fn resolve_command_executable_with_path(
    value: &OsStr,
    current_dir: &Path,
    search: Option<&OsStr>,
) -> Result<PathBuf, BindingWrapperError> {
    if value.is_empty() {
        return Err(BindingWrapperError::BuildObservation(
            "executable path is empty".to_owned(),
        ));
    }
    let path = PathBuf::from(value);
    if path.is_absolute() || path.components().count() > 1 {
        return if path.is_absolute() {
            Ok(path)
        } else {
            Ok(current_dir.join(path))
        };
    }
    let search = search.ok_or_else(|| {
        BindingWrapperError::BuildObservation(
            "PATH is missing while resolving an executable".to_owned(),
        )
    })?;
    for directory in std::env::split_paths(search) {
        let candidate = directory.join(&path);
        if candidate
            .metadata()
            .is_ok_and(|metadata| metadata.is_file() && metadata.mode() & 0o111 != 0)
        {
            return Ok(candidate);
        }
    }
    Err(BindingWrapperError::BuildObservation(format!(
        "executable {:?} was not found on PATH",
        value
    )))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PinnedCargoImageAndParentObservation {
    pinned_cargo_image_sha256: [u8; 32],
    observed_parent_pid: u64,
    observed_parent_start_time_ticks: u64,
}

fn observe_pinned_cargo_image_and_parent(
    pinned_cargo_image_sha256: [u8; 32],
) -> Result<PinnedCargoImageAndParentObservation, BindingWrapperError> {
    let initial_parent = rustix::process::getppid().ok_or_else(|| {
        BindingWrapperError::BuildObservation("wrapper has no observed parent PID".to_owned())
    })?;
    let pid = u64::try_from(initial_parent.as_raw_nonzero().get()).map_err(|_| {
        BindingWrapperError::BuildObservation("observed parent PID is negative".to_owned())
    })?;
    let initial_start_time = process_start_time_ticks(pid)?;
    let final_start_time = process_start_time_ticks(pid)?;
    let final_parent = rustix::process::getppid().ok_or_else(|| {
        BindingWrapperError::BuildObservation("observed parent disappeared".to_owned())
    })?;
    if final_parent != initial_parent || final_start_time != initial_start_time {
        return Err(BindingWrapperError::BuildObservation(
            "observed parent PID continuity changed while reading /proc".to_owned(),
        ));
    }
    Ok(PinnedCargoImageAndParentObservation {
        pinned_cargo_image_sha256,
        observed_parent_pid: pid,
        observed_parent_start_time_ticks: initial_start_time,
    })
}

fn process_start_time_ticks(pid: u64) -> Result<u64, BindingWrapperError> {
    let path = PathBuf::from(format!("/proc/{pid}/stat"));
    let bytes = fs::read(&path).map_err(|error| {
        BindingWrapperError::BuildObservation(format!(
            "cannot read observed parent {}: {error}",
            path.display()
        ))
    })?;
    if bytes.is_empty() || bytes.len() > MAX_PROC_STAT_BYTES {
        return Err(BindingWrapperError::BuildObservation(format!(
            "observed parent {} must contain 1 through {MAX_PROC_STAT_BYTES} bytes",
            path.display()
        )));
    }
    let close = bytes
        .iter()
        .rposition(|byte| *byte == b')')
        .ok_or_else(|| {
            BindingWrapperError::BuildObservation(
                "observed parent stat has no command terminator".to_owned(),
            )
        })?;
    let recorded_pid = bytes[..close]
        .split(|byte| *byte == b' ')
        .next()
        .and_then(|value| std::str::from_utf8(value).ok())
        .and_then(|value| value.parse::<u64>().ok());
    if recorded_pid != Some(pid) {
        return Err(BindingWrapperError::BuildObservation(
            "observed parent stat PID does not match the proc entry".to_owned(),
        ));
    }
    let start_time = bytes[close + 1..]
        .split(u8::is_ascii_whitespace)
        .filter(|field| !field.is_empty())
        .nth(19)
        .and_then(|value| std::str::from_utf8(value).ok())
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|value| *value != 0)
        .ok_or_else(|| {
            BindingWrapperError::BuildObservation(
                "observed parent stat has no valid start-time field".to_owned(),
            )
        })?;
    Ok(start_time)
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CompleteReviewedChildEnvironmentV2 {
    entries: Vec<(OsString, OsString)>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FixedS09InheritedInputV2 {
    CargoManifestDir,
    CodegenPipeline,
    Target,
}

impl FixedS09InheritedInputV2 {
    const ALL: [Self; 3] = [Self::CargoManifestDir, Self::CodegenPipeline, Self::Target];

    const fn name(self) -> &'static str {
        match self {
            Self::CargoManifestDir => "CARGO_MANIFEST_DIR",
            Self::CodegenPipeline => "FE2O3_CODEGEN_PIPELINE",
            Self::Target => "FE2O3_TARGET",
        }
    }

    fn accepts(self, value: &OsStr) -> bool {
        match self {
            Self::CargoManifestDir => {
                let path = Path::new(value);
                path.is_absolute() && os_bytes(value).len() <= 4096
            }
            Self::CodegenPipeline => value == "kernel-ir-worker-v2",
            Self::Target => value == "gfx942:xnack-",
        }
    }
}

impl CompleteReviewedChildEnvironmentV2 {
    #[cfg(test)]
    fn from_command(command: &Command) -> Self {
        let mut entries = command
            .get_envs()
            .filter_map(|(name, value)| value.map(|value| (name.to_owned(), value.to_owned())))
            .collect::<Vec<_>>();
        entries.sort_unstable();
        Self { entries }
    }
}

fn materialize_reviewed_child_environment(
    profile: Option<WorkerV2CompileEnvironmentProfileV1>,
    command: &mut Command,
    inherited: impl IntoIterator<Item = (OsString, OsString)>,
) -> Result<Option<CompleteReviewedChildEnvironmentV2>, BindingWrapperError> {
    match profile {
        Some(WorkerV2CompileEnvironmentProfileV1::S09AlphaGfx942O0) => {
            materialize_s09_child_environment(command, inherited).map(Some)
        }
        Some(WorkerV2CompileEnvironmentProfileV1::ScalarGemmV1Gfx942) => {
            materialize_scalar_gemm_v1_child_environment(command, inherited).map(Some)
        }
        Some(WorkerV2CompileEnvironmentProfileV1::RowSoftmaxV1Gfx942) => {
            materialize_row_softmax_v1_child_environment(command, inherited).map(Some)
        }
        None => Ok(None),
    }
}

fn materialize_row_softmax_v1_child_environment(
    command: &mut Command,
    inherited: impl IntoIterator<Item = (OsString, OsString)>,
) -> Result<CompleteReviewedChildEnvironmentV2, BindingWrapperError> {
    let inherited = inherited.into_iter().collect::<BTreeMap<_, _>>();
    for name in inherited.keys() {
        if rejected_s09_inherited_environment(name) {
            return Err(BindingWrapperError::BuildObservation(format!(
                "row-softmax child environment rejects inherited variable {name:?}"
            )));
        }
    }
    let required = |name: &'static str| {
        inherited.get(OsStr::new(name)).ok_or_else(|| {
            BindingWrapperError::BuildObservation(format!(
                "row-softmax child environment is missing required {name}"
            ))
        })
    };
    let manifest_dir = required("CARGO_MANIFEST_DIR")?;
    if !canonical_absolute_utf8_path(manifest_dir) {
        return Err(BindingWrapperError::BuildObservation(
            "row-softmax child environment has invalid CARGO_MANIFEST_DIR".to_owned(),
        ));
    }
    if required("FE2O3_CODEGEN_PIPELINE")? != ROW_SOFTMAX_V1_PIPELINE {
        return Err(BindingWrapperError::BuildObservation(
            "row-softmax child environment has changed FE2O3_CODEGEN_PIPELINE".to_owned(),
        ));
    }
    if required(TARGET_ENV)? != "gfx942:xnack-" {
        return Err(BindingWrapperError::BuildObservation(
            "row-softmax child environment requires FE2O3_TARGET=gfx942:xnack-".to_owned(),
        ));
    }
    let mut final_environment = BTreeMap::from([
        (OsString::from("CARGO_MANIFEST_DIR"), manifest_dir.clone()),
        (
            OsString::from("FE2O3_CODEGEN_PIPELINE"),
            OsString::from(ROW_SOFTMAX_V1_PIPELINE),
        ),
        (OsString::from(TARGET_ENV), OsString::from("gfx942:xnack-")),
    ]);
    let explicit = command
        .get_envs()
        .map(|(name, value)| (name.to_owned(), value.map(OsString::from)))
        .collect::<Vec<_>>();
    for (name, value) in explicit {
        if apply_managed_loader_environment(&mut final_environment, &name, value.as_deref())? {
            continue;
        }
        if !managed_s09_child_environment(&name) {
            return Err(BindingWrapperError::BuildObservation(format!(
                "row-softmax command has unreviewed explicit environment mutation {name:?}"
            )));
        }
        match value {
            Some(value) => {
                final_environment.insert(name, value);
            }
            None => {
                final_environment.remove(&name);
            }
        }
    }
    let closure = final_environment
        .get(OsStr::new(crate::EXPECTED_COMPILER_CLOSURE_SHA256_ENV))
        .ok_or_else(|| {
            BindingWrapperError::BuildObservation(
                "row-softmax command has no broker-authenticated compiler closure".to_owned(),
            )
        })?;
    let closure = os_bytes(closure);
    if closure.len() != 64
        || closure.iter().all(|byte| *byte == b'0')
        || !closure
            .iter()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
    {
        return Err(BindingWrapperError::BuildObservation(
            "row-softmax command has a noncanonical compiler closure".to_owned(),
        ));
    }
    command.env_clear();
    command.envs(&final_environment);
    Ok(CompleteReviewedChildEnvironmentV2 {
        entries: final_environment.into_iter().collect(),
    })
}

fn materialize_s09_child_environment(
    command: &mut Command,
    inherited: impl IntoIterator<Item = (OsString, OsString)>,
) -> Result<CompleteReviewedChildEnvironmentV2, BindingWrapperError> {
    let inherited = inherited.into_iter().collect::<BTreeMap<_, _>>();
    for name in inherited.keys() {
        if rejected_s09_inherited_environment(name) {
            return Err(BindingWrapperError::BuildObservation(format!(
                "S09 child environment rejects inherited variable {name:?}"
            )));
        }
    }

    let mut final_environment = BTreeMap::new();
    for input in FixedS09InheritedInputV2::ALL {
        let name = input.name();
        let value = inherited.get(OsStr::new(name)).ok_or_else(|| {
            BindingWrapperError::BuildObservation(format!(
                "S09 fixed environment is missing required {name}"
            ))
        })?;
        if !input.accepts(value) {
            return Err(BindingWrapperError::BuildObservation(format!(
                "S09 fixed environment has invalid {name}"
            )));
        }
        final_environment.insert(OsString::from(name), value.clone());
    }
    let explicit = command
        .get_envs()
        .map(|(name, value)| (name.to_owned(), value.map(OsString::from)))
        .collect::<Vec<_>>();
    for (name, value) in explicit {
        if apply_managed_loader_environment(&mut final_environment, &name, value.as_deref())? {
            continue;
        }
        if !managed_s09_child_environment(&name) {
            return Err(BindingWrapperError::BuildObservation(format!(
                "S09 command has unreviewed explicit environment mutation {name:?}"
            )));
        }
        match value {
            Some(value) => {
                final_environment.insert(name, value);
            }
            None => {
                final_environment.remove(&name);
            }
        }
    }
    command.env_clear();
    command.envs(&final_environment);
    Ok(CompleteReviewedChildEnvironmentV2 {
        entries: final_environment.into_iter().collect(),
    })
}

fn materialize_scalar_gemm_v1_child_environment(
    command: &mut Command,
    inherited: impl IntoIterator<Item = (OsString, OsString)>,
) -> Result<CompleteReviewedChildEnvironmentV2, BindingWrapperError> {
    let inherited = inherited.into_iter().collect::<BTreeMap<_, _>>();
    for (name, value) in &inherited {
        let Some(name_text) = name.to_str() else {
            return Err(BindingWrapperError::BuildObservation(
                "scalar GEMM child environment contains a non-UTF-8 variable name".to_owned(),
            ));
        };
        if value.to_str().is_none() {
            return Err(BindingWrapperError::BuildObservation(format!(
                "scalar GEMM child environment variable {name_text} has a non-UTF-8 value"
            )));
        }
        if rejected_s09_inherited_environment(name) {
            return Err(BindingWrapperError::BuildObservation(format!(
                "scalar GEMM child environment rejects inherited variable {name_text}"
            )));
        }
        if name_text.starts_with("FE2O3_") && !reviewed_scalar_inherited_environment(name) {
            return Err(BindingWrapperError::BuildObservation(format!(
                "scalar GEMM child environment rejects unreviewed inherited variable {name_text}"
            )));
        }
    }

    let required = |name: &'static str| {
        inherited.get(OsStr::new(name)).ok_or_else(|| {
            BindingWrapperError::BuildObservation(format!(
                "scalar GEMM child environment is missing required {name}"
            ))
        })
    };
    let manifest_dir = required("CARGO_MANIFEST_DIR")?;
    if !canonical_absolute_utf8_path(manifest_dir) {
        return Err(BindingWrapperError::BuildObservation(
            "scalar GEMM child environment has invalid CARGO_MANIFEST_DIR".to_owned(),
        ));
    }
    if required("FE2O3_CODEGEN_PIPELINE")? != SCALAR_GEMM_V1_PIPELINE {
        return Err(BindingWrapperError::BuildObservation(
            "scalar GEMM child environment has changed FE2O3_CODEGEN_PIPELINE".to_owned(),
        ));
    }
    if required(TARGET_ENV)? != "gfx942:xnack-" {
        return Err(BindingWrapperError::BuildObservation(
            "scalar GEMM child environment has missing or changed FE2O3_TARGET".to_owned(),
        ));
    }
    let verification = inherited
        .get(OsStr::new(VERIFY_KERNEL_IR_ENV))
        .map_or(OsStr::new("0"), OsString::as_os_str);
    if !matches!(verification.to_str(), Some("0" | "1")) {
        return Err(BindingWrapperError::BuildObservation(format!(
            "scalar GEMM child environment has invalid {VERIFY_KERNEL_IR_ENV}"
        )));
    }

    let mut final_environment = BTreeMap::from([
        (OsString::from("CARGO_MANIFEST_DIR"), manifest_dir.clone()),
        (
            OsString::from("FE2O3_CODEGEN_PIPELINE"),
            OsString::from(SCALAR_GEMM_V1_PIPELINE),
        ),
        (OsString::from(TARGET_ENV), OsString::from("gfx942:xnack-")),
        (
            OsString::from(VERIFY_KERNEL_IR_ENV),
            verification.to_owned(),
        ),
    ]);
    let explicit = command
        .get_envs()
        .map(|(name, value)| (name.to_owned(), value.map(OsString::from)))
        .collect::<Vec<_>>();
    for (name, value) in explicit {
        if apply_managed_loader_environment(&mut final_environment, &name, value.as_deref())? {
            continue;
        }
        if !managed_s09_child_environment(&name) {
            return Err(BindingWrapperError::BuildObservation(format!(
                "scalar GEMM command has unreviewed explicit environment mutation {name:?}"
            )));
        }
        if name == WORKER_V2_SOURCE_DEBUG_PROFILE_ENV && value.is_some() {
            return Err(BindingWrapperError::BuildObservation(
                "scalar GEMM command cannot select an S09 source-debug profile".to_owned(),
            ));
        }
        match value {
            Some(value) => {
                final_environment.insert(name, value);
            }
            None => {
                final_environment.remove(&name);
            }
        }
    }
    validate_scalar_gemm_v1_final_environment(&final_environment)?;
    command.env_clear();
    command.envs(&final_environment);
    Ok(CompleteReviewedChildEnvironmentV2 {
        entries: final_environment.into_iter().collect(),
    })
}

fn apply_managed_loader_environment(
    environment: &mut BTreeMap<OsString, OsString>,
    name: &OsStr,
    value: Option<&OsStr>,
) -> Result<bool, BindingWrapperError> {
    if !crate::is_dynamic_loader_environment_name(name) {
        return Ok(false);
    }
    match (os_bytes(name), value) {
        (b"LD_LIBRARY_PATH", Some(value)) if value == OsStr::new("/proc/self/fd/193") => {
            environment.insert(name.to_owned(), value.to_owned());
        }
        (_, None) => {
            environment.remove(name);
        }
        _ => {
            return Err(BindingWrapperError::BuildObservation(format!(
                "reviewed rustc environment rejects unmanaged loader variable {name:?}"
            )));
        }
    }
    Ok(true)
}

fn reviewed_scalar_inherited_environment(name: &OsStr) -> bool {
    matches!(
        os_bytes(name),
        b"FE2O3_CODEGEN_PIPELINE"
            | b"FE2O3_TARGET"
            | b"FE2O3_VERIFY_KERNEL_IR"
            | b"FE2O3_BACKEND"
            | b"FE2O3_BINDING_WRAPPER_MODE_V1"
            | b"FE2O3_MANAGED_RUSTC_ARGS_V1"
            | b"FE2O3_BUILD_SESSION_V1"
            | b"FE2O3_CAPABILITY_BROKER_V1"
            | b"FE2O3_HOST_PASSTHROUGH"
            | b"FE2O3_WORKER_V2_CONFIG_V2"
            | b"FE2O3_WORKER_V2_EXPECTED_ID_V1"
    )
}

fn canonical_absolute_utf8_path(value: &OsStr) -> bool {
    let Some(value) = value.to_str() else {
        return false;
    };
    if value.is_empty()
        || value.len() > 4096
        || !value.starts_with('/')
        || value.contains(['\0', '\\'])
    {
        return false;
    }
    value == "/"
        || !value[1..]
            .split('/')
            .any(|component| component.is_empty() || matches!(component, "." | ".."))
}

fn validate_scalar_gemm_v1_final_environment(
    environment: &BTreeMap<OsString, OsString>,
) -> Result<(), BindingWrapperError> {
    let required = |name: &'static str| {
        environment
            .get(OsStr::new(name))
            .and_then(|value| value.to_str())
            .ok_or_else(|| {
                BindingWrapperError::BuildObservation(format!(
                    "scalar GEMM final environment is missing valid {name}"
                ))
            })
    };
    if required(HSACO_DIR_ENV)? != format!("/proc/self/fd/{ARTIFACT_CHILD_FD}") {
        return Err(BindingWrapperError::BuildObservation(
            "scalar GEMM final environment has changed FE2O3_HSACO_DIR".to_owned(),
        ));
    }
    let attempt = required(BUILD_ATTEMPT_ENV)?;
    let attempt = BuildAttempt::from_env_value(attempt).map_err(|_| {
        BindingWrapperError::BuildObservation(
            "scalar GEMM final environment has invalid FE2O3_BUILD_ATTEMPT_V1".to_owned(),
        )
    })?;
    if attempt.session() == BuildSession::DIRECT {
        return Err(BindingWrapperError::BuildObservation(
            "scalar GEMM final environment has invalid FE2O3_BUILD_ATTEMPT_V1".to_owned(),
        ));
    }
    let backend = required(CODEGEN_BACKEND_BUILD_OBSERVATION_ENV_V2)?;
    if backend.len() != 64
        || backend.bytes().all(|byte| byte == b'0')
        || !backend
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(BindingWrapperError::BuildObservation(
            "scalar GEMM final environment has invalid backend observation".to_owned(),
        ));
    }
    Ok(())
}

fn rejected_s09_inherited_environment(name: &OsStr) -> bool {
    let bytes = os_bytes(name);
    bytes == b"RUSTC_BOOTSTRAP"
        || crate::is_dynamic_loader_environment_name(name)
        || credential_like_environment_name(bytes)
}

fn credential_like_environment_name(name: &[u8]) -> bool {
    let upper = name.iter().map(u8::to_ascii_uppercase).collect::<Vec<_>>();
    [
        b"TOKEN".as_slice(),
        b"PASSWORD".as_slice(),
        b"PASSWD".as_slice(),
        b"SECRET".as_slice(),
        b"CREDENTIAL".as_slice(),
        b"ACCESS_KEY".as_slice(),
        b"PRIVATE_KEY".as_slice(),
    ]
    .iter()
    .any(|pattern| {
        upper
            .windows(pattern.len())
            .any(|window| window == *pattern)
    })
}

fn managed_s09_child_environment(name: &OsStr) -> bool {
    matches!(
        os_bytes(name),
        b"LANG"
            | b"PATH"
            | b"TMPDIR"
            | b"FE2O3_HSACO_DIR"
            | b"FE2O3_BUILD_ATTEMPT_V1"
            | b"FE2O3_CARGO_METADATA_BUILD_OBSERVATION_V2"
            | b"FE2O3_CODEGEN_BACKEND_BUILD_OBSERVATION_V2"
            | b"FE2O3_WORKER_CONFIG_BUILD_OBSERVATION_V2"
            | b"FE2O3_WORKER_EXECUTABLE_BUILD_OBSERVATION_V2"
            | b"FE2O3_WORKER_BUILD_IDENTITY_OBSERVATION_V2"
            | b"FE2O3_LLVM_BUILD_IDENTITY_OBSERVATION_V2"
            | b"FE2O3_CARGO_FE2O3_EXECUTABLE_BUILD_OBSERVATION_V2"
            | b"FE2O3_DECLARED_CARGO_EXECUTABLE_BUILD_OBSERVATION_V2"
            | b"FE2O3_PINNED_CARGO_IMAGE_BUILD_OBSERVATION_V2"
            | b"FE2O3_OBSERVED_PARENT_PID_BUILD_OBSERVATION_V2"
            | b"FE2O3_OBSERVED_PARENT_START_TIME_BUILD_OBSERVATION_V2"
            | b"FE2O3_WORKER_V2_SOURCE_DEBUG_PROFILE_V1"
            | b"FE2O3_CRATE_BINDING_ID_V1"
            | b"FE2O3_EXPECTED_COMPILER_CLOSURE_SHA256_V1"
    )
}

fn prepared_rustc_command_sha256(
    command: &Command,
    configured_argv0: &OsStr,
    executable_object: LinuxObjectIdentityV3,
    executable_sha256: [u8; 32],
    current_dir_object: LinuxObjectIdentityV3,
    protected_source_tree_sha256: [u8; 32],
    environment: &CompleteReviewedChildEnvironmentV2,
) -> Result<[u8; 32], BindingWrapperError> {
    let mut argv = Vec::with_capacity(command.get_args().len() + 1);
    argv.push(configured_argv0.to_owned());
    argv.extend(command.get_args().map(OsString::from));
    parent_prepared_process_consistency_digest_v3(&ParentPreparedProcessConsistencyV3 {
        executable_object,
        executable_sha256,
        argv: &argv,
        current_dir_object,
        protected_source_tree_sha256,
        environment: &environment.entries,
    })
    .map_err(|error| {
        BindingWrapperError::BuildObservation(format!(
            "cannot encode inert parent-prepared/child-observed rustc consistency: {error}"
        ))
    })
}

struct PreparedRustcConsistencyExpectation {
    image: File,
    finalized: bool,
}

impl PreparedRustcConsistencyExpectation {
    fn attach(command: &mut Command) -> Result<Self, BindingWrapperError> {
        let display = "S09 inert process-consistency expectation";
        // SAFETY: fcntl only probes the process-local fixed descriptor number.
        let target = unsafe { BorrowedFd::borrow_raw(PROCESS_CONSISTENCY_EXPECTATION_FD_V3) };
        match rustix::io::fcntl_getfd(target) {
            Err(rustix::io::Errno::BADF) => {}
            Err(error) => {
                return Err(BindingWrapperError::BuildObservation(format!(
                    "cannot inspect {display} descriptor: {error}"
                )));
            }
            Ok(_) => {
                return Err(BindingWrapperError::BuildObservation(format!(
                    "{display} descriptor is already occupied"
                )));
            }
        }
        let image = File::from(
            rustix::fs::memfd_create(
                "fe2o3-s09-process-consistency-v3",
                rustix::fs::MemfdFlags::CLOEXEC | rustix::fs::MemfdFlags::ALLOW_SEALING,
            )
            .map_err(|error| {
                BindingWrapperError::BuildObservation(format!("cannot create {display}: {error}"))
            })?,
        );
        image.set_len(32).map_err(|error| {
            BindingWrapperError::BuildObservation(format!("cannot size {display}: {error}"))
        })?;
        let source_fd = image.as_raw_fd();
        let metadata = image.metadata().map_err(|error| {
            BindingWrapperError::BuildObservation(format!("cannot inspect {display}: {error}"))
        })?;
        let device = metadata.dev();
        let inode = metadata.ino();
        // SAFETY: `image` remains alive through spawn. The callback installs and validates only
        // this exact memfd, which is sealed before the command can be spawned.
        unsafe {
            command.pre_exec(move || {
                let source = BorrowedFd::borrow_raw(source_fd);
                let required = rustix::fs::SealFlags::WRITE
                    | rustix::fs::SealFlags::GROW
                    | rustix::fs::SealFlags::SHRINK
                    | rustix::fs::SealFlags::SEAL;
                if rustix::fs::fcntl_get_seals(source).map_err(std::io::Error::from)? != required {
                    return Err(std::io::Error::from_raw_os_error(
                        rustix::io::Errno::PERM.raw_os_error(),
                    ));
                }
                let installed =
                    rustix::io::fcntl_dupfd_cloexec(source, PROCESS_CONSISTENCY_EXPECTATION_FD_V3)
                        .map_err(std::io::Error::from)?;
                if installed.as_raw_fd() != PROCESS_CONSISTENCY_EXPECTATION_FD_V3 {
                    return Err(std::io::Error::from_raw_os_error(
                        rustix::io::Errno::BUSY.raw_os_error(),
                    ));
                }
                let stat = rustix::fs::fstat(&installed).map_err(std::io::Error::from)?;
                if stat.st_dev != device || stat.st_ino != inode {
                    return Err(std::io::Error::from_raw_os_error(
                        rustix::io::Errno::STALE.raw_os_error(),
                    ));
                }
                rustix::io::fcntl_setfd(&installed, rustix::io::FdFlags::empty())
                    .map_err(std::io::Error::from)?;
                let _ = installed.into_raw_fd();
                Ok(())
            });
        }
        Ok(Self {
            image,
            finalized: false,
        })
    }

    fn finalize(&mut self, digest: [u8; 32]) -> Result<(), BindingWrapperError> {
        if self.finalized || digest == [0; 32] {
            return Err(BindingWrapperError::BuildObservation(
                "S09 process-consistency expectation was finalized invalidly".to_owned(),
            ));
        }
        self.image.seek(SeekFrom::Start(0)).map_err(|error| {
            BindingWrapperError::BuildObservation(format!(
                "cannot rewind S09 process-consistency expectation: {error}"
            ))
        })?;
        self.image.write_all(&digest).map_err(|error| {
            BindingWrapperError::BuildObservation(format!(
                "cannot write S09 process-consistency expectation: {error}"
            ))
        })?;
        self.image.seek(SeekFrom::Start(0)).map_err(|error| {
            BindingWrapperError::BuildObservation(format!(
                "cannot prepare S09 process-consistency expectation for child reading: {error}"
            ))
        })?;
        let data_seals = rustix::fs::SealFlags::WRITE
            | rustix::fs::SealFlags::GROW
            | rustix::fs::SealFlags::SHRINK;
        rustix::fs::fcntl_add_seals(&self.image, data_seals).map_err(|error| {
            BindingWrapperError::BuildObservation(format!(
                "cannot seal S09 process-consistency expectation: {error}"
            ))
        })?;
        rustix::fs::fcntl_add_seals(&self.image, rustix::fs::SealFlags::SEAL).map_err(|error| {
            BindingWrapperError::BuildObservation(format!(
                "cannot finalize S09 process-consistency expectation seals: {error}"
            ))
        })?;
        let required = data_seals | rustix::fs::SealFlags::SEAL;
        if rustix::fs::fcntl_get_seals(&self.image).map_err(|error| {
            BindingWrapperError::BuildObservation(format!(
                "cannot inspect S09 process-consistency expectation seals: {error}"
            ))
        })? != required
        {
            return Err(BindingWrapperError::BuildObservation(
                "S09 process-consistency expectation seals changed".to_owned(),
            ));
        }
        self.finalized = true;
        Ok(())
    }
}

fn managed_rustc_args_from_environment() -> Result<Vec<OsString>, BindingWrapperError> {
    let value = std::env::var_os(MANAGED_RUSTC_ARGS_ENV).ok_or(
        BindingWrapperError::MissingManagedEnvironment(MANAGED_RUSTC_ARGS_ENV),
    )?;
    decode_managed_rustc_args(&value)
}

fn append_prepared_rustc_arguments(
    command: &mut Command,
    forwarded_args: &[OsString],
    managed_rustc_args: &[OsString],
) -> Result<(), BindingWrapperError> {
    if !managed_rustc_args.is_empty()
        && let Some(argument_index) = forwarded_args
            .iter()
            .position(|argument| is_rustc_option_terminator_v2(argument))
    {
        return Err(
            BindingWrapperError::OptionTerminatorBeforeManagedArguments {
                argument_index: argument_index + 1,
            },
        );
    }
    command.args(forwarded_args);
    command.args(managed_rustc_args);
    Ok(())
}

fn decode_managed_rustc_args(value: &OsStr) -> Result<Vec<OsString>, BindingWrapperError> {
    let fields = os_bytes(value)
        .split(|byte| *byte == 0x1f)
        .map(|field| os_string(field.to_vec()))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| {
            BindingWrapperError::InvalidManagedRustcArguments("arguments are not representable")
        })?;
    if fields.len() != 4 || fields.iter().any(|field| field.is_empty()) {
        return Err(BindingWrapperError::InvalidManagedRustcArguments(
            "expected exactly four non-empty arguments",
        ));
    }
    if fields[0] != "-Zmir-enable-passes=-JumpThreading" || fields[1] != "--cfg" {
        return Err(BindingWrapperError::InvalidManagedRustcArguments(
            "managed compiler options changed",
        ));
    }
    let generation = os_bytes(&fields[2]);
    let prefix = b"fe2o3_codegen_generation=\"";
    if generation.len() != prefix.len() + 32 + 1
        || !generation.starts_with(prefix)
        || generation.last() != Some(&b'"')
        || generation[prefix.len()..generation.len() - 1]
            .iter()
            .any(|byte| !byte.is_ascii_hexdigit() || byte.is_ascii_uppercase())
    {
        return Err(BindingWrapperError::InvalidManagedRustcArguments(
            "generation cfg is noncanonical",
        ));
    }
    let expected_backend = format!("-Zcodegen-backend=/proc/./self/fd/{BACKEND_CHILD_FD}");
    if fields[3] != OsStr::new(&expected_backend) {
        return Err(BindingWrapperError::InvalidManagedRustcArguments(
            "final backend selector is not the fixed brokered procfs descriptor",
        ));
    }
    Ok(fields)
}

fn reject_uninspectable_rustc_args(argv: &[OsString]) -> Result<(), BindingWrapperError> {
    for (index, argument) in argv.iter().enumerate() {
        let bytes = os_bytes(argument);
        if bytes.starts_with(b"@") {
            return Err(BindingWrapperError::UninspectableRustcResponseFile {
                argument_index: index,
            });
        }
        if is_rustc_codegen_backend_selector_v2(
            argument,
            argv.get(index + 1).map(OsString::as_os_str),
        ) {
            return Err(BindingWrapperError::PreexistingCodegenBackend {
                argument_index: index,
            });
        }
    }
    Ok(())
}

fn reject_authority_linker_arguments(argv: &[OsString]) -> Result<(), BindingWrapperError> {
    let mut index = 0;
    while index < argv.len() {
        let bytes = os_bytes(&argv[index]);
        let option = if matches!(bytes, b"-C" | b"-Z") {
            argv.get(index + 1)
                .map(|value| (index + 1, os_bytes(value)))
        } else if bytes.starts_with(b"-C") || bytes.starts_with(b"-Z") {
            Some((index, &bytes[2..]))
        } else {
            None
        };
        if let Some((argument_index, option)) = option {
            let key = option.split(|byte| *byte == b'=').next().unwrap_or(option);
            if key == b"linker"
                || key == b"dlltool"
                || key == b"gcc-ld"
                || key.starts_with(b"link-")
                || key.starts_with(b"linker-")
            {
                return Err(BindingWrapperError::AuthorityLinkerOverride { argument_index });
            }
        }
        index += 1;
    }
    Ok(())
}

#[cfg(unix)]
fn os_bytes(value: &OsStr) -> &[u8] {
    use std::os::unix::ffi::OsStrExt;
    value.as_bytes()
}

#[cfg(not(unix))]
fn os_bytes(value: &OsStr) -> &[u8] {
    value
        .to_str()
        .expect("managed rustc arguments must be UTF-8 off Unix")
        .as_bytes()
}

#[cfg(unix)]
fn os_string(value: Vec<u8>) -> Result<OsString, ()> {
    use std::os::unix::ffi::OsStringExt;
    Ok(OsString::from_vec(value))
}

#[cfg(not(unix))]
fn os_string(value: Vec<u8>) -> Result<OsString, ()> {
    String::from_utf8(value).map(OsString::from).map_err(|_| ())
}

fn validate_expected_worker_v2_identity(
    config: Option<&PreparedWorkerV2Config>,
) -> Result<(), BindingWrapperError> {
    let Some(expected) = std::env::var_os(WORKER_V2_EXPECTED_ID_ENV) else {
        if let Some(config) = config.filter(|config| config.requires_expected_identity()) {
            let profile = if config.source_debug_profile().is_some() {
                "S09"
            } else {
                "scalar GEMM"
            };
            return Err(BindingWrapperError::WorkerV2Configuration(
                WorkerV2ConfigError::Invalid(format!(
                    "{profile} Worker V2 configuration requires {WORKER_V2_EXPECTED_ID_ENV}"
                )),
            ));
        }
        return Ok(());
    };
    let expected = expected.to_str().ok_or_else(|| {
        BindingWrapperError::WorkerV2Configuration(WorkerV2ConfigError::Invalid(format!(
            "{WORKER_V2_EXPECTED_ID_ENV} must be lowercase hexadecimal"
        )))
    })?;
    let actual = config.map(|config| config.identity().to_hex());
    if actual.as_deref() != Some(expected) {
        return Err(BindingWrapperError::WorkerV2Configuration(
            WorkerV2ConfigError::Invalid(
                "Worker V2 transitive inputs changed after Cargo generation preparation"
                    .to_string(),
            ),
        ));
    }
    Ok(())
}

struct CompilerCapabilities {
    binding: capability_broker::CapabilityBindingV2,
    backend: PinnedCodegenBackend,
    artifact: PinnedDirectory,
    invocation_authority: Option<capability_broker::BrokeredInvocationAuthorityV1>,
    output_dir: PathBuf,
    pinned_cargo_image_sha256: Option<[u8; 32]>,
}

impl CompilerCapabilities {
    fn from_environment(
        binding: capability_broker::CapabilityBindingV2,
    ) -> Result<Self, BindingWrapperError> {
        let mut transferred = capability_broker::receive(managed_build_session()?, binding)
            .map_err(BindingWrapperError::CapabilityBroker)?;
        let invocation_authority = transferred.invocation_authority.take().ok_or_else(|| {
            BindingWrapperError::CapabilityBroker(
                "capability broker omitted invocation authority".to_owned(),
            )
        })?;
        let invocation_authority = if std::env::var_os("FE2O3_CODEGEN_PIPELINE").as_deref()
            == Some(OsStr::new(ROW_SOFTMAX_V1_PIPELINE))
        {
            Some(invocation_authority)
        } else {
            invocation_authority
                .release()
                .map_err(BindingWrapperError::CapabilityBroker)?;
            None
        };
        let pinned_cargo_image_sha256 = transferred
            .pinned_cargo_image
            .as_ref()
            .map(|image| *image.sha256());
        let output_dir = transferred.artifact.child_path();
        Ok(Self {
            binding,
            backend: transferred.backend,
            artifact: transferred.artifact,
            invocation_authority,
            output_dir,
            pinned_cargo_image_sha256,
        })
    }

    fn output_dir(&self) -> &Path {
        &self.output_dir
    }

    const fn pinned_cargo_image_sha256(&self) -> Option<[u8; 32]> {
        self.pinned_cargo_image_sha256
    }

    fn backend_sha256(&self) -> [u8; 32] {
        *self.backend.sha256()
    }

    const fn compiler_closure_sha256(&self) -> [u8; 32] {
        self.binding.compiler_closure_sha256()
    }

    fn create_reviewed_private_tmpdir(
        &self,
        attempt: BuildAttempt,
    ) -> Result<PathBuf, BindingWrapperError> {
        let component = format!(".fe2o3-s09-tmp-{}", attempt.to_env_value());
        let mode = rustix::fs::Mode::RUSR | rustix::fs::Mode::WUSR | rustix::fs::Mode::XUSR;
        rustix::fs::mkdirat(self.artifact.file(), &component, mode).map_err(|error| {
            BindingWrapperError::BuildObservation(format!(
                "cannot create private S09 compiler temporary directory: {error}"
            ))
        })?;
        let directory = rustix::fs::openat(
            self.artifact.file(),
            &component,
            rustix::fs::OFlags::RDONLY
                | rustix::fs::OFlags::DIRECTORY
                | rustix::fs::OFlags::NOFOLLOW
                | rustix::fs::OFlags::CLOEXEC,
            rustix::fs::Mode::empty(),
        )
        .map_err(|error| {
            BindingWrapperError::BuildObservation(format!(
                "cannot pin private S09 compiler temporary directory: {error}"
            ))
        })?;
        rustix::fs::fchmod(&directory, mode).map_err(|error| {
            BindingWrapperError::BuildObservation(format!(
                "cannot make S09 compiler temporary directory private: {error}"
            ))
        })?;
        Ok(PathBuf::from(format!(
            "/proc/self/fd/{ARTIFACT_CHILD_FD}/{component}"
        )))
    }

    fn prepare_command(&self, command: &mut Command) -> Result<(), BindingWrapperError> {
        let artifact_path = PathBuf::from(format!("/proc/self/fd/{ARTIFACT_CHILD_FD}"));
        self.backend
            .replace_for_child_at(command, BACKEND_CHILD_FD)
            .map_err(|error| BindingWrapperError::ChildCapability(error.to_string()))?;
        self.artifact
            .replace_for_child_at(command, ARTIFACT_CHILD_FD)
            .map_err(BindingWrapperError::ChildCapability)?;
        command.env(HSACO_DIR_ENV, artifact_path);
        command.env(
            CODEGEN_BACKEND_BUILD_OBSERVATION_ENV_V2,
            hex(&self.backend.sha256()[..]),
        );
        command.env(
            crate::EXPECTED_COMPILER_CLOSURE_SHA256_ENV,
            hex(&self.compiler_closure_sha256()),
        );
        if let Some(authority) = &self.invocation_authority {
            authority
                .inherit_for_child(command)
                .map_err(BindingWrapperError::CapabilityBroker)?;
        }
        Ok(())
    }

    fn prepare_invocation_authority(
        &self,
        claim: BrokeredInvocationCapabilityClaimV1,
    ) -> Result<(), BindingWrapperError> {
        self.invocation_authority
            .as_ref()
            .ok_or_else(|| {
                BindingWrapperError::CapabilityBroker(
                    "row-softmax invocation authority was not retained".to_owned(),
                )
            })?
            .prepare(claim)
            .map_err(BindingWrapperError::CapabilityBroker)
    }
}

fn managed_build_session() -> Result<BuildSession, BindingWrapperError> {
    std::env::var(BUILD_SESSION_ENV)
        .ok()
        .and_then(|value| BuildSession::from_hex(&value).ok())
        .filter(|session| *session != BuildSession::DIRECT)
        .ok_or(BindingWrapperError::InvalidBuildSession)
}

struct ManagedAttempt {
    output_dir: PathBuf,
    producer: ProducerIdentity,
    attempt: BuildAttempt,
    protected_source_path: Option<PathBuf>,
    compile_environment_profile: Option<WorkerV2CompileEnvironmentProfileV1>,
    worker_v2: Option<ManagedWorkerV2>,
    #[cfg(feature = "compiler-handoff-observation-test-only")]
    compiler_handoff_observation: Option<crate::compiler_handoff_observation::Request>,
}

enum ManagedWorkerV2 {
    Fresh {
        config: PreparedWorkerV2Config,
        envelope_inputs: Option<WorkerV2EnvelopeInputsV1>,
        resume: WorkerV2ResumeStoreV1,
    },
    Recovery {
        resume: WorkerV2ResumeStoreV1,
        state: ResumeMarkerStateV1,
    },
}

enum CompletionFailure {
    Uncommitted(String),
    PreserveAttempt(String),
}

impl ManagedAttempt {
    fn is_worker_v2_recovery(&self) -> bool {
        matches!(self.worker_v2, Some(ManagedWorkerV2::Recovery { .. }))
    }

    fn source_debug_profile(&self) -> Option<WorkerV2SourceDebugProfileV1> {
        match &self.worker_v2 {
            Some(ManagedWorkerV2::Fresh { config, .. }) => config.source_debug_profile(),
            Some(ManagedWorkerV2::Recovery { .. }) | None => None,
        }
    }

    const fn compile_environment_profile(&self) -> Option<WorkerV2CompileEnvironmentProfileV1> {
        self.compile_environment_profile
    }

    fn protected_source_path(&self) -> Option<&Path> {
        self.protected_source_path.as_deref()
    }

    fn worker_build_observation(
        &self,
        pinned_cargo_image_sha256: [u8; 32],
    ) -> Result<Option<WorkerV2BuildObservation<'_>>, BindingWrapperError> {
        match &self.worker_v2 {
            Some(ManagedWorkerV2::Fresh { config, .. })
                if config.source_debug_profile().is_some() =>
            {
                let cargo_fe2o3_executable_sha256 =
                    measure_build_executable("/proc/self/exe", "cargo-fe2o3 wrapper")?;
                let current_dir =
                    std::env::current_dir().map_err(BindingWrapperError::CurrentDirectory)?;
                let declared_cargo_executable = resolve_declared_cargo_executable(&current_dir)?;
                let declared_cargo_executable_sha256 = measure_build_executable(
                    &declared_cargo_executable,
                    "declared CARGO executable",
                )?;
                let observation = observe_pinned_cargo_image_and_parent(pinned_cargo_image_sha256)?;
                Ok(Some(config.build_observation(
                    [0; 32],
                    cargo_fe2o3_executable_sha256,
                    declared_cargo_executable_sha256,
                    observation.pinned_cargo_image_sha256,
                    observation.observed_parent_pid,
                    observation.observed_parent_start_time_ticks,
                )))
            }
            Some(ManagedWorkerV2::Fresh { .. }) | Some(ManagedWorkerV2::Recovery { .. }) | None => {
                Ok(None)
            }
        }
    }
}

fn prepare_managed_attempt(
    compile: RustcCompileInvocationV2<'_>,
    worker_v2: Option<PreparedWorkerV2Config>,
    current_dir: &std::path::Path,
    output_dir: &Path,
    managed_rustc_args: &[OsString],
) -> Result<ManagedAttempt, BindingWrapperError> {
    #[cfg(feature = "compiler-handoff-observation-test-only")]
    let compiler_handoff_observation = {
        let ordered_metadata = ordered_metadata_values(compile.argv())?;
        crate::compiler_handoff_observation::Request::for_compile(
            compile.crate_name(),
            compile.source_path(),
            &ordered_metadata,
        )
        .map_err(BindingWrapperError::BuildObservation)?
    };
    let compile_environment_profile = if std::env::var_os("FE2O3_CODEGEN_PIPELINE").as_deref()
        == Some(OsStr::new(ROW_SOFTMAX_V1_PIPELINE))
    {
        Some(WorkerV2CompileEnvironmentProfileV1::RowSoftmaxV1Gfx942)
    } else {
        worker_v2.as_ref().and_then(|config| {
            config.compile_environment_profile(
                compile.crate_name(),
                compile.source_path(),
                current_dir,
            )
        })
    };
    let protected_source_path = worker_v2
        .as_ref()
        .and_then(PreparedWorkerV2Config::source_debug_profile)
        .map(|_| compile.source_path().to_path_buf());
    let session = managed_build_session()?;
    let producer =
        ProducerIdentity::from_codegen(compile.crate_name(), Some(compile.source_path()))
            .map_err(BindingWrapperError::Artifact)?;
    let invocation = if std::env::var_os("FE2O3_CODEGEN_PIPELINE").as_deref()
        == Some(OsStr::new(ROW_SOFTMAX_V1_PIPELINE))
    {
        let mut effective_argv =
            Vec::with_capacity(compile.argv().len() + managed_rustc_args.len());
        effective_argv.extend_from_slice(compile.argv());
        effective_argv.extend_from_slice(managed_rustc_args);
        row_softmax_effective_rustc_argv_identity(&effective_argv)
    } else {
        derive_build_attempt_input(compile.argv(), worker_v2.as_ref(), current_dir)
    };
    let (attempt, worker_v2) = if let Some(config) = worker_v2 {
        let resume = WorkerV2ResumeStoreV1::open(output_dir, &producer)
            .map_err(BindingWrapperError::WorkerV2Restart)?;
        if let Some(state) = resume
            .load()
            .map_err(BindingWrapperError::WorkerV2Restart)?
        {
            let attempt = state.attempt();
            if attempt.session() != session || attempt.invocation() != invocation {
                return Err(BindingWrapperError::WorkerV2Restart(
                    ResumeMarkerErrorV1::StaleInvocation,
                ));
            }
            (attempt, Some(ManagedWorkerV2::Recovery { resume, state }))
        } else {
            let envelope_inputs = config
                .load_envelope_inputs()
                .map_err(BindingWrapperError::WorkerV2Configuration)?;
            let attempt = begin_build_attempt(output_dir, &producer, invocation, session)
                .map_err(BindingWrapperError::Artifact)?;
            (
                attempt,
                Some(ManagedWorkerV2::Fresh {
                    config,
                    envelope_inputs,
                    resume,
                }),
            )
        }
    } else {
        let attempt = begin_build_attempt(output_dir, &producer, invocation, session)
            .map_err(BindingWrapperError::Artifact)?;
        (attempt, None)
    };
    Ok(ManagedAttempt {
        output_dir: output_dir.to_path_buf(),
        producer,
        attempt,
        protected_source_path,
        compile_environment_profile,
        worker_v2,
        #[cfg(feature = "compiler-handoff-observation-test-only")]
        compiler_handoff_observation,
    })
}

fn complete_managed_attempt(managed: ManagedAttempt) -> Result<(), BindingWrapperError> {
    let completion = (|| -> Result<(), CompletionFailure> {
        if let Some(worker_v2) = managed.worker_v2.as_ref() {
            return match worker_v2 {
                ManagedWorkerV2::Fresh {
                    config,
                    envelope_inputs,
                    resume,
                } => complete_fresh_worker_v2(&managed, config, envelope_inputs.as_ref(), resume),
                ManagedWorkerV2::Recovery { resume, state } => {
                    complete_recovered_worker_v2(&managed, resume, *state)
                }
            };
        }
        finish_build_attempt(&managed.output_dir, &managed.producer, managed.attempt).map_err(
            |error| {
                CompletionFailure::Uncommitted(format!("build-attempt completion failed: {error}"))
            },
        )
    })();

    match completion {
        Ok(()) => Ok(()),
        Err(CompletionFailure::Uncommitted(primary)) => {
            let cleanup =
                fail_build_attempt(&managed.output_dir, &managed.producer, managed.attempt).err();
            Err(BindingWrapperError::ManagedCompletion { primary, cleanup })
        }
        Err(CompletionFailure::PreserveAttempt(primary)) => {
            Err(BindingWrapperError::ManagedCompletion {
                primary,
                cleanup: None,
            })
        }
    }
}

fn complete_fresh_worker_v2(
    managed: &ManagedAttempt,
    worker_v2: &PreparedWorkerV2Config,
    envelope_inputs: Option<&WorkerV2EnvelopeInputsV1>,
    resume: &WorkerV2ResumeStoreV1,
) -> Result<(), CompletionFailure> {
    debug_assert!(!worker_v2.envelope_mode().grants_load_authority());
    debug_assert!(!worker_v2.envelope_mode().grants_launch_authority());
    let consumed =
        consume_compiler_module_handoff_v1(&managed.output_dir, &managed.producer, managed.attempt)
            .map_err(|error| {
                CompletionFailure::Uncommitted(format!(
                    "compiler-module handoff consumption failed: {error}"
                ))
            })?;
    let evidence = worker_v2.execute(consumed).map_err(|error| {
        CompletionFailure::Uncommitted(format!("reproducible Worker V2 execution failed: {error}"))
    })?;
    debug_assert_eq!(evidence.attempt(), managed.attempt);
    let canonical_request = evidence.authorized_request_bytes().to_vec();
    let canonical_response = evidence.authorized().response().canonical_bytes().to_vec();
    let raw_output = evidence.output_bytes().to_vec();
    let worker_v2_request_identity = *evidence.authorized_request_identity();
    let inspected = inspect_worker_v2_raw_hsaco_v1(evidence).map_err(|error| {
        CompletionFailure::Uncommitted(format!(
            "independent Worker V2 HSACO inspection failed: {error}"
        ))
    })?;
    let persisted = persist_admitted_worker_v2_intent_v1(
        resume,
        &managed.producer,
        inspected,
        worker_v2.envelope_mode(),
        envelope_inputs,
    )
    .map_err(|error| preserve_restart_error("persistence", error))?;
    write_non_production_reproduction_record(
        &persisted,
        &canonical_request,
        &canonical_response,
        &raw_output,
        &worker_v2_request_identity,
    )?;
    publish_finish_and_clear(managed, resume, persisted.publication, persisted.intent)
}

fn write_non_production_reproduction_record(
    persisted: &PersistedAdmittedWorkerV2IntentV1,
    canonical_request: &[u8],
    canonical_response: &[u8],
    raw_output: &[u8],
    worker_v2_request_identity: &[u8; 32],
) -> Result<(), CompletionFailure> {
    if !crate::non_production_reproduction::enabled() {
        return Ok(());
    }
    let path = std::env::var_os(NON_PRODUCTION_REPRODUCTION_RECORD_ENV).ok_or_else(|| {
        CompletionFailure::Uncommitted(format!(
            "non-production reproduction is missing {NON_PRODUCTION_REPRODUCTION_RECORD_ENV}"
        ))
    })?;
    let path = PathBuf::from(path);
    if !path.is_absolute() || path.file_name().is_none() {
        return Err(CompletionFailure::Uncommitted(
            "non-production reproduction record path is not absolute".into(),
        ));
    }
    let parent = path.parent().expect("absolute record path has a parent");
    if parent.canonicalize().ok().as_deref() != Some(parent) {
        return Err(CompletionFailure::Uncommitted(
            "non-production reproduction record parent is not canonical".into(),
        ));
    }
    let plan = persisted.intent.record().plan();
    let exact_output = persisted.intent.exact_output();
    let sealed_worker_v2_response_identity = {
        let mut digest = Sha256::new();
        digest.update(b"FE2O3/WORKER-V2-SEALED-RESPONSE/V1\0");
        digest.update((canonical_response.len() as u64).to_le_bytes());
        digest.update(canonical_response);
        digest.finalize()
    };
    let record = format!(
        concat!(
            "{{\"authority\":\"none\",",
            "\"canonical_request_bytes\":{},",
            "\"canonical_request_hex\":\"{}\",",
            "\"canonical_request_sha256\":\"{}\",",
            "\"canonical_response_bytes\":{},",
            "\"canonical_response_hex\":\"{}\",",
            "\"canonical_response_sha256\":\"{}\",",
            "\"claim\":\"non-production-exact-artifact-observation-only\",",
            "\"final_hsaco_bytes\":{},",
            "\"final_hsaco_sha256\":\"{}\",",
            "\"finalization_identity\":\"{}\",",
            "\"finalized_output_identity\":\"{}\",",
            "\"publication_identity\":\"{}\",",
            "\"raw_output_bytes\":{},",
            "\"raw_output_hex\":\"{}\",",
            "\"raw_output_identity\":\"{}\",",
            "\"raw_output_sha256\":\"{}\",",
            "\"request_identity\":\"{}\",",
            "\"response_identity\":\"{}\",",
            "\"schema\":\"fe2o3-non-production-compiler-reproduction-record-v2\",",
            "\"sealed_worker_v2_response_identity\":\"{}\",",
            "\"worker_identity\":\"{}\",",
            "\"worker_v2_request_identity\":\"{}\"}}\n"
        ),
        canonical_request.len(),
        hex(canonical_request),
        hex(&Sha256::digest(canonical_request)),
        canonical_response.len(),
        hex(canonical_response),
        hex(&Sha256::digest(canonical_response)),
        exact_output.len(),
        hex(&Sha256::digest(exact_output)),
        hex(plan.finalization().as_bytes()),
        hex(plan.finalized_output().as_bytes()),
        hex(plan.publication().as_bytes()),
        raw_output.len(),
        hex(raw_output),
        hex(plan.linked_output().as_bytes()),
        hex(&Sha256::digest(raw_output)),
        hex(plan.request().as_bytes()),
        hex(plan.response().as_bytes()),
        hex(&sealed_worker_v2_response_identity),
        hex(plan.worker().as_bytes()),
        hex(worker_v2_request_identity),
    );
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .map_err(|error| {
            CompletionFailure::Uncommitted(format!(
                "cannot create non-production reproduction record: {error}"
            ))
        })?;
    file.write_all(record.as_bytes())
        .and_then(|()| file.sync_all())
        .and_then(|()| {
            let mut permissions = file.metadata()?.permissions();
            permissions.set_readonly(true);
            file.set_permissions(permissions)
        })
        .map_err(|error| {
            CompletionFailure::Uncommitted(format!(
                "cannot seal non-production reproduction record: {error}"
            ))
        })
}

fn complete_recovered_worker_v2(
    managed: &ManagedAttempt,
    resume: &WorkerV2ResumeStoreV1,
    state: ResumeMarkerStateV1,
) -> Result<(), CompletionFailure> {
    if matches!(state, ResumeMarkerStateV1::Completed { .. }) {
        return reconcile_completed_worker_v2(managed, resume, state);
    }
    let intent = match recover_worker_v2_intent_v1(resume, &managed.producer, state) {
        Ok(intent) => intent,
        Err(RestartIntentErrorV1::Intent(WorkerV2PublicationIntentErrorV1::NotFound))
            if matches!(state, ResumeMarkerStateV1::Pending { .. }) =>
        {
            resume
                .clear_abandoned_pending(state)
                .map_err(|error| preserve_marker_error("abandoned-pending cleanup", error))?;
            return Err(CompletionFailure::Uncommitted(
                "Worker V2 process stopped before its publication intent became durable".into(),
            ));
        }
        Err(error) => return Err(preserve_restart_error("recovery", error)),
    };
    publish_finish_and_clear(managed, resume, state.publication(), intent)
}

fn publish_finish_and_clear(
    managed: &ManagedAttempt,
    resume: &WorkerV2ResumeStoreV1,
    publication: WorkerV2PublicationKindV1,
    intent: RecoveredWorkerV2PublicationIntentV1,
) -> Result<(), CompletionFailure> {
    let record = intent.record();
    let intent_identity = record.identity();
    let receipt = publish_recovered_worker_v2(managed, &intent)?;
    #[cfg(feature = "worker-v2-fault-injection-test-only")]
    injected_fault_point_v1("published");
    let completed = if publication.requires_envelope() {
        let inputs = resume
            .recover_envelope_inputs(managed.attempt)
            .map_err(|error| preserve_marker_error("required envelope input recovery", error))?;
        let claim = recover_published_hsaco_claim_for_attempt_v1(
            &managed.output_dir,
            &managed.producer,
            managed.attempt,
            record.plan(),
            record.upstream_evidence(),
            receipt,
        )
        .map_err(|error| {
            CompletionFailure::PreserveAttempt(format!(
                "Worker V2 published-claim recovery failed: {error}"
            ))
        })?;
        let expected = assemble_recovered_worker_v2_load_envelope_v1(
            &managed.producer,
            record.plan(),
            record.upstream_evidence(),
            intent.exact_output(),
            claim,
            &inputs,
        )
        .map_err(|error| {
            CompletionFailure::PreserveAttempt(format!(
                "Worker V2 canonical envelope assembly failed: {error}"
            ))
        })?;
        resume.persist_envelope_and_completed(
            publication,
            managed.attempt,
            intent_identity,
            receipt,
            &expected,
        )
    } else {
        resume.persist_completed(publication, managed.attempt, intent_identity, receipt)
    }
    .map_err(|error| preserve_marker_error("completion persistence", error))?;
    #[cfg(feature = "worker-v2-fault-injection-test-only")]
    injected_fault_point_v1("completed");
    clear_worker_v2_publication_intent_v1(
        &managed.output_dir,
        &managed.producer,
        managed.attempt,
        intent_identity,
    )
    .map_err(|error| preserve_intent_error("cleanup", error))?;
    #[cfg(feature = "worker-v2-fault-injection-test-only")]
    injected_fault_point_v1("intent-cleared");
    finish_worker_v2_attempt(managed)?;
    #[cfg(feature = "worker-v2-fault-injection-test-only")]
    injected_fault_point_v1("finished");
    resume
        .clear_completed_and_envelope_inputs(completed)
        .map_err(|error| preserve_marker_error("cleanup", error))
}

fn reconcile_completed_worker_v2(
    managed: &ManagedAttempt,
    resume: &WorkerV2ResumeStoreV1,
    completed: ResumeMarkerStateV1,
) -> Result<(), CompletionFailure> {
    let ResumeMarkerStateV1::Completed {
        attempt,
        intent: intent_identity,
        receipt: expected_receipt,
        ..
    } = completed
    else {
        return Err(CompletionFailure::PreserveAttempt(
            "Worker V2 completion reconciliation received a non-completed marker".into(),
        ));
    };
    debug_assert_eq!(attempt, managed.attempt);
    let receipt = read_backend_publication_receipt_v1(
        &managed.output_dir,
        &managed.producer,
        managed.attempt,
    )
    .map_err(|error| {
        CompletionFailure::PreserveAttempt(format!(
            "Worker V2 completed-recovery receipt inspection failed: {error}"
        ))
    })?;
    let PersistedBackendReceiptV1::Provenance(receipt) = receipt else {
        return Err(CompletionFailure::PreserveAttempt(
            "Worker V2 completed resume marker has no exact durable publication receipt".into(),
        ));
    };
    if !expected_receipt.matches(receipt) {
        return Err(CompletionFailure::PreserveAttempt(
            "Worker V2 completed resume marker receipt was substituted".into(),
        ));
    }
    if completed.publication().requires_envelope() {
        validate_completed_worker_v2_envelope(managed, resume, completed, receipt)?;
    }
    match recover_worker_v2_intent_v1(resume, &managed.producer, completed) {
        Ok(intent) => {
            if intent.record().identity() != intent_identity {
                return Err(CompletionFailure::PreserveAttempt(
                    "Worker V2 completed resume marker disagrees with its exact journal authority"
                        .into(),
                ));
            }
            clear_worker_v2_publication_intent_v1(
                &managed.output_dir,
                &managed.producer,
                managed.attempt,
                intent_identity,
            )
            .map_err(|error| preserve_intent_error("completed recovery authorization", error))?;
        }
        Err(RestartIntentErrorV1::Intent(WorkerV2PublicationIntentErrorV1::NotFound)) => {}
        Err(error) => {
            return Err(preserve_restart_error(
                "completed recovery validation",
                error,
            ));
        }
    }
    finish_worker_v2_attempt(managed)?;
    resume
        .clear_completed_and_envelope_inputs(completed)
        .map_err(|error| preserve_marker_error("completed recovery cleanup", error))
}

fn validate_completed_worker_v2_envelope(
    managed: &ManagedAttempt,
    resume: &WorkerV2ResumeStoreV1,
    completed: ResumeMarkerStateV1,
    receipt: BackendPublicationReceiptV1,
) -> Result<(), CompletionFailure> {
    let envelope = resume.recover_load_envelope(receipt).map_err(|error| {
        CompletionFailure::PreserveAttempt(format!(
            "Worker V2 completed-recovery envelope inspection failed: {error}"
        ))
    })?;
    let inputs = resume
        .recover_envelope_inputs(managed.attempt)
        .map_err(|error| {
            CompletionFailure::PreserveAttempt(format!(
                "Worker V2 completed-recovery capsule inspection failed: {error}"
            ))
        })?;
    let claim = envelope.published_claim();
    let recovered_claim = recover_published_hsaco_claim_for_attempt_v1(
        &managed.output_dir,
        &managed.producer,
        managed.attempt,
        claim.plan(),
        claim.upstream_evidence(),
        receipt,
    )
    .map_err(|error| {
        CompletionFailure::PreserveAttempt(format!(
            "Worker V2 completed-recovery published claim failed: {error}"
        ))
    })?;
    let admission = restart_admission_commitment_with_inputs_v1(
        completed.publication(),
        claim.plan(),
        claim.upstream_evidence(),
        envelope.finalized_payload(),
        Some(inputs.identity()),
    );
    let expected = assemble_recovered_worker_v2_load_envelope_v1(
        &managed.producer,
        claim.plan(),
        claim.upstream_evidence(),
        envelope.finalized_payload(),
        recovered_claim,
        &inputs,
    )
    .map_err(|error| {
        CompletionFailure::PreserveAttempt(format!(
            "Worker V2 completed-recovery envelope reconstruction failed: {error}"
        ))
    })?;
    if admission != completed.admission()
        || completed.envelope_inputs() != inputs.identity().as_bytes()
        || completed.envelope() != envelope.identity().as_bytes()
        || claim.receipt() != receipt
        || claim.plan().attempt() != managed.attempt
        || completed.envelope() != expected.identity().as_bytes()
        || expected.to_bytes() != envelope.to_bytes()
        || envelope.grants_currentness_authority()
        || envelope.grants_load_authority()
        || envelope.grants_launch_authority()
    {
        return Err(CompletionFailure::PreserveAttempt(
            "Worker V2 completed resume marker disagrees with its canonical inert envelope".into(),
        ));
    }
    Ok(())
}

fn publish_recovered_worker_v2(
    managed: &ManagedAttempt,
    intent: &RecoveredWorkerV2PublicationIntentV1,
) -> Result<BackendPublicationReceiptV1, CompletionFailure> {
    const MAX_EXACT_RECONCILIATION_ATTEMPTS: usize = 3;
    let record = intent.record();
    for attempt in 1..=MAX_EXACT_RECONCILIATION_ATTEMPTS {
        match publish_exact_hsaco_evidence_for_attempt_v1(
            &managed.output_dir,
            &managed.producer,
            managed.attempt,
            record.plan(),
            record.upstream_evidence(),
            intent.exact_output(),
        ) {
            Ok(published) => return Ok(published.receipt()),
            Err(AttemptScopedHsacoPublicationErrorV1::ReceiptAlreadyPersisted { receipt }) => {
                return Ok(*receipt);
            }
            Err(
                AttemptScopedHsacoPublicationErrorV1::PublicationInterrupted(_)
                | AttemptScopedHsacoPublicationErrorV1::PublicationCommittedWithoutReceipt { .. },
            ) if attempt < MAX_EXACT_RECONCILIATION_ATTEMPTS => {}
            Err(error) => {
                return Err(CompletionFailure::PreserveAttempt(format!(
                    "Worker V2 journal publication failed after {attempt} attempts: {error}"
                )));
            }
        }
    }
    unreachable!("publication retry loop always returns")
}

fn finish_worker_v2_attempt(managed: &ManagedAttempt) -> Result<(), CompletionFailure> {
    const MAX_EXACT_RECONCILIATION_ATTEMPTS: usize = 3;
    for attempt in 1..=MAX_EXACT_RECONCILIATION_ATTEMPTS {
        match finish_build_attempt(&managed.output_dir, &managed.producer, managed.attempt) {
            Ok(()) => return Ok(()),
            Err(_) if attempt < MAX_EXACT_RECONCILIATION_ATTEMPTS => {}
            Err(error) => {
                return Err(CompletionFailure::PreserveAttempt(format!(
                    "published Worker V2 HSACO, but build-attempt completion failed after {attempt} attempts: {error}"
                )));
            }
        }
    }
    unreachable!("completion retry loop always returns")
}

fn preserve_restart_error(context: &str, error: RestartIntentErrorV1) -> CompletionFailure {
    CompletionFailure::PreserveAttempt(format!(
        "Worker V2 publication-intent {context} failed: {error}"
    ))
}

fn preserve_marker_error(context: &str, error: ResumeMarkerErrorV1) -> CompletionFailure {
    CompletionFailure::PreserveAttempt(format!("Worker V2 resume-marker {context} failed: {error}"))
}

fn preserve_intent_error(
    context: &str,
    error: WorkerV2PublicationIntentErrorV1,
) -> CompletionFailure {
    CompletionFailure::PreserveAttempt(format!(
        "Worker V2 publication-intent {context} failed: {error}"
    ))
}

#[cfg(unix)]
fn success_exit_status() -> ExitStatus {
    use std::os::unix::process::ExitStatusExt;
    ExitStatus::from_raw(0)
}

fn derive_build_attempt_input(
    argv: &[OsString],
    worker_v2: Option<&PreparedWorkerV2Config>,
    current_dir: &std::path::Path,
) -> BuildInvocation {
    derive_build_attempt_input_with_config_identity(
        argv,
        worker_v2.map(PreparedWorkerV2Config::identity),
        current_dir,
    )
}

fn derive_build_attempt_input_with_config_identity(
    argv: &[OsString],
    worker_v2_identity: Option<WorkerV2ConfigIdentity>,
    current_dir: &std::path::Path,
) -> BuildInvocation {
    let canonicalize = |value: &OsStr| {
        if crate::non_production_reproduction::enabled() {
            crate::non_production_reproduction::canonicalize_argument(value)
        } else {
            os_bytes(value).to_vec()
        }
    };
    let mut digest = Sha256::new();
    digest.update(BUILD_ATTEMPT_INPUT_DOMAIN);
    hash_bytes(&mut digest, &canonicalize(current_dir.as_os_str()));
    hash_bytes(
        &mut digest,
        &canonicalize(std::env::var_os(TARGET_ENV).as_deref().unwrap_or_default()),
    );
    hash_bytes(
        &mut digest,
        &canonicalize(
            std::env::var_os(HSACO_DIR_ENV)
                .as_deref()
                .unwrap_or_default(),
        ),
    );
    digest.update((argv.len() as u64).to_le_bytes());
    for argument in argv {
        hash_bytes(&mut digest, &canonicalize(argument));
    }
    if let Some(worker_v2_identity) = worker_v2_identity {
        digest.update(WORKER_V2_CONFIG_ID_DOMAIN);
        digest.update(worker_v2_identity.as_bytes());
    }
    BuildInvocation::from_bytes(digest.finalize().into())
}

fn hash_bytes(digest: &mut Sha256, bytes: &[u8]) {
    digest.update((bytes.len() as u64).to_le_bytes());
    digest.update(bytes);
}

fn row_softmax_effective_rustc_argv_identity(argv: &[OsString]) -> BuildInvocation {
    let mut digest = Sha256::new();
    digest.update(ROW_SOFTMAX_EFFECTIVE_RUSTC_ARGV_DOMAIN_V1);
    digest.update((argv.len() as u64).to_le_bytes());
    for argument in argv {
        hash_bytes(&mut digest, os_bytes(argument));
    }
    BuildInvocation::from_bytes(digest.finalize().into())
}

fn is_cargo_stdin_probe(argv: &[OsString]) -> bool {
    argv.get(1).is_some_and(|argument| argument == "-")
        && argv.iter().skip(2).any(|argument| {
            argument == "--print"
                || argument
                    .to_str()
                    .is_some_and(|argument| argument.starts_with("--print="))
        })
}

fn ordered_metadata_values(argv: &[OsString]) -> Result<Vec<String>, BindingWrapperError> {
    let mut metadata = Vec::new();
    let mut index = 1;
    while index < argv.len() {
        let argument = &argv[index];
        if argument == "-C" || argument == "--codegen" {
            let value_index = index + 1;
            let value = argv
                .get(value_index)
                .expect("the invocation classifier checked separate option values");
            inspect_codegen_value(value, value_index, &mut metadata)?;
            index += 2;
            continue;
        }

        if let Some(argument) = argument.to_str() {
            if let Some(value) = argument.strip_prefix("-C") {
                inspect_codegen_text(value, index, &mut metadata)?;
            } else if let Some(value) = argument.strip_prefix("--codegen=") {
                inspect_codegen_text(value, index, &mut metadata)?;
            }
        }
        index += 1;
    }
    Ok(metadata)
}

fn canonicalize_rustc_metadata(argv: &mut [OsString]) {
    let canonical = crate::non_production_reproduction::canonical_metadata();
    let mut index = 1;
    while index < argv.len() {
        if argv[index] == "-C" || argv[index] == "--codegen" {
            if let Some(value) = argv.get_mut(index + 1)
                && value
                    .to_str()
                    .is_some_and(|value| value.starts_with("metadata="))
            {
                *value = OsString::from(format!("metadata={canonical}"));
            }
            index += 2;
            continue;
        }
        if let Some(value) = argv[index].to_str() {
            if value.starts_with("-Cmetadata=") {
                argv[index] = OsString::from(format!("-Cmetadata={canonical}"));
            } else if value.starts_with("--codegen=metadata=") {
                argv[index] = OsString::from(format!("--codegen=metadata={canonical}"));
            }
        }
        index += 1;
    }
}

fn inspect_codegen_value(
    value: &OsStr,
    argument_index: usize,
    metadata: &mut Vec<String>,
) -> Result<(), BindingWrapperError> {
    let value = value
        .to_str()
        .ok_or(BindingWrapperError::InvalidCodegenOption { argument_index })?;
    inspect_codegen_text(value, argument_index, metadata)
}

fn inspect_codegen_text(
    value: &str,
    argument_index: usize,
    metadata: &mut Vec<String>,
) -> Result<(), BindingWrapperError> {
    let Some(value) = value.strip_prefix("metadata=") else {
        return Ok(());
    };
    if value.is_empty() {
        return Err(BindingWrapperError::EmptyMetadata { argument_index });
    }
    metadata.push(value.to_owned());
    Ok(())
}

pub(crate) fn exit_code(status: ExitStatus) -> u8 {
    status
        .code()
        .and_then(|code| u8::try_from(code).ok())
        .unwrap_or(1)
}

#[cfg(test)]
mod tests {
    use super::{
        BindingWrapperError, BuildExecutableSnapshot,
        CARGO_FE2O3_EXECUTABLE_BUILD_OBSERVATION_ENV_V2, CARGO_METADATA_BUILD_OBSERVATION_ENV_V2,
        CompileBuildObservationV2, CompleteReviewedChildEnvironmentV2,
        DECLARED_CARGO_EXECUTABLE_BUILD_OBSERVATION_ENV_V2, LLVM_BUILD_IDENTITY_OBSERVATION_ENV_V2,
        LinuxObjectIdentityV3, OBSERVED_PARENT_PID_BUILD_OBSERVATION_ENV_V2,
        OBSERVED_PARENT_START_TIME_BUILD_OBSERVATION_ENV_V2,
        PINNED_CARGO_IMAGE_BUILD_OBSERVATION_ENV_V2, PreparedRustcConsistencyExpectation,
        ROW_SOFTMAX_EFFECTIVE_RUSTC_ARGV_DOMAIN_V1, ROW_SOFTMAX_V1_PIPELINE,
        WORKER_BUILD_IDENTITY_OBSERVATION_ENV_V2, WORKER_CONFIG_BUILD_OBSERVATION_ENV_V2,
        WORKER_EXECUTABLE_BUILD_OBSERVATION_ENV_V2, append_prepared_rustc_arguments,
        canonicalize_rustc_metadata, configure_build_observation_environment,
        configure_worker_build_observation_environment, decode_managed_rustc_args,
        derive_build_attempt_input_with_config_identity, is_cargo_stdin_probe,
        materialize_reviewed_child_environment, materialize_row_softmax_v1_child_environment,
        materialize_s09_child_environment, materialize_scalar_gemm_v1_child_environment,
        measure_build_executable, observe_pinned_cargo_image_and_parent, ordered_metadata_values,
        os_bytes, prepared_rustc_command_sha256, process_start_time_ticks,
        reject_authority_linker_arguments, reject_uninspectable_rustc_args,
        resolve_command_executable_with_path, row_softmax_effective_rustc_argv_identity,
    };
    use crate::inert_rustc_invocation_capture::InertRustcInvocationCaptureV2;
    use crate::pinned_executable::PinnedExecutable;
    use crate::worker_v2::{
        WorkerV2BuildObservation, WorkerV2CompileEnvironmentProfileV1, WorkerV2ConfigIdentity,
    };
    use crate::worker_v2_restart::{
        WorkerV2PublicationKindV1, WorkerV2ResumeStoreV1,
        restart_admission_commitment_with_inputs_v1,
    };
    use fe2o3_artifact_transaction::{
        AtomicPublicationIdentityV1, BuildInvocation, BuildSession, CanonicalLinkRequestIdentityV1,
        DurableLinkPublicationPlanV1, FinalizationIdentityV1, FinalizedOutputIdentityV1,
        KernelSetIdentityV1, LinkPublicationScopeV1, LinkedOutputIdentityV1, PackageIdentityV1,
        PinnedWorkerIdentityV1, ProducerIdentity, TargetIdentityV1,
        UpstreamCodeObjectEvidenceIdentityV1, ValidatedResponseIdentityV1, begin_build_attempt,
        persist_worker_v2_publication_intent_v1, publish_exact_hsaco_evidence_for_attempt_v1,
        recover_worker_v2_publication_intent_v1,
    };
    use reserved_fe2o3_symbols::{CRATE_BINDING_ID_ENV_V1, derive_crate_binding_id_v1};
    use sha2::Digest;
    use std::ffi::{OsStr, OsString};
    use std::fs;
    #[cfg(target_os = "linux")]
    use std::path::Path;
    use std::process::Command;

    fn args(values: &[&str]) -> Vec<OsString> {
        values.iter().map(OsString::from).collect()
    }

    fn command_with_production_managed_arguments(forwarded: &[&str]) -> Command {
        let encoded =
            crate::generation::managed_rustc_args(Path::new("/proc/./self/fd/198"), [0x12; 16])
                .unwrap();
        let managed = decode_managed_rustc_args(&encoded).unwrap();
        let mut command = Command::new("/proc/self/fd/9");
        append_prepared_rustc_arguments(&mut command, &args(forwarded), &managed).unwrap();
        command
    }

    #[test]
    fn extracts_every_supported_metadata_form_in_order() {
        let argv = args(&[
            "rustc",
            "--crate-name",
            "unit",
            "unit.rs",
            "-C",
            "metadata=first",
            "-Cmetadata=second",
            "--codegen",
            "metadata=third",
            "--codegen=metadata=fourth",
            "-Copt-level=2",
        ]);

        assert_eq!(
            ordered_metadata_values(&argv).unwrap(),
            ["first", "second", "third", "fourth"]
        );
    }

    #[test]
    fn canonicalizes_every_supported_rustc_metadata_form() {
        let mut argv = args(&[
            "rustc",
            "-C",
            "metadata=first",
            "-Cmetadata=second",
            "--codegen",
            "metadata=third",
            "--codegen=metadata=fourth",
            "-Copt-level=2",
        ]);
        canonicalize_rustc_metadata(&mut argv);
        assert_eq!(
            ordered_metadata_values(&argv).unwrap(),
            [crate::non_production_reproduction::canonical_metadata(); 4]
        );
        assert_eq!(argv.last().unwrap(), "-Copt-level=2");
    }

    #[test]
    fn crate_name_and_ordered_metadata_distinguish_compilation_units() {
        let first = derive_crate_binding_id_v1("unit", ["first", "second"]);
        let reordered = derive_crate_binding_id_v1("unit", ["second", "first"]);
        let renamed = derive_crate_binding_id_v1("other", ["first", "second"]);
        assert_ne!(first, reordered);
        assert_ne!(first, renamed);
    }

    #[test]
    fn metadata_build_observation_preserves_order_and_duplicates() {
        let ordered = ["first", "second", "first"].map(String::from);
        let reordered = ["first", "first", "second"].map(String::from);
        let deduplicated = ["first", "second"].map(String::from);

        let other_crate =
            CompileBuildObservationV2::from_ordered_metadata("other", &ordered).unwrap();
        let ordered = CompileBuildObservationV2::from_ordered_metadata("unit", &ordered).unwrap();
        let reordered =
            CompileBuildObservationV2::from_ordered_metadata("unit", &reordered).unwrap();
        let deduplicated =
            CompileBuildObservationV2::from_ordered_metadata("unit", &deduplicated).unwrap();

        assert_ne!(
            ordered.cargo_metadata_digest,
            reordered.cargo_metadata_digest
        );
        assert_ne!(
            ordered.cargo_metadata_digest,
            deduplicated.cargo_metadata_digest
        );
        assert_ne!(ordered.crate_binding, reordered.crate_binding);
        assert_ne!(ordered.crate_binding, deduplicated.crate_binding);
        assert_eq!(
            ordered.cargo_metadata_digest_hex(),
            other_crate.cargo_metadata_digest_hex()
        );
        assert_ne!(ordered.crate_binding, other_crate.crate_binding);
        assert_eq!(
            ordered.cargo_metadata_digest_hex(),
            "02bb68e8c8b5aa67c836f32263beaa4738b50f4689f0622d75130fbf9f7008a9"
        );
    }

    #[test]
    fn metadata_build_observation_rejects_missing_metadata() {
        let error = CompileBuildObservationV2::from_ordered_metadata("unit", &[]).unwrap_err();
        assert!(matches!(
            error,
            BindingWrapperError::MissingMetadata { ref crate_name } if crate_name == "unit"
        ));
    }

    #[test]
    fn build_observation_handoff_is_digest_only_and_cleared_when_absent() {
        let private_metadata = [
            "private-checkout-fingerprint".to_owned(),
            "private-checkout-fingerprint".to_owned(),
        ];
        let observation =
            CompileBuildObservationV2::from_ordered_metadata("unit", &private_metadata).unwrap();
        let mut compile = Command::new("rustc");
        configure_build_observation_environment(&mut compile, Some(observation));

        let compile_environment = compile
            .get_envs()
            .map(|(name, value)| {
                (
                    name.to_string_lossy().into_owned(),
                    value.map(|value| value.to_string_lossy().into_owned()),
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(
            compile_environment,
            [
                (
                    CARGO_METADATA_BUILD_OBSERVATION_ENV_V2.to_owned(),
                    Some(observation.cargo_metadata_digest_hex()),
                ),
                (
                    CRATE_BINDING_ID_ENV_V1.to_owned(),
                    Some(observation.crate_binding.to_hex()),
                ),
            ]
        );
        let rendered = format!("{observation:?} {compile_environment:?}");
        assert!(!rendered.contains("private-checkout-fingerprint"));

        let mut non_compile = Command::new("rustc");
        configure_build_observation_environment(&mut non_compile, None);
        assert_eq!(
            non_compile
                .get_envs()
                .map(|(name, value)| (name.to_owned(), value.map(OsString::from)))
                .collect::<Vec<_>>(),
            [
                (
                    OsString::from(CARGO_METADATA_BUILD_OBSERVATION_ENV_V2),
                    None
                ),
                (OsString::from(CRATE_BINDING_ID_ENV_V1), None),
            ]
        );
    }

    #[test]
    fn s09_worker_observation_propagates_every_exact_digest_and_clears_absence() {
        let observation = WorkerV2BuildObservation {
            config_identity: WorkerV2ConfigIdentity::for_test([0x11; 32]),
            executable_sha256: [0x12; 32],
            worker_build_identity: "worker-build-v2",
            llvm_build_identity: "llvm-build-v2",
            prepared_rustc_command_sha256: [0x13; 32],
            cargo_fe2o3_executable_sha256: [0x14; 32],
            declared_cargo_executable_sha256: [0x15; 32],
            pinned_cargo_image_sha256: [0x16; 32],
            observed_parent_pid: 17,
            observed_parent_start_time_ticks: 18,
        };
        let mut command = Command::new("rustc");
        configure_worker_build_observation_environment(&mut command, Some(observation));
        let environment = command
            .get_envs()
            .map(|(name, value)| {
                (
                    name.to_string_lossy().into_owned(),
                    value
                        .expect("configured observation value")
                        .to_string_lossy()
                        .into_owned(),
                )
            })
            .collect::<std::collections::BTreeMap<_, _>>();
        assert_eq!(environment.len(), 9);
        for (name, expected) in [
            (WORKER_CONFIG_BUILD_OBSERVATION_ENV_V2, "11".repeat(32)),
            (WORKER_EXECUTABLE_BUILD_OBSERVATION_ENV_V2, "12".repeat(32)),
            (
                CARGO_FE2O3_EXECUTABLE_BUILD_OBSERVATION_ENV_V2,
                "14".repeat(32),
            ),
            (
                DECLARED_CARGO_EXECUTABLE_BUILD_OBSERVATION_ENV_V2,
                "15".repeat(32),
            ),
            (PINNED_CARGO_IMAGE_BUILD_OBSERVATION_ENV_V2, "16".repeat(32)),
            (
                OBSERVED_PARENT_PID_BUILD_OBSERVATION_ENV_V2,
                "17".to_owned(),
            ),
            (
                OBSERVED_PARENT_START_TIME_BUILD_OBSERVATION_ENV_V2,
                "18".to_owned(),
            ),
            (
                WORKER_BUILD_IDENTITY_OBSERVATION_ENV_V2,
                "worker-build-v2".to_owned(),
            ),
            (
                LLVM_BUILD_IDENTITY_OBSERVATION_ENV_V2,
                "llvm-build-v2".to_owned(),
            ),
        ] {
            assert_eq!(environment.get(name), Some(&expected));
        }

        let mut absent = Command::new("rustc");
        configure_worker_build_observation_environment(&mut absent, None);
        assert_eq!(absent.get_envs().count(), 9);
        assert!(absent.get_envs().all(|(_, value)| value.is_none()));
    }

    #[test]
    fn executable_observation_hashes_the_real_running_test_image() {
        let executable = std::env::current_exe().unwrap();
        let expected: [u8; 32] = sha2::Sha256::digest(fs::read(&executable).unwrap()).into();
        assert_eq!(
            measure_build_executable(&executable, "test executable").unwrap(),
            expected
        );
    }

    #[test]
    fn executable_snapshot_detects_change_time_differences() {
        let initial = BuildExecutableSnapshot {
            device: 1,
            inode: 2,
            mode: 0o100700,
            size: 3,
            modified_seconds: 4,
            modified_nanoseconds: 5,
            changed_seconds: 6,
            changed_nanoseconds: 7,
        };

        assert_ne!(
            initial,
            BuildExecutableSnapshot {
                changed_seconds: 8,
                ..initial
            }
        );
        assert_ne!(
            initial,
            BuildExecutableSnapshot {
                changed_nanoseconds: 8,
                ..initial
            }
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn pinned_rustc_execution_ignores_later_path_disagreement() {
        use std::os::unix::fs::PermissionsExt;

        let root = std::env::temp_dir().join(format!(
            "fe2o3-rustc-path-disagreement-{}",
            std::process::id()
        ));
        let first = root.join("first");
        let second = root.join("second");
        fs::create_dir_all(&first).unwrap();
        fs::create_dir_all(&second).unwrap();
        for (source, destination) in [
            (Path::new("/bin/true"), first.join("rustc")),
            (Path::new("/bin/false"), second.join("rustc")),
        ] {
            fs::copy(source, &destination).unwrap();
            let mut permissions = fs::metadata(&destination).unwrap().permissions();
            permissions.set_mode(0o700);
            fs::set_permissions(destination, permissions).unwrap();
        }
        let first_path = std::env::join_paths([&first]).unwrap();
        let second_path = std::env::join_paths([&second]).unwrap();
        let selected =
            resolve_command_executable_with_path(OsStr::new("rustc"), &root, Some(&first_path))
                .unwrap();
        let pinned = PinnedExecutable::open(&selected).unwrap();
        let disagreed =
            resolve_command_executable_with_path(OsStr::new("rustc"), &root, Some(&second_path))
                .unwrap();
        assert_ne!(selected, disagreed);
        assert!(pinned.command().unwrap().status().unwrap().success());
        assert!(!Command::new(disagreed).status().unwrap().success());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_empty_metadata() {
        let error = ordered_metadata_values(&args(&[
            "rustc",
            "--crate-name",
            "unit",
            "unit.rs",
            "-Cmetadata=",
        ]))
        .unwrap_err();
        assert!(matches!(
            error,
            BindingWrapperError::EmptyMetadata { argument_index: 4 }
        ));
    }

    #[cfg(unix)]
    #[test]
    fn rejects_non_utf8_metadata_without_rendering_its_bytes() {
        use std::os::unix::ffi::OsStringExt;

        let invalid = OsString::from_vec(b"metadata=private-\xff-value".to_vec());
        let argv = vec![
            OsString::from("rustc"),
            OsString::from("unit.rs"),
            OsString::from("-C"),
            invalid,
        ];
        let error = ordered_metadata_values(&argv).unwrap_err();
        assert!(matches!(
            error,
            BindingWrapperError::InvalidCodegenOption { argument_index: 3 }
        ));
        assert_eq!(
            error.to_string(),
            "rustc codegen option at argv[3] is not valid UTF-8"
        );
    }

    #[test]
    fn recognizes_only_stdin_print_probe_shape() {
        assert!(is_cargo_stdin_probe(&args(&[
            "rustc",
            "-",
            "--crate-name",
            "___",
            "--print=file-names",
        ])));
        assert!(!is_cargo_stdin_probe(&args(&[
            "rustc",
            "source.rs",
            "--print=file-names",
        ])));
        assert!(!is_cargo_stdin_probe(&args(&[
            "rustc",
            "-",
            "--crate-name",
            "real_compile",
        ])));
    }

    #[test]
    fn attempt_input_identity_is_deterministic_and_argument_order_sensitive() {
        let first = args(&["rustc", "--crate-name", "unit", "unit.rs"]);
        let second = args(&["rustc", "unit.rs", "--crate-name", "unit"]);
        let current_dir = std::env::current_dir().unwrap();
        assert_eq!(
            derive_build_attempt_input_with_config_identity(&first, None, &current_dir),
            derive_build_attempt_input_with_config_identity(&first, None, &current_dir)
        );
        assert_ne!(
            derive_build_attempt_input_with_config_identity(&first, None, &current_dir),
            derive_build_attempt_input_with_config_identity(&second, None, &current_dir)
        );
    }

    #[test]
    fn row_softmax_attempt_identity_covers_the_exact_effective_rustc_argv() {
        let argv = args(&[
            "/toolchain/bin/rustc",
            "src/lib.rs",
            "--crate-name",
            "row_softmax",
            "-Zmir-enable-passes=-JumpThreading",
            "--cfg",
            "fe2o3_codegen_generation=\"0123456789abcdef0123456789abcdef\"",
            "-Zcodegen-backend=/proc/./self/fd/198",
        ]);
        let identity = row_softmax_effective_rustc_argv_identity(&argv);

        let mut oracle = sha2::Sha256::new();
        oracle.update(ROW_SOFTMAX_EFFECTIVE_RUSTC_ARGV_DOMAIN_V1);
        oracle.update((argv.len() as u64).to_le_bytes());
        for argument in &argv {
            let bytes = os_bytes(argument);
            oracle.update((bytes.len() as u64).to_le_bytes());
            oracle.update(bytes);
        }
        assert_eq!(identity.as_bytes(), &<[u8; 32]>::from(oracle.finalize()));

        for index in 0..argv.len() {
            let mut changed = argv.clone();
            changed[index].push("-changed");
            assert_ne!(
                row_softmax_effective_rustc_argv_identity(&changed),
                identity,
                "argv[{index}] was not bound"
            );
        }
        let mut reordered = argv.clone();
        reordered.swap(1, 2);
        assert_ne!(
            row_softmax_effective_rustc_argv_identity(&reordered),
            identity
        );
    }

    #[test]
    fn worker_v2_config_identity_changes_attempt_input() {
        let argv = args(&["rustc", "--crate-name", "unit", "unit.rs"]);
        let current_dir = std::env::current_dir().unwrap();
        let first = WorkerV2ConfigIdentity::for_test([0x11; 32]);
        let second = WorkerV2ConfigIdentity::for_test([0x12; 32]);
        assert_ne!(
            derive_build_attempt_input_with_config_identity(&argv, Some(first), &current_dir),
            derive_build_attempt_input_with_config_identity(&argv, Some(second), &current_dir)
        );
    }

    #[test]
    fn prepared_command_consistency_covers_every_parent_observation() {
        fn identity(
            argv0: &str,
            arguments: &[String],
            environment: &[(String, Option<String>)],
            executable_object: LinuxObjectIdentityV3,
            executable_sha256: [u8; 32],
            current_dir_object: LinuxObjectIdentityV3,
            protected_source_tree_sha256: [u8; 32],
        ) -> [u8; 32] {
            let mut command = Command::new("/proc/self/fd/9");
            command.args(arguments);
            for (name, value) in environment {
                match value {
                    Some(value) => {
                        command.env(name, value);
                    }
                    None => {
                        command.env_remove(name);
                    }
                }
            }
            let complete_environment = CompleteReviewedChildEnvironmentV2::from_command(&command);
            prepared_rustc_command_sha256(
                &command,
                OsStr::new(argv0),
                executable_object,
                executable_sha256,
                current_dir_object,
                protected_source_tree_sha256,
                &complete_environment,
            )
            .unwrap()
        }

        let arguments = [
            "--crate-name",
            "unit",
            "unit.rs",
            "-Zmir-enable-passes=-JumpThreading",
            "--cfg",
            "fe2o3_codegen_generation=\"0123456789abcdef0123456789abcdef\"",
            "-Zcodegen-backend=/proc/./self/fd/198",
        ]
        .map(String::from);
        let environment = [
            ("FE2O3_BUILD_ATTEMPT_V1", Some("attempt")),
            ("FE2O3_HSACO_DIR", Some("/proc/self/fd/199")),
            ("FE2O3_CARGO_METADATA_BUILD_OBSERVATION_V2", Some("01")),
            ("FE2O3_CODEGEN_BACKEND_BUILD_OBSERVATION_V2", Some("02")),
            ("FE2O3_WORKER_CONFIG_BUILD_OBSERVATION_V2", Some("03")),
            ("FE2O3_WORKER_EXECUTABLE_BUILD_OBSERVATION_V2", Some("04")),
            ("FE2O3_WORKER_BUILD_IDENTITY_OBSERVATION_V2", Some("worker")),
            ("FE2O3_LLVM_BUILD_IDENTITY_OBSERVATION_V2", Some("llvm")),
            (
                "FE2O3_CARGO_FE2O3_EXECUTABLE_BUILD_OBSERVATION_V2",
                Some("05"),
            ),
            (
                "FE2O3_DECLARED_CARGO_EXECUTABLE_BUILD_OBSERVATION_V2",
                Some("06"),
            ),
            ("FE2O3_PINNED_CARGO_IMAGE_BUILD_OBSERVATION_V2", Some("07")),
            ("FE2O3_OBSERVED_PARENT_PID_BUILD_OBSERVATION_V2", Some("17")),
            (
                "FE2O3_OBSERVED_PARENT_START_TIME_BUILD_OBSERVATION_V2",
                Some("18"),
            ),
            ("FE2O3_WORKER_V2_SOURCE_DEBUG_PROFILE_V1", Some("s09")),
            ("REMOVED_MANAGED_INPUT", None),
        ]
        .map(|(name, value)| (name.to_owned(), value.map(str::to_owned)));
        let executable_object = LinuxObjectIdentityV3::from_linux_stat(1, 2, 0o100755);
        let current_dir_object = LinuxObjectIdentityV3::from_linux_stat(3, 4, 0o40700);
        let baseline = identity(
            "/toolchain/rustc",
            &arguments,
            &environment,
            executable_object,
            [0x31; 32],
            current_dir_object,
            [0x41; 32],
        );

        for index in 0..arguments.len() {
            let mut changed = arguments.clone();
            changed[index].push_str("-changed");
            assert_ne!(
                identity(
                    "/toolchain/rustc",
                    &changed,
                    &environment,
                    executable_object,
                    [0x31; 32],
                    current_dir_object,
                    [0x41; 32],
                ),
                baseline,
                "argv[{index}] was not covered"
            );
        }
        for index in 0..environment.len() {
            let mut changed = environment.clone();
            changed[index].1 = match &changed[index].1 {
                Some(value) => Some(format!("{value}-changed")),
                None => Some("restored".to_owned()),
            };
            assert_ne!(
                identity(
                    "/toolchain/rustc",
                    &arguments,
                    &changed,
                    executable_object,
                    [0x31; 32],
                    current_dir_object,
                    [0x41; 32],
                ),
                baseline,
                "environment mutation {} was not covered",
                environment[index].0
            );
        }
        assert_ne!(
            identity(
                "/other/argv0",
                &arguments,
                &environment,
                executable_object,
                [0x31; 32],
                current_dir_object,
                [0x41; 32],
            ),
            baseline
        );
        assert_ne!(
            identity(
                "/toolchain/rustc",
                &arguments,
                &environment,
                LinuxObjectIdentityV3::from_linux_stat(1, 9, 0o100755),
                [0x31; 32],
                current_dir_object,
                [0x41; 32],
            ),
            baseline
        );
        assert_ne!(
            identity(
                "/toolchain/rustc",
                &arguments,
                &environment,
                executable_object,
                [0x32; 32],
                current_dir_object,
                [0x41; 32],
            ),
            baseline
        );
        assert_ne!(
            identity(
                "/toolchain/rustc",
                &arguments,
                &environment,
                executable_object,
                [0x31; 32],
                LinuxObjectIdentityV3::from_linux_stat(3, 8, 0o40700),
                [0x41; 32],
            ),
            baseline
        );
        assert_ne!(
            identity(
                "/toolchain/rustc",
                &arguments,
                &environment,
                executable_object,
                [0x31; 32],
                current_dir_object,
                [0x42; 32],
            ),
            baseline
        );
    }

    #[test]
    fn s09_environment_is_explicit_complete_and_identity_bound() {
        fn prepared(inherited: &[(&str, &str)]) -> (Command, [u8; 32]) {
            let mut command = Command::new("/proc/self/fd/9");
            command
                .arg("--crate-name=unit")
                .current_dir("/workspace")
                .env("LANG", "C.UTF-8")
                .env("PATH", "/usr/bin")
                .env("TMPDIR", "/proc/self/fd/197/private")
                .env("FE2O3_BUILD_ATTEMPT_V1", "attempt")
                .env_remove("FE2O3_HSACO_DIR");
            let complete_environment = materialize_s09_child_environment(
                &mut command,
                inherited
                    .iter()
                    .map(|(name, value)| (OsString::from(name), OsString::from(value))),
            )
            .unwrap();
            let digest = prepared_rustc_command_sha256(
                &command,
                OsStr::new("/toolchain/rustc"),
                LinuxObjectIdentityV3::from_linux_stat(1, 2, 0o100755),
                [0x44; 32],
                LinuxObjectIdentityV3::from_linux_stat(3, 4, 0o40700),
                [0x45; 32],
                &complete_environment,
            )
            .unwrap();
            (command, digest)
        }

        let inherited = [
            ("CARGO_MANIFEST_DIR", "/workspace"),
            ("FE2O3_CODEGEN_PIPELINE", "kernel-ir-worker-v2"),
            ("FE2O3_TARGET", "gfx942:xnack-"),
            ("CARGO_PKG_NAME", "unit"),
            ("CUSTOM_BUILD_INPUT", "first"),
            ("FE2O3_S09_COMPILE_ENV_ALLOWLIST_V2", "CUSTOM_BUILD_INPUT"),
            ("RUSTC_WORKSPACE_WRAPPER", "/already-consumed/wrapper"),
        ];
        let (command, baseline) = prepared(&inherited);
        let effective = command
            .get_envs()
            .filter_map(|(name, value)| value.map(|value| (name, value)))
            .collect::<std::collections::BTreeMap<_, _>>();
        assert_eq!(
            effective.get(OsStr::new("CARGO_MANIFEST_DIR")),
            Some(&OsStr::new("/workspace"))
        );
        assert_eq!(
            effective.get(OsStr::new("LANG")),
            Some(&OsStr::new("C.UTF-8"))
        );
        assert_eq!(
            effective.get(OsStr::new("TMPDIR")),
            Some(&OsStr::new("/proc/self/fd/197/private"))
        );
        assert_eq!(
            effective
                .keys()
                .map(|name| name.to_str().unwrap())
                .collect::<Vec<_>>(),
            [
                "CARGO_MANIFEST_DIR",
                "FE2O3_BUILD_ATTEMPT_V1",
                "FE2O3_CODEGEN_PIPELINE",
                "FE2O3_TARGET",
                "LANG",
                "PATH",
                "TMPDIR",
            ]
        );

        let mut inherited_change = inherited;
        inherited_change[0].1 = "/other-workspace";
        assert_ne!(prepared(&inherited_change).1, baseline);
        let mut ignored_change = inherited;
        ignored_change[6].1 = "second";
        assert_eq!(prepared(&ignored_change).1, baseline);

        let mut removed = Command::new("/toolchain/rustc");
        removed.current_dir("/workspace").env_remove("OPTIONAL");
        let removed_environment = CompleteReviewedChildEnvironmentV2::from_command(&removed);
        let removed = prepared_rustc_command_sha256(
            &removed,
            OsStr::new("/toolchain/rustc"),
            LinuxObjectIdentityV3::from_linux_stat(1, 2, 0o100755),
            [0x44; 32],
            LinuxObjectIdentityV3::from_linux_stat(3, 4, 0o40700),
            [0x45; 32],
            &removed_environment,
        )
        .unwrap();
        let mut empty = Command::new("/toolchain/rustc");
        empty.current_dir("/workspace").env("OPTIONAL", "");
        let empty_environment = CompleteReviewedChildEnvironmentV2::from_command(&empty);
        let empty = prepared_rustc_command_sha256(
            &empty,
            OsStr::new("/toolchain/rustc"),
            LinuxObjectIdentityV3::from_linux_stat(1, 2, 0o100755),
            [0x44; 32],
            LinuxObjectIdentityV3::from_linux_stat(3, 4, 0o40700),
            [0x45; 32],
            &empty_environment,
        )
        .unwrap();
        assert_ne!(
            removed, empty,
            "removed and empty environments were conflated"
        );
    }

    #[test]
    fn fresh_s09_command_capture_uses_production_argument_order_and_path() {
        let mut command = command_with_production_managed_arguments(&[
            "--crate-name",
            "s09_alpha",
            "/workspace/src/lib.rs",
        ]);
        command
            .env("LANG", "C.UTF-8")
            .env("PATH", "/usr/bin")
            .env("TMPDIR", "/proc/self/fd/197/private")
            .env("FE2O3_HSACO_DIR", "/proc/self/fd/197")
            .env("FE2O3_BUILD_ATTEMPT_V1", "attempt")
            .env(
                "FE2O3_CODEGEN_BACKEND_BUILD_OBSERVATION_V2",
                "44".repeat(32),
            );
        let inherited = [
            ("CARGO_MANIFEST_DIR", "/workspace"),
            ("FE2O3_CODEGEN_PIPELINE", "kernel-ir-worker-v2"),
            ("FE2O3_TARGET", "gfx942:xnack-"),
        ]
        .map(|(name, value)| (OsString::from(name), OsString::from(value)));
        let complete = materialize_reviewed_child_environment(
            Some(WorkerV2CompileEnvironmentProfileV1::S09AlphaGfx942O0),
            &mut command,
            inherited,
        )
        .unwrap()
        .unwrap();
        let capture = InertRustcInvocationCaptureV2::capture(
            &command,
            OsStr::new("/toolchains/rustc"),
            Path::new("/workspace"),
            &complete.entries,
            [0x33; 32],
            [0x44; 32],
        )
        .unwrap();
        let argv = capture.descriptor().rustc().argv().collect::<Vec<_>>();

        assert_eq!(
            argv.last().copied(),
            Some("-Zcodegen-backend=/proc/./self/fd/198")
        );
        assert_eq!(
            argv.iter()
                .filter(|argument| argument.starts_with("-Zcodegen-backend="))
                .count(),
            1
        );
        assert_eq!(
            capture.descriptor().codegen_backend_path(),
            "/proc/./self/fd/198"
        );
    }

    #[test]
    fn ordinary_command_keeps_the_rustc_compatible_brokered_selector() {
        let command = command_with_production_managed_arguments(&[
            "--crate-name",
            "ordinary",
            "/workspace/src/lib.rs",
        ]);
        let arguments = command.get_args().collect::<Vec<_>>();

        assert_eq!(
            arguments.last().copied(),
            Some(OsStr::new("-Zcodegen-backend=/proc/./self/fd/198"))
        );
        assert_eq!(
            arguments
                .iter()
                .filter(|argument| os_bytes(argument).starts_with(b"-Zcodegen-backend="))
                .count(),
            1
        );
    }

    fn scalar_environment() -> Vec<(OsString, OsString)> {
        [
            ("CARGO_MANIFEST_DIR", "/workspace/scalar"),
            ("FE2O3_CODEGEN_PIPELINE", "collected-scalar-gemm-v1"),
            ("FE2O3_TARGET", "gfx942:xnack-"),
            ("FE2O3_VERIFY_KERNEL_IR", "1"),
            ("FE2O3_BACKEND", "/outer/backend.so"),
            ("FE2O3_BINDING_WRAPPER_MODE_V1", "1"),
            ("FE2O3_MANAGED_RUSTC_ARGS_V1", "consumed"),
            ("FE2O3_BUILD_SESSION_V1", "consumed"),
            ("FE2O3_CAPABILITY_BROKER_V1", "consumed"),
            ("FE2O3_HOST_PASSTHROUGH", "0"),
            ("FE2O3_WORKER_V2_CONFIG_V2", "/outer/config.json"),
            ("FE2O3_WORKER_V2_EXPECTED_ID_V1", "consumed"),
            ("HOME", "/discarded/home"),
            ("CARGO_PKG_NAME", "discarded-package"),
        ]
        .map(|(name, value)| (OsString::from(name), OsString::from(value)))
        .into()
    }

    fn scalar_command() -> Command {
        let attempt = format!("1:{}:{}", "11".repeat(16), "22".repeat(32));
        let mut command = command_with_production_managed_arguments(&[
            "--crate-name",
            "fe2o3_scalar_gemm_v1",
            "/workspace/scalar/src/lib.rs",
        ]);
        command
            .env("LANG", "C.UTF-8")
            .env("PATH", "/usr/bin")
            .env("TMPDIR", "/proc/self/fd/197/private")
            .env("FE2O3_HSACO_DIR", "/proc/self/fd/197")
            .env("FE2O3_BUILD_ATTEMPT_V1", attempt)
            .env("FE2O3_CARGO_METADATA_BUILD_OBSERVATION_V2", "33".repeat(32))
            .env(
                "FE2O3_CODEGEN_BACKEND_BUILD_OBSERVATION_V2",
                "44".repeat(32),
            )
            .env("FE2O3_CRATE_BINDING_ID_V1", "55".repeat(32))
            .env_remove("FE2O3_WORKER_V2_SOURCE_DEBUG_PROFILE_V1");
        command
    }

    #[test]
    fn scalar_environment_is_closed_and_available_to_inert_capture() {
        let mut command = scalar_command();
        let complete =
            materialize_scalar_gemm_v1_child_environment(&mut command, scalar_environment())
                .unwrap();
        let effective = command
            .get_envs()
            .filter_map(|(name, value)| value.map(|value| (name, value)))
            .collect::<std::collections::BTreeMap<_, _>>();

        assert_eq!(
            effective.get(OsStr::new("FE2O3_CODEGEN_PIPELINE")),
            Some(&OsStr::new("collected-scalar-gemm-v1"))
        );
        assert_eq!(
            effective.get(OsStr::new("FE2O3_TARGET")),
            Some(&OsStr::new("gfx942:xnack-"))
        );
        assert_eq!(
            effective.get(OsStr::new("FE2O3_VERIFY_KERNEL_IR")),
            Some(&OsStr::new("1"))
        );
        assert_eq!(
            effective.get(OsStr::new("FE2O3_HSACO_DIR")),
            Some(&OsStr::new("/proc/self/fd/197"))
        );
        assert!(effective.contains_key(OsStr::new("FE2O3_BUILD_ATTEMPT_V1")));
        assert!(effective.contains_key(OsStr::new("FE2O3_CODEGEN_BACKEND_BUILD_OBSERVATION_V2")));
        for discarded in [
            "HOME",
            "CARGO_PKG_NAME",
            "FE2O3_BACKEND",
            "FE2O3_CAPABILITY_BROKER_V1",
            "FE2O3_WORKER_V2_CONFIG_V2",
        ] {
            assert!(!effective.contains_key(OsStr::new(discarded)));
        }

        let capture = InertRustcInvocationCaptureV2::capture(
            &command,
            OsStr::new("/toolchains/rustc"),
            Path::new("/workspace/scalar"),
            &complete.entries,
            [0x66; 32],
            [0x44; 32],
        )
        .unwrap();
        assert_eq!(capture.descriptor().amd_target(), "gfx942:xnack-");
        assert_eq!(
            capture.descriptor().artifact_output_directory(),
            "/proc/self/fd/197"
        );
        assert!(capture.descriptor().verification_required());
        let argv = capture.descriptor().rustc().argv().collect::<Vec<_>>();
        assert_eq!(
            argv.last().copied(),
            Some("-Zcodegen-backend=/proc/./self/fd/198")
        );
        assert_eq!(
            argv.iter()
                .filter(|argument| argument.starts_with("-Zcodegen-backend="))
                .count(),
            1
        );

        let mut disabled = scalar_environment();
        disabled.retain(|(name, _)| name != "FE2O3_VERIFY_KERNEL_IR");
        let mut disabled_command = scalar_command();
        materialize_scalar_gemm_v1_child_environment(&mut disabled_command, disabled).unwrap();
        assert_eq!(
            disabled_command
                .get_envs()
                .find(|(name, _)| *name == OsStr::new("FE2O3_VERIFY_KERNEL_IR"))
                .and_then(|(_, value)| value),
            Some(OsStr::new("0"))
        );
    }

    #[test]
    fn scalar_environment_rejects_secrets_and_unreviewed_fe2o3_controls() {
        for name in [
            "AWS_SECRET_ACCESS_KEY",
            "PRIVATE_TOKEN",
            "FE2O3_VERBOSE",
            "FE2O3_UNREVIEWED_COMPILER_CONTROL",
        ] {
            let mut inherited = scalar_environment();
            inherited.push((OsString::from(name), OsString::from("not-forwarded")));
            let error =
                materialize_scalar_gemm_v1_child_environment(&mut scalar_command(), inherited)
                    .unwrap_err();
            assert!(error.to_string().contains(name));
        }
    }

    #[test]
    fn scalar_environment_rejects_missing_and_changed_target() {
        let mut missing = scalar_environment();
        missing.retain(|(name, _)| name != "FE2O3_TARGET");
        assert!(
            materialize_scalar_gemm_v1_child_environment(&mut scalar_command(), missing).is_err()
        );

        let mut changed = scalar_environment();
        changed
            .iter_mut()
            .find(|(name, _)| name == "FE2O3_TARGET")
            .unwrap()
            .1 = OsString::from("gfx942:xnack+");
        assert!(
            materialize_scalar_gemm_v1_child_environment(&mut scalar_command(), changed).is_err()
        );
    }

    #[cfg(unix)]
    #[test]
    fn scalar_environment_rejects_non_utf8_names_and_values() {
        use std::os::unix::ffi::OsStringExt as _;

        let mut name = scalar_environment();
        name.push((OsString::from_vec(vec![0xff]), OsString::from("value")));
        assert!(materialize_scalar_gemm_v1_child_environment(&mut scalar_command(), name).is_err());

        let mut value = scalar_environment();
        value.push((OsString::from("IGNORED"), OsString::from_vec(vec![0xff])));
        assert!(
            materialize_scalar_gemm_v1_child_environment(&mut scalar_command(), value).is_err()
        );
    }

    #[test]
    fn scalar_environment_rejects_explicit_mutation() {
        for (name, value) in [
            ("CUSTOM_ENV_MACRO_INPUT", "unsupported"),
            ("FE2O3_TARGET", "gfx1100"),
            ("FE2O3_WORKER_V2_SOURCE_DEBUG_PROFILE_V1", "s09"),
        ] {
            let mut command = scalar_command();
            command.env(name, value);
            let error =
                materialize_scalar_gemm_v1_child_environment(&mut command, scalar_environment())
                    .unwrap_err();
            assert!(error.to_string().contains(name) || error.to_string().contains("S09"));
        }
    }

    #[test]
    fn ordinary_compile_environment_is_not_inspected_or_changed() {
        let mut command = Command::new("/toolchains/rustc");
        command
            .arg("--crate-name=ordinary")
            .env("ORDINARY_EXPLICIT_INPUT", "preserved");
        let before = format!("{command:?}");
        let result = materialize_reviewed_child_environment(
            None,
            &mut command,
            [
                (OsString::from("PRIVATE_TOKEN"), OsString::from("ignored")),
                (
                    OsString::from("FE2O3_UNREVIEWED_COMPILER_CONTROL"),
                    OsString::from("ignored"),
                ),
            ],
        )
        .unwrap();

        assert!(result.is_none());
        assert_eq!(format!("{command:?}"), before);
    }

    #[test]
    fn s09_environment_rejects_loader_and_bootstrap_controls() {
        for name in [
            "LD_PRELOAD",
            "LD_AUDIT",
            "LD_LIBRARY_PATH",
            "LD_DEBUG",
            "DYLD_INSERT_LIBRARIES",
            "GLIBC_TUNABLES",
            "RUSTC_BOOTSTRAP",
            "PRIVATE_TOKEN",
            "AWS_SECRET_ACCESS_KEY",
        ] {
            let mut command = Command::new("/toolchain/rustc");
            command
                .current_dir("/workspace")
                .env("LANG", "C.UTF-8")
                .env("TMPDIR", "/proc/self/fd/197/private");
            let mut inherited = vec![
                (
                    OsString::from("CARGO_MANIFEST_DIR"),
                    OsString::from("/workspace"),
                ),
                (
                    OsString::from("FE2O3_CODEGEN_PIPELINE"),
                    OsString::from("kernel-ir-worker-v2"),
                ),
                (
                    OsString::from("FE2O3_TARGET"),
                    OsString::from("gfx942:xnack-"),
                ),
            ];
            inherited.push((OsString::from(name), OsString::from("forbidden")));
            let error = materialize_s09_child_environment(&mut command, inherited).unwrap_err();
            assert!(error.to_string().contains(name));
        }
    }

    #[test]
    fn row_softmax_environment_is_complete_and_rejects_loader_controls() {
        let fixed = || {
            [
                (
                    OsString::from("CARGO_MANIFEST_DIR"),
                    OsString::from("/workspace/row"),
                ),
                (
                    OsString::from("FE2O3_CODEGEN_PIPELINE"),
                    OsString::from(ROW_SOFTMAX_V1_PIPELINE),
                ),
                (
                    OsString::from("FE2O3_TARGET"),
                    OsString::from("gfx942:xnack-"),
                ),
            ]
        };
        let mut command = Command::new("/proc/self/fd/194");
        command
            .env("LANG", "C.UTF-8")
            .env("PATH", "/usr/bin")
            .env("TMPDIR", "/proc/self/fd/197/private")
            .env("LD_LIBRARY_PATH", "/proc/self/fd/193")
            .env_remove("LD_PRELOAD")
            .env(crate::EXPECTED_COMPILER_CLOSURE_SHA256_ENV, "ab".repeat(32))
            .env("FE2O3_BUILD_ATTEMPT_V1", "attempt");
        let mut inherited = fixed().to_vec();
        inherited.push((
            OsString::from(crate::EXPECTED_COMPILER_CLOSURE_SHA256_ENV),
            OsString::from("01".repeat(32)),
        ));
        let complete = materialize_row_softmax_v1_child_environment(&mut command, inherited)
            .expect("materialize reviewed row-softmax environment");
        assert!(complete.entries.contains(&(
            OsString::from("LD_LIBRARY_PATH"),
            OsString::from("/proc/self/fd/193")
        )));
        assert!(
            !complete
                .entries
                .iter()
                .any(|(name, _)| name == "LD_PRELOAD")
        );
        assert!(complete.entries.contains(&(
            OsString::from(crate::EXPECTED_COMPILER_CLOSURE_SHA256_ENV),
            OsString::from("ab".repeat(32)),
        )));

        for name in [
            "LD_PRELOAD",
            "LD_AUDIT",
            "LD_LIBRARY_PATH",
            "LD_DEBUG",
            "DYLD_INSERT_LIBRARIES",
            "GLIBC_TUNABLES",
        ] {
            let mut command = Command::new("/proc/self/fd/194");
            command
                .env("LANG", "C.UTF-8")
                .env("PATH", "/usr/bin")
                .env("TMPDIR", "/proc/self/fd/197/private")
                .env(crate::EXPECTED_COMPILER_CLOSURE_SHA256_ENV, "ab".repeat(32));
            let mut inherited = fixed().to_vec();
            inherited.push((OsString::from(name), OsString::from("attacker")));
            let error =
                materialize_row_softmax_v1_child_environment(&mut command, inherited).unwrap_err();
            assert!(error.to_string().contains(name), "{error}");
        }
    }

    #[test]
    fn s09_environment_rejects_custom_explicit_inputs() {
        let mut command = Command::new("/toolchain/rustc");
        command
            .current_dir("/workspace")
            .env("LANG", "C.UTF-8")
            .env("TMPDIR", "/proc/self/fd/197/private")
            .env("CUSTOM_ENV_MACRO_INPUT", "unsupported");
        let error = materialize_s09_child_environment(
            &mut command,
            [
                (
                    OsString::from("CARGO_MANIFEST_DIR"),
                    OsString::from("/workspace"),
                ),
                (
                    OsString::from("FE2O3_CODEGEN_PIPELINE"),
                    OsString::from("kernel-ir-worker-v2"),
                ),
                (
                    OsString::from("FE2O3_TARGET"),
                    OsString::from("gfx942:xnack-"),
                ),
            ],
        )
        .unwrap_err();
        assert!(error.to_string().contains("CUSTOM_ENV_MACRO_INPUT"));
    }

    #[test]
    fn prepared_digest_finalization_does_not_mutate_the_command() {
        use std::io::Read as _;
        use std::os::fd::AsRawFd as _;

        let mut command = Command::new("/proc/self/fd/9");
        command
            .arg("--crate-name=unit")
            .current_dir("/workspace")
            .env_clear()
            .env("PATH", "/reviewed/bin");
        let mut expectation = PreparedRustcConsistencyExpectation::attach(&mut command).unwrap();
        let before = format!("{command:?}");
        let complete_environment = CompleteReviewedChildEnvironmentV2::from_command(&command);
        let digest = prepared_rustc_command_sha256(
            &command,
            OsStr::new("/toolchain/rustc"),
            LinuxObjectIdentityV3::from_linux_stat(1, 2, 0o100755),
            [0x55; 32],
            LinuxObjectIdentityV3::from_linux_stat(3, 4, 0o40700),
            [0x56; 32],
            &complete_environment,
        )
        .unwrap();
        expectation.finalize(digest).unwrap();
        assert_eq!(format!("{command:?}"), before);
        let mut observed = [0_u8; 32];
        fs::File::open(format!("/proc/self/fd/{}", expectation.image.as_raw_fd()))
            .unwrap()
            .read_exact(&mut observed)
            .unwrap();
        assert_eq!(observed, digest);
    }

    #[test]
    fn parent_pid_and_start_time_are_diagnostic_observations() {
        let pid = u64::from(std::process::id());
        assert_ne!(process_start_time_ticks(pid).unwrap(), 0);
        let observed = observe_pinned_cargo_image_and_parent([0x5a; 32]).unwrap();
        assert_eq!(observed.pinned_cargo_image_sha256, [0x5a; 32]);
        assert_ne!(observed.observed_parent_pid, 0);
        assert_ne!(observed.observed_parent_start_time_ticks, 0);
    }

    #[test]
    fn managed_rustc_arguments_are_exact_and_brokered() {
        let value = OsString::from(
            "-Zmir-enable-passes=-JumpThreading\x1f--cfg\x1ffe2o3_codegen_generation=\"0123456789abcdef0123456789abcdef\"\x1f-Zcodegen-backend=/proc/./self/fd/198",
        );
        let decoded = decode_managed_rustc_args(&value).unwrap();
        assert_eq!(decoded.len(), 4);
        assert_eq!(
            decoded.last().map(OsString::as_os_str),
            Some(OsStr::new("-Zcodegen-backend=/proc/./self/fd/198"))
        );

        for invalid in [
            OsString::from(""),
            OsString::from(
                "-Zmir-enable-passes=-JumpThreading\x1f--cfg\x1ffe2o3_codegen_generation=\"0123456789abcdef0123456789abcdef\"\x1f-Zcodegen-backend=/tmp/backend",
            ),
            OsString::from(
                "-Zmir-enable-passes=-JumpThreading\x1f--cfg\x1ffe2o3_codegen_generation=\"ABCDEF0123456789abcdef0123456789\"\x1f-Zcodegen-backend=/proc/self/fd/198",
            ),
            OsString::from(
                "-Zcodegen-backend=/proc/./self/fd/198\x1f-Zmir-enable-passes=-JumpThreading\x1f--cfg\x1ffe2o3_codegen_generation=\"0123456789abcdef0123456789abcdef\"",
            ),
            OsString::from(
                "-Zmir-enable-passes=-JumpThreading\x1f--cfg\x1ffe2o3_codegen_generation=\"0123456789abcdef0123456789abcdef\"\x1f-Zcodegen-backend=/proc/self/fd/198",
            ),
        ] {
            assert!(decode_managed_rustc_args(&invalid).is_err());
        }
    }

    #[test]
    fn managed_rustc_arguments_reject_an_option_terminator_before_mutation() {
        let mut command = Command::new("/toolchain/rustc");
        let forwarded = args(&["--crate-name", "unit", "unit.rs", "--"]);
        let managed = args(&["-Zcodegen-backend=/proc/./self/fd/198"]);

        assert!(matches!(
            append_prepared_rustc_arguments(&mut command, &forwarded, &managed),
            Err(BindingWrapperError::OptionTerminatorBeforeManagedArguments { argument_index: 4 })
        ));
        assert_eq!(command.get_args().count(), 0);
    }

    #[test]
    fn cargo_rustflags_cannot_select_a_backend_or_hide_in_response_files() {
        for argv in [
            args(&["rustc", "unit.rs", "-Zcodegen-backend=/tmp/evil.so"]),
            args(&["rustc", "unit.rs", "-Zcodegen_backend=/tmp/evil.so"]),
            args(&["rustc", "unit.rs", "-Z=codegen-backend=/tmp/evil.so"]),
            args(&["rustc", "unit.rs", "-Z=codegen_backend=/tmp/evil.so"]),
            args(&["rustc", "unit.rs", "-Z", "codegen-backend=/tmp/evil.so"]),
            args(&["rustc", "unit.rs", "-Z", "codegen_backend=/tmp/evil.so"]),
            args(&["rustc", "unit.rs", "-Z", "codegen-backend", "/tmp/evil.so"]),
            args(&["rustc", "unit.rs", "-Zcodegen-backend", "/tmp/evil.so"]),
            args(&["rustc", "unit.rs", "@response"]),
        ] {
            assert!(reject_uninspectable_rustc_args(&argv).is_err());
        }
        assert!(
            reject_uninspectable_rustc_args(&args(&[
                "rustc",
                "unit.rs",
                "--cfg",
                "from_cargo_config"
            ]))
            .is_ok()
        );
    }

    #[test]
    fn authority_rustflags_cannot_select_or_program_a_linker() {
        for argv in [
            args(&["rustc", "unit.rs", "-Clinker=/tmp/evil"]),
            args(&["rustc", "unit.rs", "-C", "link-arg=-fplugin=/tmp/evil"]),
            args(&["rustc", "unit.rs", "-Zgcc-ld=/tmp/evil"]),
            args(&["rustc", "unit.rs", "-Z", "linker-features=+lld"]),
        ] {
            assert!(matches!(
                reject_authority_linker_arguments(&argv),
                Err(BindingWrapperError::AuthorityLinkerOverride { .. })
            ));
        }
        assert!(
            reject_authority_linker_arguments(&args(&[
                "rustc",
                "unit.rs",
                "-Coverflow-checks=off",
                "--cfg",
                "reviewed"
            ]))
            .is_ok()
        );
    }

    #[test]
    fn required_finalized_completion_cannot_bypass_the_envelope_transition() {
        let directory = std::env::temp_dir().join(format!(
            "cargo-fe2o3-completed-envelope-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&directory);
        fs::create_dir(&directory).unwrap();
        let producer = ProducerIdentity::from_codegen(
            "completed_envelope",
            Some(Path::new("/workspace/completed_envelope.rs")),
        )
        .unwrap();
        let attempt = begin_build_attempt(
            &directory,
            &producer,
            BuildInvocation::from_bytes([0x11; 32]),
            BuildSession::from_bytes([0x12; 16]),
        )
        .unwrap();
        let output = b"finalized-envelope-output";
        let finalized: [u8; 32] = sha2::Sha256::digest(output).into();
        let plan = DurableLinkPublicationPlanV1::new(
            attempt,
            LinkPublicationScopeV1::new(
                PackageIdentityV1::from_bytes([0x21; 32]),
                KernelSetIdentityV1::from_bytes([0x22; 32]),
                TargetIdentityV1::from_bytes([0x23; 32]),
            ),
            CanonicalLinkRequestIdentityV1::from_bytes([0x24; 32]),
            PinnedWorkerIdentityV1::from_bytes([0x25; 32]),
            ValidatedResponseIdentityV1::from_bytes([0x26; 32]),
            LinkedOutputIdentityV1::from_bytes([0x27; 32]),
            FinalizationIdentityV1::from_bytes([0x28; 32]),
            FinalizedOutputIdentityV1::from_bytes(finalized),
            AtomicPublicationIdentityV1::from_bytes([0x29; 32]),
        );
        let upstream = UpstreamCodeObjectEvidenceIdentityV1::from_bytes([0x31; 32]);
        let required = WorkerV2PublicationKindV1::FinalizedEnvelopeRequired;
        let input_identity =
            fe2o3_worker_v2_bundle::WorkerV2EnvelopeInputsIdentityV1::from_bytes([0x32; 32]);
        let admission = restart_admission_commitment_with_inputs_v1(
            required,
            plan,
            upstream,
            output,
            Some(input_identity),
        );
        let store = WorkerV2ResumeStoreV1::open(&directory, &producer).unwrap();
        store
            .persist_pending_with_envelope_inputs(
                required,
                attempt,
                admission,
                Some(input_identity),
            )
            .unwrap();
        let intent = persist_worker_v2_publication_intent_v1(
            &directory, &producer, attempt, plan, upstream, output,
        )
        .unwrap();
        store
            .persist_ready(required, attempt, intent.record().identity())
            .unwrap();
        let ready = store.load().unwrap().unwrap();
        let publication = publish_exact_hsaco_evidence_for_attempt_v1(
            &directory, &producer, attempt, plan, upstream, output,
        )
        .unwrap();
        assert!(matches!(
            store.persist_completed(
                required,
                attempt,
                intent.record().identity(),
                publication.receipt(),
            ),
            Err(crate::worker_v2_restart::ResumeMarkerErrorV1::InvalidTransition)
        ));
        assert_eq!(store.load().unwrap(), Some(ready));
        assert!(recover_worker_v2_publication_intent_v1(&directory, &producer, attempt).is_ok());
        drop(publication);
        drop(store);
        fs::remove_dir_all(&directory).unwrap();
    }
}
