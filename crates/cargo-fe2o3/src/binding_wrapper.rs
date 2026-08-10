use std::collections::{BTreeMap, BTreeSet};
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
    AttemptScopedHsacoPublicationErrorV1, BackendPublicationReceiptV1, BuildAttempt,
    BuildInvocation, BuildSession, EmitError, PersistedBackendReceiptV1, ProducerIdentity,
    RecoveredWorkerV2PublicationIntentV1, WorkerV2PublicationIntentErrorV1, begin_build_attempt,
    clear_worker_v2_publication_intent_v1, consume_compiler_module_handoff_v1, fail_build_attempt,
    finish_build_attempt, publish_exact_hsaco_evidence_for_attempt_v1,
    read_backend_publication_receipt_v1, recover_published_hsaco_claim_for_attempt_v1,
};
use fe2o3_hsaco_finalize::inspect_worker_v2_raw_hsaco_v1;
use fe2o3_process_identity::{PreparedCommandIdentityV2, prepared_command_digest_v2};
use fe2o3_rustc_invocation::{
    RustcArgsErrorV2, RustcCompileInvocationV2, RustcInvocationV2, classify_rustc_invocation_v2,
};
use fe2o3_worker_v2_bundle::WorkerV2EnvelopeInputsV1;
use reserved_fe2o3_symbols::{
    CRATE_BINDING_ID_ENV_V1, CrateBindingIdV1, derive_crate_binding_id_v1,
};
use sha2::{Digest, Sha256};

use crate::capability_broker;
use crate::pinned_codegen_backend::{PinCodegenBackendError, PinnedCodegenBackend};
use crate::pinned_executable::{PinExecutableError, PinnedExecutable};
use crate::project::PinnedDirectory;
use crate::worker_v2::{
    PreparedWorkerV2Config, WORKER_V2_EXPECTED_ID_ENV, WORKER_V2_SOURCE_DEBUG_PROFILE_ENV,
    WorkerV2BuildObservation, WorkerV2ConfigError, WorkerV2ConfigIdentity,
    WorkerV2SourceDebugProfileV1,
};
use crate::worker_v2_artifact_container::assemble_recovered_worker_v2_load_envelope_v1;
#[cfg(feature = "worker-v2-fault-injection-test-only")]
use crate::worker_v2_restart::injected_fault_point_v1;
use crate::worker_v2_restart::{
    RestartIntentErrorV1, ResumeMarkerErrorV1, ResumeMarkerStateV1, WorkerV2PublicationKindV1,
    WorkerV2ResumeStoreV1, persist_admitted_worker_v2_intent_v1, recover_worker_v2_intent_v1,
    restart_admission_commitment_with_inputs_v1,
};
use crate::{ARTIFACT_CHILD_FD, BACKEND_CHILD_FD, MANAGED_RUSTC_ARGS_ENV};

const HSACO_DIR_ENV: &str = "FE2O3_HSACO_DIR";
const TARGET_ENV: &str = "FE2O3_TARGET";
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
const PREPARED_RUSTC_COMMAND_OBSERVATION_FD_V2: std::os::fd::RawFd =
    fe2o3_process_identity::S09_PREPARED_COMMAND_EXPECTATION_FD_V2;
const S09_COMPILE_ENV_ALLOWLIST_ENV_V2: &str = "FE2O3_S09_COMPILE_ENV_ALLOWLIST_V2";
const MAX_S09_COMPILE_ENV_NAMES_V2: usize = 64;
const BUILD_ATTEMPT_INPUT_DOMAIN: &[u8] = b"FE2O3/BUILD-ATTEMPT-INPUT/V2\0";
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
    ManagedBackend(PinCodegenBackendError),
    PinnedExecutable(PinExecutableError),
    ManagedArtifact(String),
    CapabilityBroker(String),
    ChildCapability(String),
    UninspectableRustcResponseFile {
        argument_index: usize,
    },
    PreexistingCodegenBackend {
        argument_index: usize,
    },
    CurrentDirectory(std::io::Error),
    BuildProvenance(String),
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
            Self::ManagedBackend(error) => {
                write!(formatter, "failed to pin managed codegen backend: {error}")
            }
            Self::PinnedExecutable(error) => {
                write!(formatter, "failed to pin rustc executable: {error}")
            }
            Self::ManagedArtifact(error) => {
                write!(
                    formatter,
                    "failed to pin managed artifact directory: {error}"
                )
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
            Self::CurrentDirectory(error) => {
                write!(
                    formatter,
                    "failed to resolve rustc working directory: {error}"
                )
            }
            Self::BuildProvenance(error) => {
                write!(formatter, "failed to measure build provenance: {error}")
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
            Self::ManagedBackend(error) => Some(error),
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
            | Self::ManagedArtifact(_)
            | Self::CapabilityBroker(_)
            | Self::ChildCapability(_)
            | Self::UninspectableRustcResponseFile { .. }
            | Self::PreexistingCodegenBackend { .. }
            | Self::UnsupportedInvocation => None,
            Self::BuildProvenance(_) => None,
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

pub(crate) fn run(argv: Vec<OsString>) -> Result<ExitStatus, BindingWrapperError> {
    reject_uninspectable_rustc_args(&argv)?;
    let invocation = match classify_rustc_invocation_v2(&argv) {
        Ok(invocation) => invocation,
        Err(_) if is_cargo_stdin_probe(&argv) => {
            let current_dir =
                std::env::current_dir().map_err(BindingWrapperError::CurrentDirectory)?;
            let executable = resolve_command_executable(&argv[0], &current_dir)?;
            let pinned = PinnedExecutable::open(&executable)?;
            let mut command = pinned.command()?;
            command.args(&argv[1..]);
            configure_build_observation_environment(command.as_command_mut(), None);
            return command.status().map_err(BindingWrapperError::Spawn);
        }
        Err(error) => return Err(error.into()),
    };
    let (
        build_observation,
        managed_attempt,
        managed_rustc_args,
        compiler_capabilities,
        rustc_working_directory,
    ) = match invocation {
        RustcInvocationV2::Compile(compile) => {
            let managed_rustc_args = managed_rustc_args_from_environment()?;
            let compiler_capabilities = CompilerCapabilities::from_environment()?;
            let metadata = ordered_metadata_values(compile.argv())?;
            let build_observation =
                CompileBuildObservationV2::from_ordered_metadata(compile.crate_name(), &metadata)?;
            let worker_v2 = PreparedWorkerV2Config::from_environment()
                .map_err(BindingWrapperError::WorkerV2Configuration)?;
            validate_expected_worker_v2_identity(worker_v2.as_ref())?;
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
    let rustc_path = resolve_command_executable(invocation.executable(), &execution_directory)?;
    let pinned_rustc = PinnedExecutable::open(&rustc_path)?;
    let mut command = pinned_rustc.command_by_canonical_path()?;
    command.args(invocation.forwarded_args());
    command.args(managed_rustc_args);
    if let Some(current_dir) = &rustc_working_directory {
        command.as_command_mut().current_dir(current_dir);
    }
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
    let mut worker_build_observation = managed_attempt
        .as_ref()
        .map(|managed| {
            let pinned_cargo_image_sha256 = compiler_capabilities
                .as_ref()
                .map(CompilerCapabilities::pinned_cargo_image_sha256)
                .ok_or_else(|| {
                    BindingWrapperError::BuildProvenance(
                        "S09 build has no brokered pinned Cargo image observation".to_owned(),
                    )
                })?;
            managed.worker_build_observation(pinned_cargo_image_sha256)
        })
        .transpose()?
        .flatten();
    configure_worker_build_observation_environment(
        command.as_command_mut(),
        worker_build_observation,
    );
    let complete_s09_environment = worker_build_observation
        .is_some()
        .then(|| materialize_s09_child_environment(command.as_command_mut(), std::env::vars_os()))
        .transpose()?;
    let mut prepared_command_capability = if worker_build_observation.is_some() {
        Some(PreparedRustcCommandDigestCapability::attach(
            command.as_command_mut(),
        )?)
    } else {
        None
    };
    if let Some(observation) = worker_build_observation.as_mut() {
        observation.prepared_rustc_command_sha256 = prepared_rustc_command_sha256(
            command.as_command(),
            *pinned_rustc.sha256(),
            complete_s09_environment
                .as_ref()
                .expect("S09 complete child environment exists"),
        )?;
        prepared_command_capability
            .as_mut()
            .expect("S09 prepared-command capability exists")
            .finalize(observation.prepared_rustc_command_sha256)?;
    }
    let status = match command.status() {
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
        }
    }
}

fn measure_build_executable(
    path: impl AsRef<Path>,
    label: &str,
) -> Result<[u8; 32], BindingWrapperError> {
    let path = path.as_ref();
    let file = File::open(path).map_err(|error| {
        BindingWrapperError::BuildProvenance(format!(
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
        BindingWrapperError::BuildProvenance(format!("{label} {}: {reason}", path.display()))
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
        BindingWrapperError::BuildProvenance(
            "CARGO is missing from the rustc wrapper environment".to_owned(),
        )
    })?;
    if value.is_empty() {
        return Err(BindingWrapperError::BuildProvenance(
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
        return Err(BindingWrapperError::BuildProvenance(
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
        BindingWrapperError::BuildProvenance(
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
    Err(BindingWrapperError::BuildProvenance(format!(
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
        BindingWrapperError::BuildProvenance("wrapper has no observed parent PID".to_owned())
    })?;
    let pid = u64::try_from(initial_parent.as_raw_nonzero().get()).map_err(|_| {
        BindingWrapperError::BuildProvenance("observed parent PID is negative".to_owned())
    })?;
    let initial_start_time = process_start_time_ticks(pid)?;
    let final_start_time = process_start_time_ticks(pid)?;
    let final_parent = rustix::process::getppid().ok_or_else(|| {
        BindingWrapperError::BuildProvenance("observed parent disappeared".to_owned())
    })?;
    if final_parent != initial_parent || final_start_time != initial_start_time {
        return Err(BindingWrapperError::BuildProvenance(
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
        BindingWrapperError::BuildProvenance(format!(
            "cannot read observed parent {}: {error}",
            path.display()
        ))
    })?;
    if bytes.is_empty() || bytes.len() > MAX_PROC_STAT_BYTES {
        return Err(BindingWrapperError::BuildProvenance(format!(
            "observed parent {} must contain 1 through {MAX_PROC_STAT_BYTES} bytes",
            path.display()
        )));
    }
    let close = bytes
        .iter()
        .rposition(|byte| *byte == b')')
        .ok_or_else(|| {
            BindingWrapperError::BuildProvenance(
                "observed parent stat has no command terminator".to_owned(),
            )
        })?;
    let recorded_pid = bytes[..close]
        .split(|byte| *byte == b' ')
        .next()
        .and_then(|value| std::str::from_utf8(value).ok())
        .and_then(|value| value.parse::<u64>().ok());
    if recorded_pid != Some(pid) {
        return Err(BindingWrapperError::BuildProvenance(
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
            BindingWrapperError::BuildProvenance(
                "observed parent stat has no valid start-time field".to_owned(),
            )
        })?;
    Ok(start_time)
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CompleteS09ChildEnvironmentV2 {
    entries: Vec<(OsString, OsString)>,
}

impl CompleteS09ChildEnvironmentV2 {
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

fn materialize_s09_child_environment(
    command: &mut Command,
    inherited: impl IntoIterator<Item = (OsString, OsString)>,
) -> Result<CompleteS09ChildEnvironmentV2, BindingWrapperError> {
    let inherited = inherited.into_iter().collect::<BTreeMap<_, _>>();
    let explicit_policy = parse_s09_compile_environment_policy(
        inherited
            .get(OsStr::new(S09_COMPILE_ENV_ALLOWLIST_ENV_V2))
            .map(OsString::as_os_str),
    )?;
    for name in inherited.keys() {
        if forbidden_s09_environment(name) {
            return Err(BindingWrapperError::BuildProvenance(format!(
                "S09 child environment rejects inherited variable {name:?}"
            )));
        }
        if compilation_control_environment(name)
            && !reviewed_s09_inherited_environment(name)
            && !consumed_before_s09_rustc_environment(name)
            && !explicit_policy.contains(name)
        {
            return Err(BindingWrapperError::BuildProvenance(format!(
                "S09 child environment has non-allowlisted compilation control {name:?}"
            )));
        }
    }

    let mut final_environment = inherited
        .into_iter()
        .filter(|(name, _)| {
            reviewed_s09_inherited_environment(name) || explicit_policy.contains(name)
        })
        .collect::<BTreeMap<_, _>>();
    let explicit = command
        .get_envs()
        .map(|(name, value)| (name.to_owned(), value.map(OsString::from)))
        .collect::<Vec<_>>();
    for (name, value) in explicit {
        if !managed_s09_child_environment(&name) {
            return Err(BindingWrapperError::BuildProvenance(format!(
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
    Ok(CompleteS09ChildEnvironmentV2 {
        entries: final_environment.into_iter().collect(),
    })
}

fn parse_s09_compile_environment_policy(
    value: Option<&OsStr>,
) -> Result<BTreeSet<OsString>, BindingWrapperError> {
    let Some(value) = value else {
        return Ok(BTreeSet::new());
    };
    let value = value.to_str().ok_or_else(|| {
        BindingWrapperError::BuildProvenance(format!(
            "{S09_COMPILE_ENV_ALLOWLIST_ENV_V2} is not UTF-8"
        ))
    })?;
    if value.is_empty() {
        return Err(BindingWrapperError::BuildProvenance(format!(
            "{S09_COMPILE_ENV_ALLOWLIST_ENV_V2} must not be empty when present"
        )));
    }
    let names = value.split(',').collect::<Vec<_>>();
    if names.len() > MAX_S09_COMPILE_ENV_NAMES_V2 {
        return Err(BindingWrapperError::BuildProvenance(format!(
            "{S09_COMPILE_ENV_ALLOWLIST_ENV_V2} exceeds {MAX_S09_COMPILE_ENV_NAMES_V2} names"
        )));
    }
    let mut previous = None;
    let mut policy = BTreeSet::new();
    for name in names {
        if name.len() > 64
            || name.is_empty()
            || !name.bytes().enumerate().all(|(index, byte)| {
                byte == b'_' || byte.is_ascii_uppercase() || (index != 0 && byte.is_ascii_digit())
            })
        {
            return Err(BindingWrapperError::BuildProvenance(format!(
                "{S09_COMPILE_ENV_ALLOWLIST_ENV_V2} contains an invalid name"
            )));
        }
        if previous.is_some_and(|previous| previous >= name) {
            return Err(BindingWrapperError::BuildProvenance(format!(
                "{S09_COMPILE_ENV_ALLOWLIST_ENV_V2} names must be strictly sorted"
            )));
        }
        if forbidden_s09_environment(OsStr::new(name)) {
            return Err(BindingWrapperError::BuildProvenance(format!(
                "{S09_COMPILE_ENV_ALLOWLIST_ENV_V2} cannot permit {name}"
            )));
        }
        policy.insert(OsString::from(name));
        previous = Some(name);
    }
    Ok(policy)
}

fn forbidden_s09_environment(name: &OsStr) -> bool {
    let bytes = os_bytes(name);
    bytes == b"RUSTC_BOOTSTRAP" || bytes.starts_with(b"LD_") || bytes.starts_with(b"DYLD_")
}

fn compilation_control_environment(name: &OsStr) -> bool {
    let bytes = os_bytes(name);
    matches!(
        bytes,
        b"CC" | b"CXX" | b"AR" | b"CFLAGS" | b"CXXFLAGS" | b"LDFLAGS"
    ) || bytes.starts_with(b"LLVM_")
        || bytes.starts_with(b"RUSTC_")
}

fn consumed_before_s09_rustc_environment(name: &OsStr) -> bool {
    matches!(
        os_bytes(name),
        b"RUSTC"
            | b"RUSTFLAGS"
            | b"CARGO_ENCODED_RUSTFLAGS"
            | b"RUSTC_WRAPPER"
            | b"RUSTC_WORKSPACE_WRAPPER"
            | b"CARGO_BUILD_RUSTC_WRAPPER"
    )
}

fn reviewed_s09_inherited_environment(name: &OsStr) -> bool {
    let bytes = os_bytes(name);
    matches!(
        bytes,
        b"PATH"
            | b"HOME"
            | b"TMPDIR"
            | b"LANG"
            | b"LC_ALL"
            | b"TZ"
            | b"TERM"
            | b"SOURCE_DATE_EPOCH"
            | b"CARGO"
            | b"CARGO_BIN_NAME"
            | b"CARGO_CRATE_NAME"
            | b"CARGO_MANIFEST_DIR"
            | b"CARGO_MANIFEST_PATH"
            | b"CARGO_PRIMARY_PACKAGE"
            | b"CARGO_TARGET_TMPDIR"
            | b"OUT_DIR"
            | b"OPT_LEVEL"
            | b"DEBUG"
            | b"PROFILE"
            | b"TARGET"
            | b"HOST"
            | b"NUM_JOBS"
            | b"FE2O3_TARGET"
            | b"FE2O3_CODEGEN_PIPELINE"
            | b"FE2O3_WORKER_V2_CONFIG_V2"
            | b"FE2O3_WORKER_V2_EXPECTED_ID_V1"
            | b"FE2O3_HOST_PASSTHROUGH"
            | b"FE2O3_CAPABILITY_BROKER_V1"
            | b"FE2O3_BUILD_SESSION_V1"
            | b"FE2O3_MANAGED_RUSTC_ARGS_V1"
            | b"FE2O3_S09_COMPILE_ENV_ALLOWLIST_V2"
    ) || bytes.starts_with(b"CARGO_PKG_")
        || bytes.starts_with(b"CARGO_CFG_")
        || bytes.starts_with(b"CARGO_FEATURE_")
        || bytes.starts_with(b"DEP_")
}

fn managed_s09_child_environment(name: &OsStr) -> bool {
    matches!(
        os_bytes(name),
        b"FE2O3_HSACO_DIR"
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
    )
}

fn prepared_rustc_command_sha256(
    command: &Command,
    executable_sha256: [u8; 32],
    environment: &CompleteS09ChildEnvironmentV2,
) -> Result<[u8; 32], BindingWrapperError> {
    let current_dir = command.get_current_dir().ok_or_else(|| {
        BindingWrapperError::BuildProvenance(
            "prepared rustc command has no explicit working directory".to_owned(),
        )
    })?;
    let executable_path = Path::new(command.get_program());
    let arguments_after_argv0 = command.get_args().map(OsString::from).collect::<Vec<_>>();
    prepared_command_digest_v2(&PreparedCommandIdentityV2 {
        executable_path,
        executable_sha256,
        arguments_after_argv0: &arguments_after_argv0,
        current_dir,
        environment: &environment.entries,
    })
    .map_err(|error| {
        BindingWrapperError::BuildProvenance(format!(
            "cannot encode prepared rustc command identity: {error}"
        ))
    })
}

struct PreparedRustcCommandDigestCapability {
    image: File,
    finalized: bool,
}

impl PreparedRustcCommandDigestCapability {
    fn attach(command: &mut Command) -> Result<Self, BindingWrapperError> {
        let display = "S09 prepared-command digest capability";
        // SAFETY: fcntl only probes the process-local fixed descriptor number.
        let target = unsafe { BorrowedFd::borrow_raw(PREPARED_RUSTC_COMMAND_OBSERVATION_FD_V2) };
        match rustix::io::fcntl_getfd(target) {
            Err(rustix::io::Errno::BADF) => {}
            Err(error) => {
                return Err(BindingWrapperError::BuildProvenance(format!(
                    "cannot inspect {display} descriptor: {error}"
                )));
            }
            Ok(_) => {
                return Err(BindingWrapperError::BuildProvenance(format!(
                    "{display} descriptor is already occupied"
                )));
            }
        }
        let image = File::from(
            rustix::fs::memfd_create(
                "fe2o3-s09-prepared-command-v2",
                rustix::fs::MemfdFlags::CLOEXEC | rustix::fs::MemfdFlags::ALLOW_SEALING,
            )
            .map_err(|error| {
                BindingWrapperError::BuildProvenance(format!("cannot create {display}: {error}"))
            })?,
        );
        image.set_len(32).map_err(|error| {
            BindingWrapperError::BuildProvenance(format!("cannot size {display}: {error}"))
        })?;
        let source_fd = image.as_raw_fd();
        let metadata = image.metadata().map_err(|error| {
            BindingWrapperError::BuildProvenance(format!("cannot inspect {display}: {error}"))
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
                let installed = rustix::io::fcntl_dupfd_cloexec(
                    source,
                    PREPARED_RUSTC_COMMAND_OBSERVATION_FD_V2,
                )
                .map_err(std::io::Error::from)?;
                if installed.as_raw_fd() != PREPARED_RUSTC_COMMAND_OBSERVATION_FD_V2 {
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
            return Err(BindingWrapperError::BuildProvenance(
                "S09 prepared-command digest capability was finalized invalidly".to_owned(),
            ));
        }
        self.image.seek(SeekFrom::Start(0)).map_err(|error| {
            BindingWrapperError::BuildProvenance(format!(
                "cannot rewind S09 prepared-command digest capability: {error}"
            ))
        })?;
        self.image.write_all(&digest).map_err(|error| {
            BindingWrapperError::BuildProvenance(format!(
                "cannot write S09 prepared-command digest capability: {error}"
            ))
        })?;
        self.image.seek(SeekFrom::Start(0)).map_err(|error| {
            BindingWrapperError::BuildProvenance(format!(
                "cannot prepare S09 command digest capability for child reading: {error}"
            ))
        })?;
        let data_seals = rustix::fs::SealFlags::WRITE
            | rustix::fs::SealFlags::GROW
            | rustix::fs::SealFlags::SHRINK;
        rustix::fs::fcntl_add_seals(&self.image, data_seals).map_err(|error| {
            BindingWrapperError::BuildProvenance(format!(
                "cannot seal S09 prepared-command digest capability: {error}"
            ))
        })?;
        rustix::fs::fcntl_add_seals(&self.image, rustix::fs::SealFlags::SEAL).map_err(|error| {
            BindingWrapperError::BuildProvenance(format!(
                "cannot finalize S09 prepared-command digest capability seals: {error}"
            ))
        })?;
        let required = data_seals | rustix::fs::SealFlags::SEAL;
        if rustix::fs::fcntl_get_seals(&self.image).map_err(|error| {
            BindingWrapperError::BuildProvenance(format!(
                "cannot verify S09 prepared-command digest capability seals: {error}"
            ))
        })? != required
        {
            return Err(BindingWrapperError::BuildProvenance(
                "S09 prepared-command digest capability seals changed".to_owned(),
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
    let expected_backend = format!("-Zcodegen-backend=/proc/./self/fd/{BACKEND_CHILD_FD}");
    if fields[0] != OsStr::new(&expected_backend) {
        return Err(BindingWrapperError::InvalidManagedRustcArguments(
            "backend selector is not a fixed procfs descriptor",
        ));
    }
    if fields[1] != "-Zmir-enable-passes=-JumpThreading" || fields[2] != "--cfg" {
        return Err(BindingWrapperError::InvalidManagedRustcArguments(
            "managed compiler options changed",
        ));
    }
    let generation = os_bytes(&fields[3]);
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
        let joined = bytes
            .strip_prefix(b"-Z")
            .is_some_and(|value| backend_selector_value(value.strip_prefix(b"=").unwrap_or(value)));
        let split = bytes == b"-Z"
            && argv
                .get(index + 1)
                .is_some_and(|next| backend_selector_value(os_bytes(next)));
        if joined || split {
            return Err(BindingWrapperError::PreexistingCodegenBackend {
                argument_index: index,
            });
        }
    }
    Ok(())
}

fn backend_selector_value(value: &[u8]) -> bool {
    [b"codegen-backend".as_slice(), b"codegen_backend".as_slice()]
        .iter()
        .any(|name| {
            value == *name
                || value
                    .strip_prefix(*name)
                    .is_some_and(|rest| rest.starts_with(b"="))
        })
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
    backend: PinnedCodegenBackend,
    artifact: PinnedDirectory,
    output_dir: PathBuf,
    pinned_cargo_image_sha256: [u8; 32],
}

impl CompilerCapabilities {
    fn from_environment() -> Result<Self, BindingWrapperError> {
        let transferred = capability_broker::receive(managed_build_session()?)
            .map_err(BindingWrapperError::CapabilityBroker)?;
        let backend = PinnedCodegenBackend::from_transferred_file(transferred.backend)
            .map_err(BindingWrapperError::ManagedBackend)?;
        let artifact = PinnedDirectory::from_transferred_file(
            transferred.artifact,
            "artifact output directory",
        )
        .map_err(BindingWrapperError::ManagedArtifact)?;
        let pinned_cargo_image = PinnedExecutable::from_transferred_file(
            transferred.pinned_cargo_image,
            PathBuf::from("<brokered pinned Cargo image observation>"),
        )?;
        let pinned_cargo_image_sha256 = *pinned_cargo_image.sha256();
        drop(pinned_cargo_image);
        let output_dir = artifact.child_path();
        Ok(Self {
            backend,
            artifact,
            output_dir,
            pinned_cargo_image_sha256,
        })
    }

    fn output_dir(&self) -> &Path {
        &self.output_dir
    }

    const fn pinned_cargo_image_sha256(&self) -> [u8; 32] {
        self.pinned_cargo_image_sha256
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
        Ok(())
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
    worker_v2: Option<ManagedWorkerV2>,
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
) -> Result<ManagedAttempt, BindingWrapperError> {
    let session = managed_build_session()?;
    let producer =
        ProducerIdentity::from_codegen(compile.crate_name(), Some(compile.source_path()))
            .map_err(BindingWrapperError::Artifact)?;
    let invocation = derive_build_attempt_input(compile.argv(), worker_v2.as_ref(), current_dir);
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
        worker_v2,
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
    publish_finish_and_clear(managed, resume, persisted.publication, persisted.intent)
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
            "Worker V2 completed resume marker has no exact durable provenance receipt".into(),
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
    let mut digest = Sha256::new();
    digest.update(BUILD_ATTEMPT_INPUT_DOMAIN);
    hash_os(&mut digest, current_dir.as_os_str());
    hash_os(
        &mut digest,
        std::env::var_os(TARGET_ENV).as_deref().unwrap_or_default(),
    );
    hash_os(
        &mut digest,
        std::env::var_os(HSACO_DIR_ENV)
            .as_deref()
            .unwrap_or_default(),
    );
    digest.update((argv.len() as u64).to_le_bytes());
    for argument in argv {
        hash_os(&mut digest, argument);
    }
    if let Some(worker_v2_identity) = worker_v2_identity {
        digest.update(WORKER_V2_CONFIG_ID_DOMAIN);
        digest.update(worker_v2_identity.as_bytes());
    }
    BuildInvocation::from_bytes(digest.finalize().into())
}

fn hash_os(digest: &mut Sha256, value: &OsStr) {
    #[cfg(unix)]
    let bytes = {
        use std::os::unix::ffi::OsStrExt;
        value.as_bytes()
    };
    #[cfg(not(unix))]
    let bytes = value.to_str().unwrap_or_default().as_bytes();
    digest.update((bytes.len() as u64).to_le_bytes());
    digest.update(bytes);
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
        BindingWrapperError, CARGO_FE2O3_EXECUTABLE_BUILD_OBSERVATION_ENV_V2,
        CARGO_METADATA_BUILD_OBSERVATION_ENV_V2, CompileBuildObservationV2,
        CompleteS09ChildEnvironmentV2, DECLARED_CARGO_EXECUTABLE_BUILD_OBSERVATION_ENV_V2,
        LLVM_BUILD_IDENTITY_OBSERVATION_ENV_V2, OBSERVED_PARENT_PID_BUILD_OBSERVATION_ENV_V2,
        OBSERVED_PARENT_START_TIME_BUILD_OBSERVATION_ENV_V2,
        PINNED_CARGO_IMAGE_BUILD_OBSERVATION_ENV_V2, PreparedRustcCommandDigestCapability,
        S09_COMPILE_ENV_ALLOWLIST_ENV_V2, WORKER_BUILD_IDENTITY_OBSERVATION_ENV_V2,
        WORKER_CONFIG_BUILD_OBSERVATION_ENV_V2, WORKER_EXECUTABLE_BUILD_OBSERVATION_ENV_V2,
        configure_build_observation_environment, configure_worker_build_observation_environment,
        decode_managed_rustc_args, derive_build_attempt_input_with_config_identity,
        is_cargo_stdin_probe, materialize_s09_child_environment, measure_build_executable,
        observe_pinned_cargo_image_and_parent, ordered_metadata_values,
        prepared_rustc_command_sha256, process_start_time_ticks, reject_uninspectable_rustc_args,
        resolve_command_executable_with_path,
    };
    use crate::pinned_executable::PinnedExecutable;
    use crate::worker_v2::{WorkerV2BuildObservation, WorkerV2ConfigIdentity};
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
    fn prepared_command_identity_covers_final_args_cwd_executable_and_managed_environment() {
        fn identity(
            arguments: &[String],
            environment: &[(String, Option<String>)],
            current_dir: &Path,
            resolved_program: &Path,
            executable_sha256: [u8; 32],
        ) -> [u8; 32] {
            let mut command = Command::new(resolved_program);
            command.args(arguments).current_dir(current_dir);
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
            let complete_environment = CompleteS09ChildEnvironmentV2::from_command(&command);
            prepared_rustc_command_sha256(&command, executable_sha256, &complete_environment)
                .unwrap()
        }

        let arguments = [
            "--crate-name",
            "unit",
            "unit.rs",
            "-Zcodegen-backend=/proc/./self/fd/198",
            "-Zmir-enable-passes=-JumpThreading",
            "--cfg",
            "fe2o3_codegen_generation=\"0123456789abcdef0123456789abcdef\"",
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
        let current_dir = Path::new("/workspace/a");
        let resolved = Path::new("/toolchain/a/rustc");
        let baseline = identity(&arguments, &environment, current_dir, resolved, [0x31; 32]);

        for index in 0..arguments.len() {
            let mut changed = arguments.clone();
            changed[index].push_str("-changed");
            assert_ne!(
                identity(&changed, &environment, current_dir, resolved, [0x31; 32]),
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
                identity(&arguments, &changed, current_dir, resolved, [0x31; 32]),
                baseline,
                "environment mutation {} was not covered",
                environment[index].0
            );
        }
        assert_ne!(
            identity(
                &arguments,
                &environment,
                Path::new("/workspace/b"),
                resolved,
                [0x31; 32]
            ),
            baseline
        );
        assert_ne!(
            identity(
                &arguments,
                &environment,
                current_dir,
                Path::new("/toolchain/b/rustc"),
                [0x31; 32],
            ),
            baseline
        );
        assert_ne!(
            identity(&arguments, &environment, current_dir, resolved, [0x32; 32]),
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
                .env("FE2O3_BUILD_ATTEMPT_V1", "attempt")
                .env_remove("FE2O3_HSACO_DIR");
            let complete_environment = materialize_s09_child_environment(
                &mut command,
                inherited
                    .iter()
                    .map(|(name, value)| (OsString::from(name), OsString::from(value))),
            )
            .unwrap();
            let digest =
                prepared_rustc_command_sha256(&command, [0x44; 32], &complete_environment).unwrap();
            (command, digest)
        }

        let inherited = [
            ("PATH", "/reviewed/bin"),
            ("CARGO_PKG_NAME", "unit"),
            (S09_COMPILE_ENV_ALLOWLIST_ENV_V2, "CUSTOM_BUILD_INPUT"),
            ("CUSTOM_BUILD_INPUT", "first"),
            ("PRIVATE_TOKEN", "must-not-cross"),
            ("RUSTC_WORKSPACE_WRAPPER", "/already-consumed/wrapper"),
        ];
        let (command, baseline) = prepared(&inherited);
        let effective = command
            .get_envs()
            .filter_map(|(name, value)| value.map(|value| (name, value)))
            .collect::<std::collections::BTreeMap<_, _>>();
        assert_eq!(
            effective.get(OsStr::new("PATH")),
            Some(&OsStr::new("/reviewed/bin"))
        );
        assert_eq!(
            effective.get(OsStr::new("CUSTOM_BUILD_INPUT")),
            Some(&OsStr::new("first"))
        );
        assert!(!effective.contains_key(OsStr::new("PRIVATE_TOKEN")));
        assert!(!effective.contains_key(OsStr::new("RUSTC_WORKSPACE_WRAPPER")));

        let mut inherited_change = inherited;
        inherited_change[1].1 = "other-unit";
        assert_ne!(prepared(&inherited_change).1, baseline);
        let mut policy_value_change = inherited;
        policy_value_change[3].1 = "second";
        assert_ne!(prepared(&policy_value_change).1, baseline);

        let mut removed = Command::new("/toolchain/rustc");
        removed.current_dir("/workspace").env_remove("OPTIONAL");
        let removed_environment = CompleteS09ChildEnvironmentV2::from_command(&removed);
        let removed =
            prepared_rustc_command_sha256(&removed, [0x44; 32], &removed_environment).unwrap();
        let mut empty = Command::new("/toolchain/rustc");
        empty.current_dir("/workspace").env("OPTIONAL", "");
        let empty_environment = CompleteS09ChildEnvironmentV2::from_command(&empty);
        let empty = prepared_rustc_command_sha256(&empty, [0x44; 32], &empty_environment).unwrap();
        assert_ne!(
            removed, empty,
            "removed and empty environments were conflated"
        );
    }

    #[test]
    fn s09_environment_rejects_loader_and_bootstrap_controls() {
        for name in ["LD_PRELOAD", "LD_LIBRARY_PATH", "RUSTC_BOOTSTRAP"] {
            let mut command = Command::new("rustc");
            command.current_dir("/workspace");
            let error = materialize_s09_child_environment(
                &mut command,
                [(OsString::from(name), OsString::from("forbidden"))],
            )
            .unwrap_err();
            assert!(error.to_string().contains(name));
        }
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
        let mut capability = PreparedRustcCommandDigestCapability::attach(&mut command).unwrap();
        let before = format!("{command:?}");
        let complete_environment = CompleteS09ChildEnvironmentV2::from_command(&command);
        let digest =
            prepared_rustc_command_sha256(&command, [0x55; 32], &complete_environment).unwrap();
        capability.finalize(digest).unwrap();
        assert_eq!(format!("{command:?}"), before);
        let mut observed = [0_u8; 32];
        fs::File::open(format!("/proc/self/fd/{}", capability.image.as_raw_fd()))
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
    fn managed_rustc_arguments_are_exact_and_canonical() {
        let value = OsString::from(
            "-Zcodegen-backend=/proc/./self/fd/198\x1f-Zmir-enable-passes=-JumpThreading\x1f--cfg\x1ffe2o3_codegen_generation=\"0123456789abcdef0123456789abcdef\"",
        );
        let decoded = decode_managed_rustc_args(&value).unwrap();
        assert_eq!(decoded.len(), 4);

        for invalid in [
            OsString::from(""),
            OsString::from(
                "-Zcodegen-backend=/tmp/backend\x1f-Zmir-enable-passes=-JumpThreading\x1f--cfg\x1ffe2o3_codegen_generation=\"0123456789abcdef0123456789abcdef\"",
            ),
            OsString::from(
                "-Zcodegen-backend=/proc/./self/fd/198\x1f-Zmir-enable-passes=-JumpThreading\x1f--cfg\x1ffe2o3_codegen_generation=\"ABCDEF0123456789abcdef0123456789\"",
            ),
        ] {
            assert!(decode_managed_rustc_args(&invalid).is_err());
        }
    }

    #[test]
    fn cargo_rustflags_cannot_select_a_backend_or_hide_in_response_files() {
        for argv in [
            args(&["rustc", "unit.rs", "-Zcodegen-backend=/tmp/evil.so"]),
            args(&["rustc", "unit.rs", "-Z", "codegen_backend=/tmp/evil.so"]),
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
