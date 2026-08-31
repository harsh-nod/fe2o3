use std::collections::BTreeMap;
use std::error::Error;
use std::ffi::{OsStr, OsString};
use std::fmt;
use std::os::fd::BorrowedFd;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus};
use std::time::{Duration, Instant};

use fe2o3_artifact_transaction::{
    BuildAttempt, BuildInvocation, BuildSession, EmitError, ProducerIdentity,
    WorkerV3PublicationIntentErrorV1, begin_build_attempt, fail_build_attempt,
    finish_build_attempt, recover_compiler_execution_receipt_transport_v1,
    retire_worker_v3_publication_intent_after_load_readiness_v1,
};
use fe2o3_build_authority::CompilerClosureV2;
use fe2o3_compiler_execution_protocol::CompilerExecutionReceiptCarriageV1;
use fe2o3_hsaco_finalize::{
    PublishedProtectedWorkerV3HsacoV1, RecoveredProtectedWorkerV3HsacoPublicationV1,
    WorkerV3HsacoPublicationErrorV1, finalize_protected_worker_v3_hsaco_v1,
    inspect_protected_worker_v3_hsaco_v1,
    persist_prepared_protected_worker_v3_hsaco_publication_v1,
    prepare_protected_worker_v3_hsaco_publication_v1,
    publish_recovered_protected_worker_v3_hsaco_v1,
    recover_protected_worker_v3_hsaco_publication_v1,
};
use fe2o3_process_identity::PinnedWorkingDirectoryV3;
use fe2o3_runtime_protocol::{
    RecoveredWorkerV3LoadEnvelopeV2, WorkerV3LoadEnvelopeErrorV2, WorkerV3LoadEnvelopeV2,
    recover_worker_v3_load_envelope_v2,
};
use fe2o3_rustc_invocation::{
    CARGO_METADATA_BUILD_OBSERVATION_ENV_V2, CargoMetadataBuildObservationV2, RustcArgsErrorV2,
    RustcCodegenMetadataErrorV1, RustcCompileInvocationV2, RustcInvocationV2,
    classify_rustc_invocation_v2, derive_cargo_metadata_build_observation_v2,
    is_rustc_codegen_backend_selector_v2, is_rustc_option_terminator_v2,
    ordered_rustc_codegen_metadata_v1,
};
use reserved_fe2o3_symbols::{
    CRATE_BINDING_ID_ENV_V1, CrateBindingIdV1, derive_crate_binding_id_v1,
};
use sha2::{Digest, Sha256};

use crate::build_config::{
    BuildCompileEnvironmentProfileV1, BuildConfigError, BuildConfigIdentity,
    PRODUCTION_BUILD_CONFIG_ENV, PRODUCTION_BUILD_CONFIG_V2_ENV, PRODUCTION_BUILD_EXPECTED_ID_ENV,
    PRODUCTION_BUILD_EXPECTED_ID_V2_ENV, PreparedProductionBuildConfig, WORKER_V2_CONFIG_ENV,
    WORKER_V2_EXPECTED_ID_ENV, WORKER_V2_SOURCE_DEBUG_PROFILE_ENV,
    validate_expected_build_config_identity_values,
};
use crate::capability_broker;
use crate::compiler_execution_boundary::{
    ParentCompilerExecutionReadinessCustodyV1, PreparedCompilerExecutionBoundaryV1,
    admit_compiler_execution_receipt_transport, validate_compiler_execution_receipt_carriage,
};
use crate::inert_rustc_invocation_capture::{
    InertPreparedRustcInvocationCapture, InertRustcInvocationCaptureV2,
};
use crate::pinned_codegen_backend::PinnedCodegenBackend;
use crate::pinned_executable::{PinExecutableError, PinnedExecutable};
use crate::project::PinnedDirectory;
use crate::protected_compiler_handoff_v3::{
    ParentRustcInvocationCustody, ProductionCompilerModuleHandoffIntake,
};
use crate::source_isa_observation::{
    finalized_source_isa_observation_frame_v1, ready_source_isa_observation_frame_v1,
};
use crate::{
    ARTIFACT_CHILD_FD, BACKEND_CHILD_FD, MANAGED_RUSTC_ARGS_ENV, RUSTC_CHILD_FD,
    RUSTC_INVOCATION_CHILD_FD, RUSTC_LIBRARY_CHILD_FD,
};

const HSACO_DIR_ENV: &str = "FE2O3_HSACO_DIR";
const TARGET_ENV: &str = "FE2O3_TARGET";
const BUILD_SESSION_ENV: &str = "FE2O3_BUILD_SESSION_V1";
const BUILD_ATTEMPT_ENV: &str = fe2o3_artifact_transaction::BUILD_ATTEMPT_ENV_V1;
const QUALIFICATION_RELEASE_ACTION_ENV: &str = "FE2O3_PROTECTED_RELEASE_ACTION_V1";
const CODEGEN_BACKEND_BUILD_OBSERVATION_ENV_V2: &str = "FE2O3_CODEGEN_BACKEND_BUILD_OBSERVATION_V2";
const QUALIFICATION_CODEGEN_BACKEND_SHA256_ENV_V1: &str =
    "FE2O3_QUALIFICATION_CODEGEN_BACKEND_SHA256_V1";
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
const BUILD_ATTEMPT_INPUT_DOMAIN: &[u8] = b"FE2O3/BUILD-ATTEMPT-INPUT/V2\0";
// Frozen legacy domain bytes remain part of persisted build-attempt identities.
const BUILD_CONFIG_ID_DOMAIN: &[u8] = b"FE2O3/WORKER-V2-CONFIG-ID/V1\0";
const RUSTC_TERMINATION_REAP_TIMEOUT: Duration = Duration::from_secs(2);
const RUSTC_TERMINATION_REAP_POLL_INTERVAL: Duration = Duration::from_millis(10);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CompileBuildObservationV2 {
    crate_binding: CrateBindingIdV1,
    cargo_metadata_digest: CargoMetadataBuildObservationV2,
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
        Ok(Self {
            crate_binding,
            cargo_metadata_digest: derive_cargo_metadata_build_observation_v2(metadata),
        })
    }

    fn cargo_metadata_digest_hex(self) -> String {
        self.cargo_metadata_digest.to_hex()
    }
}

#[derive(Debug)]
pub(crate) enum BindingWrapperError {
    Arguments(RustcArgsErrorV2),
    MissingMetadata {
        crate_name: String,
    },
    CodegenMetadata(RustcCodegenMetadataErrorV1),
    MissingManagedEnvironment(&'static str),
    InvalidBuildSession,
    InvalidManagedRustcArguments(&'static str),
    InvalidCargoPrimaryPackage,
    PinnedExecutable(PinExecutableError),
    CapabilityBroker(String),
    ChildCapability(String),
    UninspectableRustcResponseFile {
        argument_index: usize,
    },
    PreexistingCodegenBackend {
        argument_index: usize,
    },
    OptionTerminatorBeforeManagedArguments {
        argument_index: usize,
    },
    CurrentDirectory(std::io::Error),
    BuildObservation(String),
    BuildConfiguration(BuildConfigError),
    Artifact(EmitError),
    ManagedCompletion {
        primary: String,
        cleanup: Option<EmitError>,
    },
    AttemptTermination {
        rustc_status: ExitStatus,
        cleanup: EmitError,
    },
    CompilerExecutionBoundary {
        stage: &'static str,
        primary: String,
        cleanup: Option<String>,
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
            Self::CodegenMetadata(error) => error.fmt(formatter),
            Self::MissingManagedEnvironment(name) => {
                write!(formatter, "managed rustc invocation is missing {name}")
            }
            Self::InvalidBuildSession => formatter
                .write_str("managed rustc invocation has a noncanonical or reserved build session"),
            Self::InvalidManagedRustcArguments(reason) => {
                write!(formatter, "invalid {MANAGED_RUSTC_ARGS_ENV}: {reason}")
            }
            Self::InvalidCargoPrimaryPackage => write!(
                formatter,
                "Cargo rustc unit has a noncanonical {} marker",
                crate::CARGO_PRIMARY_PACKAGE_ENV,
            ),
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
            Self::BuildConfiguration(error) => {
                write!(formatter, "build configuration setup failed: {error}")
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
            Self::CompilerExecutionBoundary {
                stage,
                primary,
                cleanup,
            } => {
                write!(
                    formatter,
                    "compiler-execution boundary failed during {stage}: {primary}"
                )?;
                if let Some(cleanup) = cleanup {
                    write!(
                        formatter,
                        "; rustc termination/reaping also failed: {cleanup}"
                    )?;
                }
                Ok(())
            }
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
            Self::CodegenMetadata(error) => Some(error),
            Self::Spawn(error) => Some(error),
            Self::CurrentDirectory(error) => Some(error),
            Self::BuildConfiguration(error) => Some(error),
            Self::Artifact(error) => Some(error),
            Self::PinnedExecutable(error) => Some(error),
            Self::ManagedCompletion { cleanup, .. } => cleanup
                .as_ref()
                .map(|error| error as &(dyn Error + 'static)),
            Self::AttemptTermination { cleanup, .. } => Some(cleanup),
            Self::MissingMetadata { .. }
            | Self::MissingManagedEnvironment(_)
            | Self::InvalidBuildSession
            | Self::InvalidManagedRustcArguments(_)
            | Self::InvalidCargoPrimaryPackage
            | Self::CapabilityBroker(_)
            | Self::ChildCapability(_)
            | Self::UninspectableRustcResponseFile { .. }
            | Self::PreexistingCodegenBackend { .. }
            | Self::OptionTerminatorBeforeManagedArguments { .. }
            | Self::CompilerExecutionBoundary { .. }
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

impl From<RustcCodegenMetadataErrorV1> for BindingWrapperError {
    fn from(value: RustcCodegenMetadataErrorV1) -> Self {
        Self::CodegenMetadata(value)
    }
}

impl From<PinExecutableError> for BindingWrapperError {
    fn from(value: PinExecutableError) -> Self {
        Self::PinnedExecutable(value)
    }
}

pub(crate) fn run(mut argv: Vec<OsString>) -> Result<ExitStatus, BindingWrapperError> {
    reject_dynamic_loader_environment()?;
    normalize_unprotected_validation_loader_environment();
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
            let mut managed_rustc_args = managed_rustc_args_from_environment()?;
            let metadata = ordered_rustc_codegen_metadata_v1(compile)?;
            let build_observation =
                CompileBuildObservationV2::from_ordered_metadata(compile.crate_name(), &metadata)?;
            let build_config = PreparedProductionBuildConfig::from_environment()
                .map_err(BindingWrapperError::BuildConfiguration)?;
            validate_expected_build_config_identity(build_config.as_ref())?;
            let source_isa_selection = if build_config
                .as_ref()
                .is_some_and(PreparedProductionBuildConfig::source_isa_summary_enabled)
            {
                let current_dir =
                    std::env::current_dir().map_err(BindingWrapperError::CurrentDirectory)?;
                let selected_kernel_root = selected_kernel_root(
                    build_config.as_ref().map(|config| {
                        config.selects(compile.crate_name(), compile.source_path(), &current_dir)
                    }),
                    std::env::var_os(crate::CARGO_PRIMARY_PACKAGE_ENV).as_deref(),
                )?;
                let source_isa_binding = if selected_kernel_root {
                    build_config.as_ref().and_then(|config| {
                        config
                            .source_isa_unit_identity(
                                compile.crate_name(),
                                compile.source_path(),
                                &current_dir,
                            )
                            .map(|unit| (*config.identity().as_bytes(), *unit.as_bytes()))
                    })
                } else {
                    None
                };
                Some((current_dir, selected_kernel_root, source_isa_binding))
            } else {
                None
            };
            let capability_profile = capability_broker::CapabilityProfileV1::Ordinary;
            let capability_binding =
                capability_broker::CapabilityBindingV3::from_environment_for_client(
                    capability_profile,
                    build_config
                        .as_ref()
                        .map(|config| *config.identity().as_bytes()),
                )
                .map_err(BindingWrapperError::CapabilityBroker)?;
            authenticate_pinned_rustc(&pinned_rustc, capability_binding.rustc_executable_sha256())?;
            validate_rustc_lib_tree_descriptor(capability_binding)?;
            let retain_source_isa_observer =
                source_isa_selection
                    .as_ref()
                    .is_some_and(|(_, selected, binding)| {
                        retain_source_isa_observer(*selected, *binding)
                    });
            let mut compiler_capabilities = if retain_source_isa_observer {
                CompilerCapabilities::from_production_environment_with_source_isa_observer(
                    capability_binding,
                )?
            } else {
                CompilerCapabilities::from_production_environment(capability_binding)?
            };
            let (current_dir, selected_kernel_root, source_isa_binding) = match source_isa_selection
            {
                Some(selection) => selection,
                None => {
                    let current_dir =
                        std::env::current_dir().map_err(BindingWrapperError::CurrentDirectory)?;
                    let selected_kernel_root = selected_kernel_root(
                        build_config.as_ref().map(|config| {
                            config.selects(
                                compile.crate_name(),
                                compile.source_path(),
                                &current_dir,
                            )
                        }),
                        std::env::var_os(crate::CARGO_PRIMARY_PACKAGE_ENV).as_deref(),
                    )?;
                    (current_dir, selected_kernel_root, None)
                }
            };
            let mut managed = if !selected_kernel_root {
                None
            } else {
                let build_config = build_config.ok_or_else(|| {
                    BindingWrapperError::BuildConfiguration(BuildConfigError::Invalid(format!(
                        "selected production kernel root requires {PRODUCTION_BUILD_CONFIG_ENV} or {PRODUCTION_BUILD_CONFIG_V2_ENV}"
                    )))
                })?;
                Some(prepare_production_managed_attempt(
                    compile,
                    build_config,
                    &current_dir,
                    compiler_capabilities.output_dir(),
                    &compiler_capabilities,
                )?)
            };
            let mut release_guard = managed.as_ref().map(ManagedAttemptRevocationGuard::arm);
            let source_isa_observer = match (managed.as_ref(), source_isa_binding) {
                (Some(managed), Some((config, unit))) => compiler_capabilities
                    .release_invocation_with_source_isa_observer(config, unit, managed.attempt)
                    .map(Some),
                _ => Ok(None),
            };
            let source_isa_observer = match source_isa_observer {
                Ok(observer) => observer,
                Err(primary) => {
                    return Err(pre_spawn_failure(release_guard.as_mut(), primary));
                }
            };
            if let Some(guard) = release_guard.as_mut() {
                guard.disarm();
            }
            if let (Some(managed), Some(observer)) = (&mut managed, source_isa_observer) {
                managed.source_isa_observer = Some(SourceIsaObservationEmitterV1 {
                    config: source_isa_binding.expect("V2 observer binding exists").0,
                    unit: source_isa_binding.expect("V2 observer binding exists").1,
                    attempt: managed.attempt,
                    sink: Some(observer),
                });
            }
            scope_managed_rustc_arguments(&mut managed_rustc_args, managed.is_some());
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
    let protected_kernel_root = managed_attempt.is_some();

    if managed_attempt
        .as_ref()
        .is_some_and(ManagedAttempt::is_managed_recovery)
    {
        complete_managed_attempt(
            managed_attempt.expect("managed recovery exists"),
            None,
            None,
        )?;
        return Ok(success_exit_status());
    }

    // From this point until rustc is spawned, every early return must revoke the live attempt.
    // Spawn and completion paths below take over explicit lifecycle handling once disarmed.
    let mut pre_spawn_attempt_guard = managed_attempt
        .as_ref()
        .map(ManagedAttemptRevocationGuard::arm);

    let pre_spawn_result = (|| -> Result<_, BindingWrapperError> {
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
            if managed_attempt.is_none() {
                capabilities.prepare_host_dependency_command(command.as_command_mut());
            } else if protected_kernel_root {
                capabilities.prepare_protected_command(command.as_command_mut())?;
            } else {
                return Err(BindingWrapperError::BuildObservation(
                    "production managed rustc invocation lost protected root admission".to_owned(),
                ));
            }
        }
        configure_build_observation_environment(command.as_command_mut(), build_observation);
        if let Some(managed) = &managed_attempt {
            command
                .as_command_mut()
                .env(BUILD_ATTEMPT_ENV, managed.attempt.to_env_value());
        } else {
            command.as_command_mut().env_remove(BUILD_ATTEMPT_ENV);
        }
        command
            .as_command_mut()
            .env_remove(WORKER_V2_SOURCE_DEBUG_PROFILE_ENV);
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
        clear_worker_build_observation_environment(command.as_command_mut());
        let complete_reviewed_environment = materialize_production_child_environment(
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
                let capture_v2 = InertRustcInvocationCaptureV2::capture(
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
                let protected_compiler_closure = if protected_kernel_root {
                    capabilities.protected_compiler_closure()?
                } else {
                    None
                };
                let capture = InertPreparedRustcInvocationCapture::from_v2_and_protected_closure(
                    capture_v2,
                    protected_compiler_closure,
                )
                .map_err(|error| {
                    BindingWrapperError::BuildObservation(format!(
                        "cannot bind prepared rustc invocation to compiler closure: {error}"
                    ))
                })?;
                debug_assert!(matches!(
                    capture.amd_target(),
                    "gfx942:xnack-" | "gfx950:xnack-"
                ));
                debug_assert_eq!(
                    capture.descriptor_v3().is_some(),
                    protected_compiler_closure.is_some()
                );
                Ok::<_, BindingWrapperError>(capture)
            })
            .transpose()?;
        if let Some(capture) = inert_rustc_invocation.as_ref()
            && std::env::var_os("FE2O3_VERBOSE").as_deref() == Some(OsStr::new("1"))
        {
            let version = capture.descriptor_version();
            let digest = capture.digest_hex();
            eprintln!(
                "[cargo-fe2o3] inert prepared RustcInvocationDescriptorV{version} observation sha256={digest}; no execution or authority claim"
            );
        }
        let rustc_invocation_capability = inert_rustc_invocation
            .as_ref()
            .and_then(InertPreparedRustcInvocationCapture::descriptor_v3)
            .map(|descriptor| {
                let capability =
                    fe2o3_compiler_closure_capability::RustcInvocationCapabilityV1::create(
                        descriptor.clone(),
                    )
                    .map_err(BindingWrapperError::ChildCapability)?;
                capability
                    .inherit_for_child_at(command.as_command_mut(), RUSTC_INVOCATION_CHILD_FD)
                    .map_err(BindingWrapperError::ChildCapability)?;
                Ok::<_, BindingWrapperError>(capability)
            })
            .transpose()?;
        let parent_rustc_invocation_custody = ParentRustcInvocationCustody::retain(
            inert_rustc_invocation,
            rustc_invocation_capability,
        )
        .map_err(|error| BindingWrapperError::ChildCapability(error.to_string()))?;
        let compiler_execution_boundary = if protected_kernel_root {
            let capabilities = compiler_capabilities.as_ref().ok_or_else(|| {
                BindingWrapperError::BuildObservation(
                    "selected protected rustc has no retained compiler capabilities".to_owned(),
                )
            })?;
            Some(
                PreparedCompilerExecutionBoundaryV1::prepare(
                    capabilities.protected_compiler_execution_profile()?,
                    command.as_command_mut(),
                )
                .map_err(|error| {
                    BindingWrapperError::CompilerExecutionBoundary {
                        stage: error.stage(),
                        primary: error.to_string(),
                        cleanup: None,
                    }
                })?,
            )
        } else {
            None
        };
        let mut child = match command.spawn() {
            Ok(child) => child,
            Err(error) => {
                return Ok((Err(error), parent_rustc_invocation_custody, None));
            }
        };
        let compiler_execution_readiness = match compiler_execution_boundary {
            Some(boundary) => match boundary.finish(child.id()) {
                Ok(custody) => Some(custody),
                Err(error) => {
                    let stage = error.stage();
                    let primary = error.to_string();
                    let cleanup = terminate_spawned_rustc(&mut child);
                    return Err(BindingWrapperError::CompilerExecutionBoundary {
                        stage,
                        primary,
                        cleanup,
                    });
                }
            },
            None => None,
        };
        let status = child.wait().map_err(|error| {
            let cleanup = terminate_spawned_rustc(&mut child);
            BindingWrapperError::CompilerExecutionBoundary {
                stage: "rustc wait/reaping",
                primary: error.to_string(),
                cleanup,
            }
        })?;
        Ok((
            Ok(status),
            parent_rustc_invocation_custody,
            compiler_execution_readiness,
        ))
    })();
    let (status, parent_rustc_invocation_custody, compiler_execution_readiness) =
        match pre_spawn_result {
            Ok(prepared) => {
                if let Some(guard) = pre_spawn_attempt_guard.as_mut() {
                    guard.disarm();
                }
                prepared
            }
            Err(primary) => {
                return Err(pre_spawn_failure(pre_spawn_attempt_guard.as_mut(), primary));
            }
        };
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
            complete_managed_attempt(
                managed,
                parent_rustc_invocation_custody,
                compiler_execution_readiness,
            )?;
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

fn terminate_spawned_rustc(child: &mut Child) -> Option<String> {
    let mut failures = Vec::new();
    match child.try_wait() {
        Ok(Some(_)) => return None,
        Ok(None) => {}
        Err(error) => failures.push(format!("initial rustc status check failed: {error}")),
    }

    if let Err(error) = child.kill() {
        failures.push(format!("kill failed: {error}"));
    }
    let deadline = Instant::now()
        .checked_add(RUSTC_TERMINATION_REAP_TIMEOUT)
        .unwrap_or_else(Instant::now);
    loop {
        match child.try_wait() {
            Ok(Some(_)) => {
                return (!failures.is_empty()).then(|| failures.join("; "));
            }
            Ok(None) => {}
            Err(error) => {
                failures.push(format!("status check while reaping failed: {error}"));
                break;
            }
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            break;
        }
        std::thread::sleep(remaining.min(RUSTC_TERMINATION_REAP_POLL_INTERVAL));
    }

    failures.push(format!(
        "rustc pid {} remained unreaped after {:?}",
        child.id(),
        RUSTC_TERMINATION_REAP_TIMEOUT
    ));
    Some(failures.join("; "))
}

fn selected_kernel_root(
    configured_selection: Option<bool>,
    cargo_primary_package: Option<&OsStr>,
) -> Result<bool, BindingWrapperError> {
    if let Some(selected) = configured_selection {
        return Ok(selected);
    }
    match cargo_primary_package {
        None => Ok(false),
        Some(value) if value == OsStr::new("1") => Ok(true),
        Some(_) => Err(BindingWrapperError::InvalidCargoPrimaryPackage),
    }
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
    let authority_sensitive = std::env::var_os("FE2O3_QUALIFICATION_ORACLE_V1").is_some()
        || std::env::var_os(PRODUCTION_BUILD_CONFIG_ENV).is_some()
        || std::env::var_os(PRODUCTION_BUILD_CONFIG_V2_ENV).is_some()
        || std::env::var_os(crate::build_config::WORKER_V2_CONFIG_ENV).is_some();
    let unprotected_validation = cfg!(debug_assertions)
        && std::env::var_os(crate::NON_PRODUCTION_AUTHORITY_VALIDATION_ENV).as_deref()
            == Some(OsStr::new("1"));
    for (name, value) in std::env::vars_os() {
        if crate::is_dynamic_loader_injection_environment_name(&name) {
            // The exact fd path is installed by the admitted parent after retaining the pinned
            // rustc library tree on fd 193. Cargo may add an ambient loader path for ordinary
            // wrappers; it is never admitted for an authority-sensitive invocation.
            if is_managed_rustc_loader_environment(&name, &value)
                || (unprotected_validation
                    && is_cargo_augmented_validation_loader_environment(&name, &value))
                || (name == OsStr::new("LD_LIBRARY_PATH") && !authority_sensitive)
            {
                continue;
            }
            return Err(BindingWrapperError::BuildObservation(format!(
                "binding wrapper rejects dynamic-loader injection variable {name:?}={value:?}"
            )));
        }
    }
    Ok(())
}

fn is_managed_rustc_loader_environment(name: &OsStr, value: &OsStr) -> bool {
    name == OsStr::new("LD_LIBRARY_PATH") && value == OsStr::new("/proc/self/fd/193")
}

fn is_cargo_augmented_validation_loader_environment(name: &OsStr, value: &OsStr) -> bool {
    name == OsStr::new("LD_LIBRARY_PATH")
        && std::env::split_paths(value).last().as_deref() == Some(Path::new("/proc/self/fd/193"))
}

fn normalize_unprotected_validation_loader_environment() {
    if !cfg!(debug_assertions)
        || std::env::var_os(crate::NON_PRODUCTION_AUTHORITY_VALIDATION_ENV).as_deref()
            != Some(OsStr::new("1"))
    {
        return;
    }
    let Some(value) = std::env::var_os("LD_LIBRARY_PATH") else {
        return;
    };
    if is_cargo_augmented_validation_loader_environment(OsStr::new("LD_LIBRARY_PATH"), &value) {
        // This process is an explicitly non-production validation wrapper and has not started
        // worker threads. The mutable Cargo prefix has already affected this exec, so removing it
        // cannot create an authority claim. The pinned-rustc command installs fd 193 again after
        // the inherited environment has been validated.
        unsafe { std::env::remove_var("LD_LIBRARY_PATH") };
    }
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
    binding: capability_broker::CapabilityBindingV3,
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

fn clear_worker_build_observation_environment(command: &mut Command) {
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

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(DIGITS[usize::from(byte >> 4)] as char);
        encoded.push(DIGITS[usize::from(byte & 0x0f)] as char);
    }
    encoded
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

#[derive(Clone, Debug, Eq, PartialEq)]
struct CompleteReviewedChildEnvironment {
    entries: Vec<(OsString, OsString)>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FixedReviewedInheritedInput {
    CargoManifestDir,
    Target,
}

impl FixedReviewedInheritedInput {
    const ALL: [Self; 2] = [Self::CargoManifestDir, Self::Target];

    const fn name(self) -> &'static str {
        match self {
            Self::CargoManifestDir => "CARGO_MANIFEST_DIR",
            Self::Target => "FE2O3_TARGET",
        }
    }

    fn accepts(self, value: &OsStr) -> bool {
        match self {
            Self::CargoManifestDir => {
                let path = Path::new(value);
                path.is_absolute() && os_bytes(value).len() <= 4096
            }
            Self::Target => matches!(value.to_str(), Some("gfx942:xnack-" | "gfx950:xnack-")),
        }
    }
}

fn materialize_production_child_environment(
    profile: Option<BuildCompileEnvironmentProfileV1>,
    command: &mut Command,
    inherited: impl IntoIterator<Item = (OsString, OsString)>,
) -> Result<Option<CompleteReviewedChildEnvironment>, BindingWrapperError> {
    match profile {
        Some(BuildCompileEnvironmentProfileV1::ProductionAmd) => {
            materialize_closed_child_environment(command, inherited, "production").map(Some)
        }
        None => Ok(None),
    }
}

fn materialize_closed_child_environment(
    command: &mut Command,
    inherited: impl IntoIterator<Item = (OsString, OsString)>,
    profile: &str,
) -> Result<CompleteReviewedChildEnvironment, BindingWrapperError> {
    let inherited = inherited.into_iter().collect::<BTreeMap<_, _>>();
    for name in inherited.keys() {
        if rejected_reviewed_inherited_environment(name) {
            return Err(BindingWrapperError::BuildObservation(format!(
                "{profile} child environment rejects inherited variable {name:?}"
            )));
        }
    }

    let mut final_environment = BTreeMap::new();
    for input in FixedReviewedInheritedInput::ALL {
        let name = input.name();
        let value = inherited.get(OsStr::new(name)).ok_or_else(|| {
            BindingWrapperError::BuildObservation(format!(
                "{profile} fixed environment is missing required {name}"
            ))
        })?;
        if !input.accepts(value) {
            return Err(BindingWrapperError::BuildObservation(format!(
                "{profile} fixed environment has invalid {name}"
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
        if !managed_reviewed_child_environment(&name) {
            return Err(BindingWrapperError::BuildObservation(format!(
                "{profile} command has unreviewed explicit environment mutation {name:?}"
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
    Ok(CompleteReviewedChildEnvironment {
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

fn rejected_reviewed_inherited_environment(name: &OsStr) -> bool {
    let bytes = os_bytes(name);
    bytes == b"RUSTC_BOOTSTRAP"
        || bytes == b"FE2O3_CODEGEN_PIPELINE"
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

fn managed_reviewed_child_environment(name: &OsStr) -> bool {
    let name = os_bytes(name);
    name == BUILD_ATTEMPT_ENV.as_bytes()
        || matches!(
            name,
            b"LANG"
                | b"PATH"
                | b"TMPDIR"
                | b"FE2O3_HSACO_DIR"
                | b"FE2O3_CARGO_METADATA_BUILD_OBSERVATION_V2"
                | b"FE2O3_CODEGEN_BACKEND_BUILD_OBSERVATION_V2"
                | b"FE2O3_QUALIFICATION_CODEGEN_BACKEND_SHA256_V1"
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

fn managed_rustc_args_from_environment() -> Result<Vec<OsString>, BindingWrapperError> {
    let value = std::env::var_os(MANAGED_RUSTC_ARGS_ENV).ok_or(
        BindingWrapperError::MissingManagedEnvironment(MANAGED_RUSTC_ARGS_ENV),
    )?;
    decode_managed_rustc_args(&value)
}

fn scope_managed_rustc_arguments(arguments: &mut Vec<OsString>, selected_kernel_root: bool) {
    if !selected_kernel_root {
        arguments.clear();
    }
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

fn validate_expected_build_config_identity(
    config: Option<&PreparedProductionBuildConfig>,
) -> Result<(), BindingWrapperError> {
    if std::env::var_os(WORKER_V2_EXPECTED_ID_ENV).is_some() {
        return Err(BindingWrapperError::BuildConfiguration(
            BuildConfigError::Invalid(format!(
                "{WORKER_V2_EXPECTED_ID_ENV} is unavailable in a production cargo-fe2o3 build"
            )),
        ));
    }
    let v1 = std::env::var_os(PRODUCTION_BUILD_EXPECTED_ID_ENV);
    let v2 = std::env::var_os(PRODUCTION_BUILD_EXPECTED_ID_V2_ENV);
    validate_expected_build_config_identity_values(config, v1.as_deref(), v2.as_deref())
        .map_err(BindingWrapperError::BuildConfiguration)
}

struct CompilerCapabilities {
    binding: capability_broker::CapabilityBindingV3,
    backend: PinnedCodegenBackend,
    artifact: PinnedDirectory,
    compiler_closure: Option<fe2o3_compiler_closure_capability::CompilerClosureCapabilityV1>,
    compiler_execution_profile:
        Option<fe2o3_compiler_closure_capability::CompilerExecutionClientProfileCapabilityV1>,
    invocation_authority: Option<capability_broker::BrokeredInvocationAuthorityV1>,
    output_dir: PathBuf,
}

fn receive_validated_compiler_capabilities(
    binding: capability_broker::CapabilityBindingV3,
) -> Result<capability_broker::BrokeredCapabilities, BindingWrapperError> {
    let transferred = capability_broker::receive(managed_build_session()?, binding)
        .map_err(BindingWrapperError::CapabilityBroker)?;
    if binding.requires_compiler_closure_v2() != transferred.compiler_closure.is_some() {
        return Err(BindingWrapperError::CapabilityBroker(
            "brokered compiler-closure descriptor presence differs from the authenticated binding"
                .to_owned(),
        ));
    }
    if binding.requires_compiler_closure_v2() != transferred.compiler_execution_profile.is_some() {
        return Err(BindingWrapperError::CapabilityBroker(
            "brokered compiler-execution client-profile presence differs from the authenticated binding"
                .to_owned(),
        ));
    }
    if let Some(capability) = &transferred.compiler_closure {
        capability
            .revalidate()
            .map_err(BindingWrapperError::CapabilityBroker)?;
        let closure = capability.closure();
        let closure_mismatch = closure.identity_sha256() != binding.compiler_closure_sha256()
            || closure.rustc_executable_sha256() != binding.rustc_executable_sha256()
            || closure.codegen_backend_sha256() != *transferred.backend.sha256();
        if closure_mismatch {
            return Err(BindingWrapperError::CapabilityBroker(
                "brokered compiler closure differs from the retained compiler capabilities"
                    .to_owned(),
            ));
        }
    }
    if let Some(profile) = &transferred.compiler_execution_profile {
        profile
            .revalidate()
            .map_err(BindingWrapperError::CapabilityBroker)?;
    }
    Ok(transferred)
}

impl CompilerCapabilities {
    fn from_production_environment(
        binding: capability_broker::CapabilityBindingV3,
    ) -> Result<Self, BindingWrapperError> {
        let mut transferred = receive_validated_compiler_capabilities(binding)?;
        let invocation_authority = release_or_retain_invocation_authority(
            transferred.invocation_authority.take(),
            false,
            capability_broker::BrokeredInvocationAuthorityV1::release,
        )?;
        Ok(Self::from_transferred(
            binding,
            transferred,
            invocation_authority,
        ))
    }

    fn from_production_environment_with_source_isa_observer(
        binding: capability_broker::CapabilityBindingV3,
    ) -> Result<Self, BindingWrapperError> {
        let mut transferred = receive_validated_compiler_capabilities(binding)?;
        let invocation_authority = release_or_retain_invocation_authority(
            transferred.invocation_authority.take(),
            true,
            capability_broker::BrokeredInvocationAuthorityV1::release,
        )?;
        Ok(Self::from_transferred(
            binding,
            transferred,
            invocation_authority,
        ))
    }

    fn from_transferred(
        binding: capability_broker::CapabilityBindingV3,
        transferred: capability_broker::BrokeredCapabilities,
        invocation_authority: Option<capability_broker::BrokeredInvocationAuthorityV1>,
    ) -> Self {
        let output_dir = transferred.artifact.child_path();
        Self {
            binding,
            backend: transferred.backend,
            artifact: transferred.artifact,
            compiler_closure: transferred.compiler_closure,
            compiler_execution_profile: transferred.compiler_execution_profile,
            invocation_authority,
            output_dir,
        }
    }

    fn take_invocation_authority(
        &mut self,
    ) -> Result<capability_broker::BrokeredInvocationAuthorityV1, BindingWrapperError> {
        self.invocation_authority.take().ok_or_else(|| {
            BindingWrapperError::CapabilityBroker(
                "capability broker omitted or already consumed invocation authority".to_owned(),
            )
        })
    }

    fn release_invocation_with_source_isa_observer(
        &mut self,
        config: [u8; 32],
        unit: [u8; 32],
        attempt: BuildAttempt,
    ) -> Result<capability_broker::SourceIsaObservationSinkV1, BindingWrapperError> {
        release_selected_source_isa_invocation_authority(
            self.take_invocation_authority()?,
            config,
            unit,
            attempt,
            capability_broker::BrokeredInvocationAuthorityV1::release_with_source_isa_observer,
        )
    }

    fn output_dir(&self) -> &Path {
        &self.output_dir
    }

    fn backend_sha256(&self) -> [u8; 32] {
        *self.backend.sha256()
    }

    fn protected_compiler_closure(
        &self,
    ) -> Result<Option<fe2o3_build_authority::CompilerClosureV2>, BindingWrapperError> {
        let Some(capability) = &self.compiler_closure else {
            return Ok(None);
        };
        capability
            .revalidate()
            .map_err(BindingWrapperError::CapabilityBroker)?;
        let closure = capability.closure();
        if closure.identity_sha256() != self.binding.compiler_closure_sha256()
            || closure.rustc_executable_sha256() != self.binding.rustc_executable_sha256()
            || closure.codegen_backend_sha256() != self.backend_sha256()
        {
            return Err(BindingWrapperError::CapabilityBroker(
                "revalidated compiler closure differs from the retained compiler capabilities"
                    .to_owned(),
            ));
        }
        Ok(Some(closure))
    }

    fn protected_compiler_execution_profile(
        &self,
    ) -> Result<
        &fe2o3_compiler_closure_capability::CompilerExecutionClientProfileCapabilityV1,
        BindingWrapperError,
    > {
        let profile = self.compiler_execution_profile.as_ref().ok_or_else(|| {
            BindingWrapperError::CapabilityBroker(
                "protected compiler binding has no compiler-execution client profile".to_owned(),
            )
        })?;
        profile
            .revalidate()
            .map_err(BindingWrapperError::CapabilityBroker)?;
        Ok(profile)
    }

    const fn compiler_closure_sha256(&self) -> [u8; 32] {
        self.binding.compiler_closure_sha256()
    }

    fn create_reviewed_private_tmpdir(
        &self,
        attempt: BuildAttempt,
    ) -> Result<PathBuf, BindingWrapperError> {
        let component = format!(".fe2o3-rustc-tmp-{}", attempt.to_env_value());
        let mode = rustix::fs::Mode::RUSR | rustix::fs::Mode::WUSR | rustix::fs::Mode::XUSR;
        rustix::fs::mkdirat(self.artifact.file(), &component, mode).map_err(|error| {
            BindingWrapperError::BuildObservation(format!(
                "cannot create private reviewed rustc temporary directory: {error}"
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
                "cannot pin private reviewed rustc temporary directory: {error}"
            ))
        })?;
        rustix::fs::fchmod(&directory, mode).map_err(|error| {
            BindingWrapperError::BuildObservation(format!(
                "cannot make reviewed rustc temporary directory private: {error}"
            ))
        })?;
        Ok(PathBuf::from(format!(
            "/proc/self/fd/{ARTIFACT_CHILD_FD}/{component}"
        )))
    }

    fn prepare_artifact_command(&self, command: &mut Command) -> Result<(), BindingWrapperError> {
        let artifact_path = PathBuf::from(format!("/proc/self/fd/{ARTIFACT_CHILD_FD}"));
        self.prepare_backend_command(command)?;
        self.artifact
            .replace_for_child_at(command, ARTIFACT_CHILD_FD)
            .map_err(BindingWrapperError::ChildCapability)?;
        command.env(HSACO_DIR_ENV, artifact_path);
        Ok(())
    }

    fn prepare_protected_command(&self, command: &mut Command) -> Result<(), BindingWrapperError> {
        self.prepare_artifact_command(command)?;
        command.env_remove(QUALIFICATION_CODEGEN_BACKEND_SHA256_ENV_V1);
        command.env(
            CODEGEN_BACKEND_BUILD_OBSERVATION_ENV_V2,
            hex(&self.backend.sha256()[..]),
        );
        command.env(
            crate::EXPECTED_COMPILER_CLOSURE_SHA256_ENV,
            hex(&self.compiler_closure_sha256()),
        );
        Ok(())
    }

    fn prepare_host_dependency_command(&self, command: &mut Command) {
        scope_host_dependency_environment(command);
    }

    fn prepare_backend_command(&self, command: &mut Command) -> Result<(), BindingWrapperError> {
        self.backend
            .replace_for_child_at(command, BACKEND_CHILD_FD)
            .map_err(|error| BindingWrapperError::ChildCapability(error.to_string()))
    }
}

fn release_or_retain_invocation_authority<Authority>(
    authority: Option<Authority>,
    retain_for_selected_source_isa_observer: bool,
    release: impl FnOnce(Authority) -> Result<(), String>,
) -> Result<Option<Authority>, BindingWrapperError> {
    let authority = authority.ok_or_else(|| {
        BindingWrapperError::CapabilityBroker(
            "capability broker omitted invocation authority".to_owned(),
        )
    })?;
    if retain_for_selected_source_isa_observer {
        return Ok(Some(authority));
    }
    release(authority).map_err(BindingWrapperError::CapabilityBroker)?;
    Ok(None)
}

fn retain_source_isa_observer(
    selected_kernel_root: bool,
    source_isa_binding: Option<([u8; 32], [u8; 32])>,
) -> bool {
    selected_kernel_root && source_isa_binding.is_some()
}

fn release_selected_source_isa_invocation_authority<Authority, Sink>(
    authority: Authority,
    config: [u8; 32],
    unit: [u8; 32],
    attempt: BuildAttempt,
    release: impl FnOnce(Authority, [u8; 32], [u8; 32], BuildAttempt) -> Result<Sink, String>,
) -> Result<Sink, BindingWrapperError> {
    release(authority, config, unit, attempt).map_err(BindingWrapperError::CapabilityBroker)
}

fn scope_host_dependency_environment(command: &mut Command) {
    // Host-only dependencies use rustc's built-in LLVM backend. They are not a
    // device compilation route and receive no fe2o3 backend or artifact custody.
    command.env_remove("FE2O3_QUALIFICATION_ORACLE_V1");
    command.env_remove("FE2O3_CODEGEN_PIPELINE");
    for name in [
        "FE2O3_BACKEND",
        HSACO_DIR_ENV,
        CODEGEN_BACKEND_BUILD_OBSERVATION_ENV_V2,
        QUALIFICATION_CODEGEN_BACKEND_SHA256_ENV_V1,
        crate::EXPECTED_COMPILER_CLOSURE_SHA256_ENV,
        crate::NON_PRODUCTION_AUTHORITY_VALIDATION_ENV,
        PRODUCTION_BUILD_CONFIG_ENV,
        PRODUCTION_BUILD_CONFIG_V2_ENV,
        PRODUCTION_BUILD_EXPECTED_ID_ENV,
        PRODUCTION_BUILD_EXPECTED_ID_V2_ENV,
        WORKER_V2_CONFIG_ENV,
        WORKER_V2_EXPECTED_ID_ENV,
        QUALIFICATION_RELEASE_ACTION_ENV,
    ] {
        command.env_remove(name);
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
    compile_environment_profile: Option<BuildCompileEnvironmentProfileV1>,
    source_isa_observer: Option<SourceIsaObservationEmitterV1>,
    production_build: ManagedProductionBuild,
}

struct ManagedProductionAttempt {
    output_dir: PathBuf,
    producer: ProducerIdentity,
    attempt: BuildAttempt,
    source_isa_observer: Option<SourceIsaObservationEmitterV1>,
}

struct SourceIsaObservationEmitterV1 {
    config: [u8; 32],
    unit: [u8; 32],
    attempt: BuildAttempt,
    sink: Option<capability_broker::SourceIsaObservationSinkV1>,
}

#[derive(Debug, Eq, PartialEq)]
enum SourceIsaObservationEmissionTelemetryV1 {
    AlreadyConsumed,
    AttemptMismatch,
    MappingFailed(String),
    SubmissionFailed(String),
    Submitted,
}

fn emit_source_isa_observation_once<Sink, MappingError, SubmissionError>(
    sink: &mut Option<Sink>,
    expected_attempt: BuildAttempt,
    observed_attempt: BuildAttempt,
    map: impl FnOnce()
        -> Result<crate::source_isa_observation::SourceIsaObservationFrameV1, MappingError>,
    submit: impl FnOnce(
        Sink,
        &crate::source_isa_observation::SourceIsaObservationFrameV1,
    ) -> Result<(), SubmissionError>,
) -> SourceIsaObservationEmissionTelemetryV1
where
    MappingError: fmt::Display,
    SubmissionError: fmt::Display,
{
    let Some(sink) = sink.take() else {
        return SourceIsaObservationEmissionTelemetryV1::AlreadyConsumed;
    };
    if observed_attempt != expected_attempt {
        return SourceIsaObservationEmissionTelemetryV1::AttemptMismatch;
    }
    let frame = match map() {
        Ok(frame) => frame,
        Err(error) => {
            return SourceIsaObservationEmissionTelemetryV1::MappingFailed(error.to_string());
        }
    };
    match submit(sink, &frame) {
        Ok(()) => SourceIsaObservationEmissionTelemetryV1::Submitted,
        Err(error) => SourceIsaObservationEmissionTelemetryV1::SubmissionFailed(error.to_string()),
    }
}

fn report_source_isa_observation_emission(status: SourceIsaObservationEmissionTelemetryV1) {
    match status {
        SourceIsaObservationEmissionTelemetryV1::AlreadyConsumed
        | SourceIsaObservationEmissionTelemetryV1::Submitted => {}
        SourceIsaObservationEmissionTelemetryV1::AttemptMismatch => {
            crate::observer_telemetry::write_line(format_args!(
                "[cargo-fe2o3] source/ISA observer skipped evidence with a different build attempt"
            ));
        }
        SourceIsaObservationEmissionTelemetryV1::MappingFailed(error) => {
            crate::observer_telemetry::write_line(format_args!(
                "[cargo-fe2o3] source/ISA observer mapping failed: {error}"
            ));
        }
        SourceIsaObservationEmissionTelemetryV1::SubmissionFailed(error) => {
            crate::observer_telemetry::write_line(format_args!(
                "[cargo-fe2o3] source/ISA observer submission failed: {error}"
            ));
        }
    }
}

impl SourceIsaObservationEmitterV1 {
    fn emit_finalized(
        &mut self,
        finalized: &fe2o3_hsaco_finalize::PreparedFinalizedProtectedWorkerV3HsacoV1,
    ) {
        report_source_isa_observation_emission(emit_source_isa_observation_once(
            &mut self.sink,
            self.attempt,
            finalized.attempt(),
            || finalized_source_isa_observation_frame_v1(self.config, self.unit, finalized),
            capability_broker::SourceIsaObservationSinkV1::submit,
        ));
    }

    fn emit_ready(&mut self, attempt: BuildAttempt, finalization: [u8; 32]) {
        report_source_isa_observation_emission(emit_source_isa_observation_once(
            &mut self.sink,
            self.attempt,
            attempt,
            || ready_source_isa_observation_frame_v1(self.config, self.unit, attempt, finalization),
            capability_broker::SourceIsaObservationSinkV1::submit,
        ));
    }
}

impl ManagedProductionAttempt {
    fn emit_finalized_source_isa_observation(
        &mut self,
        finalized: &fe2o3_hsaco_finalize::PreparedFinalizedProtectedWorkerV3HsacoV1,
    ) {
        if let Some(observer) = self.source_isa_observer.as_mut() {
            observer.emit_finalized(finalized);
        }
    }

    fn emit_ready_source_isa_observation(&mut self, attempt: BuildAttempt, finalization: [u8; 32]) {
        if let Some(observer) = self.source_isa_observer.as_mut() {
            observer.emit_ready(attempt, finalization);
        }
    }
}

struct ManagedAttemptRevocationGuard {
    output_dir: PathBuf,
    producer: ProducerIdentity,
    attempt: BuildAttempt,
    armed: bool,
}

impl ManagedAttemptRevocationGuard {
    fn arm(managed: &ManagedAttempt) -> Self {
        Self {
            output_dir: managed.output_dir.clone(),
            producer: managed.producer.clone(),
            attempt: managed.attempt,
            armed: true,
        }
    }

    fn disarm(&mut self) {
        self.armed = false;
    }

    fn revoke(&mut self) -> Result<(), EmitError> {
        self.armed = false;
        fail_build_attempt(&self.output_dir, &self.producer, self.attempt)
    }
}

impl Drop for ManagedAttemptRevocationGuard {
    fn drop(&mut self) {
        if self.armed
            && let Err(error) = fail_build_attempt(&self.output_dir, &self.producer, self.attempt)
        {
            crate::observer_telemetry::write_line(format_args!(
                "[cargo-fe2o3] failed to revoke managed build attempt after pre-spawn error: {error}"
            ));
        }
    }
}

fn pre_spawn_failure(
    guard: Option<&mut ManagedAttemptRevocationGuard>,
    primary: BindingWrapperError,
) -> BindingWrapperError {
    let Some(guard) = guard else {
        return primary;
    };
    let cleanup = guard.revoke().err();
    BindingWrapperError::ManagedCompletion {
        primary: primary.to_string(),
        cleanup,
    }
}

fn worker_v3_readiness_is_absent(error: &WorkerV3LoadEnvelopeErrorV2) -> bool {
    matches!(
        error,
        WorkerV3LoadEnvelopeErrorV2::LoadReadiness(
            fe2o3_artifact_transaction::WorkerV3LoadReadinessErrorV1::AttemptState
                | fe2o3_artifact_transaction::WorkerV3LoadReadinessErrorV1::MissingEnvelope
                | fe2o3_artifact_transaction::WorkerV3LoadReadinessErrorV1::MissingClaim
                | fe2o3_artifact_transaction::WorkerV3LoadReadinessErrorV1::MissingReceipt
        )
    )
}

enum ManagedProductionBuild {
    Fresh {
        config: Box<PreparedProductionBuildConfig>,
        compiler_closure: CompilerClosureV2,
    },
    Recovered {
        recovered: Box<RecoveredProtectedWorkerV3HsacoPublicationV1>,
        compiler_closure: CompilerClosureV2,
        compiler_execution: Box<CompilerExecutionReceiptCarriageV1>,
    },
    Ready {
        envelope: Box<RecoveredWorkerV3LoadEnvelopeV2>,
    },
}

enum CompletionFailure {
    Uncommitted(String),
    PreserveAttempt(String),
}

impl ManagedAttempt {
    fn is_managed_recovery(&self) -> bool {
        if matches!(
            &self.production_build,
            ManagedProductionBuild::Recovered { .. } | ManagedProductionBuild::Ready { .. }
        ) {
            return true;
        }
        false
    }

    const fn compile_environment_profile(&self) -> Option<BuildCompileEnvironmentProfileV1> {
        self.compile_environment_profile
    }
}

fn prepare_managed_production_build(
    config: PreparedProductionBuildConfig,
    compiler_closure: CompilerClosureV2,
    output_dir: &Path,
    producer: &ProducerIdentity,
    invocation: BuildInvocation,
    session: BuildSession,
    compiler_execution_profile: &fe2o3_compiler_closure_capability::CompilerExecutionClientProfileCapabilityV1,
) -> Result<(BuildAttempt, ManagedProductionBuild, bool), BindingWrapperError> {
    let attempt = begin_build_attempt(output_dir, producer, invocation, session)
        .map_err(BindingWrapperError::Artifact)?;
    let recovered_envelope = match recover_worker_v3_load_envelope_v2(output_dir, attempt) {
        Ok(envelope) => Some(envelope),
        Err(error) if worker_v3_readiness_is_absent(&error) => None,
        Err(error) => {
            return Err(BindingWrapperError::BuildObservation(format!(
                "production V3 load-readiness recovery failed closed: {error}"
            )));
        }
    };
    if let Some(envelope) = recovered_envelope {
        let subject = envelope
            .wire()
            .reconstructed_compiler_execution_subject_v1()
            .map_err(|error| {
                BindingWrapperError::BuildObservation(format!(
                    "receipt-bearing production load readiness has no exact compiler subject: {error}"
                ))
            })?;
        validate_compiler_execution_receipt_carriage(
            compiler_execution_profile,
            &subject,
            envelope.wire().compiler_execution_receipt(),
        )
        .map_err(|error| {
            BindingWrapperError::BuildObservation(format!(
                "receipt-bearing production load readiness differs from the protected compiler profile: {error}"
            ))
        })?;
        return Ok((
            attempt,
            ManagedProductionBuild::Ready {
                envelope: Box::new(envelope),
            },
            false,
        ));
    }
    match recover_protected_worker_v3_hsaco_publication_v1(output_dir, producer, attempt) {
        Ok(recovered) => {
            let subject = recovered.compiler_execution_subject_v1().map_err(|error| {
                BindingWrapperError::BuildObservation(format!(
                    "recovered production HSACO has no exact compiler-execution subject: {error}"
                ))
            })?;
            let receipt_transport = recover_compiler_execution_receipt_transport_v1(
                output_dir, producer, &subject,
            )
            .map_err(|error| {
                BindingWrapperError::BuildObservation(format!(
                    "recovered production HSACO has no subject-bound compiler receipt: {error}"
                ))
            })?;
            let compiler_execution = admit_compiler_execution_receipt_transport(
                compiler_execution_profile,
                &subject,
                receipt_transport,
            )
            .map_err(|error| {
                BindingWrapperError::BuildObservation(format!(
                    "recovered production compiler receipt differs from the protected profile: {error}"
                ))
            })?;
            Ok((
                attempt,
                ManagedProductionBuild::Recovered {
                    recovered: Box::new(recovered),
                    compiler_closure,
                    compiler_execution: Box::new(compiler_execution),
                },
                false,
            ))
        }
        Err(WorkerV3HsacoPublicationErrorV1::Storage(
            WorkerV3PublicationIntentErrorV1::NotFound,
        )) => Ok((
            attempt,
            ManagedProductionBuild::Fresh {
                config: Box::new(config),
                compiler_closure,
            },
            true,
        )),
        Err(error) => Err(BindingWrapperError::BuildObservation(format!(
            "production V3 restart recovery failed closed: {error}"
        ))),
    }
}

fn prepare_production_managed_attempt(
    compile: RustcCompileInvocationV2<'_>,
    build_config: PreparedProductionBuildConfig,
    current_dir: &Path,
    output_dir: &Path,
    compiler_capabilities: &CompilerCapabilities,
) -> Result<ManagedAttempt, BindingWrapperError> {
    let compile_environment_profile = build_config.compile_environment_profile(
        compile.crate_name(),
        compile.source_path(),
        current_dir,
    );
    let session = managed_build_session()?;
    let producer = ProducerIdentity::from_rustc_compile_invocation_v2(compile)
        .map_err(BindingWrapperError::Artifact)?;
    let invocation = derive_build_attempt_input_with_config_identity(
        compile.argv(),
        Some(build_config.identity()),
        current_dir,
        compiler_capabilities.compiler_closure_sha256(),
    );
    let compiler_closure = compiler_capabilities
        .protected_compiler_closure()?
        .ok_or_else(|| {
            BindingWrapperError::BuildObservation(
                "production requires protected V3 compiler-closure custody before preparation"
                    .to_owned(),
            )
        })?;
    let compiler_execution_profile =
        compiler_capabilities.protected_compiler_execution_profile()?;
    let (attempt, production_build, began_attempt) = prepare_managed_production_build(
        build_config,
        compiler_closure,
        output_dir,
        &producer,
        invocation,
        session,
        compiler_execution_profile,
    )?;
    let mut begin_attempt_guard = began_attempt.then(|| ManagedAttemptRevocationGuard {
        output_dir: output_dir.to_path_buf(),
        producer: producer.clone(),
        attempt,
        armed: true,
    });
    let managed = ManagedAttempt {
        output_dir: output_dir.to_path_buf(),
        producer,
        attempt,
        compile_environment_profile,
        source_isa_observer: None,
        production_build,
    };
    if let Some(guard) = begin_attempt_guard.as_mut() {
        guard.disarm();
    }
    Ok(managed)
}

fn complete_managed_attempt(
    managed: ManagedAttempt,
    parent_rustc_invocation_custody: Option<ParentRustcInvocationCustody>,
    compiler_execution_readiness: Option<ParentCompilerExecutionReadinessCustodyV1>,
) -> Result<(), BindingWrapperError> {
    let mut revocation = ManagedAttemptRevocationGuard::arm(&managed);
    let completion = match (
        parent_rustc_invocation_custody,
        compiler_execution_readiness,
    ) {
        (Some(invocation), Some(readiness)) => invocation.retain_through(|invocation| {
            readiness.retain_through(|readiness| {
                invocation.revalidate().map_err(|error| {
                    CompletionFailure::Uncommitted(format!(
                        "parent protected rustc invocation custody failed before managed completion: {error}"
                    ))
                })?;
                readiness.revalidate().map_err(|error| {
                    CompletionFailure::Uncommitted(format!(
                        "parent compiler-execution readiness custody failed before managed completion: {error}"
                    ))
                })?;
                debug_assert!(!invocation.grants_compiler_authority());
                debug_assert!(!readiness.grants_compiler_authority());
                complete_managed_attempt_inner(
                    managed,
                    Some(invocation),
                    Some(readiness),
                )
            })
        }),
        (Some(invocation), None) => invocation.retain_through(|invocation| {
            invocation.revalidate().map_err(|error| {
                CompletionFailure::Uncommitted(format!(
                    "parent protected rustc invocation custody failed before managed completion: {error}"
                ))
            })?;
            debug_assert!(!invocation.grants_compiler_authority());
            complete_managed_attempt_inner(managed, Some(invocation), None)
        }),
        (None, Some(readiness)) => readiness.retain_through(|readiness| {
            readiness.revalidate().map_err(|error| {
                CompletionFailure::Uncommitted(format!(
                    "parent compiler-execution readiness custody failed before managed completion: {error}"
                ))
            })?;
            debug_assert!(!readiness.grants_compiler_authority());
            complete_managed_attempt_inner(managed, None, Some(readiness))
        }),
        (None, None) => complete_managed_attempt_inner(managed, None, None),
    };

    match completion {
        Ok(()) => {
            revocation.disarm();
            Ok(())
        }
        Err(CompletionFailure::Uncommitted(primary)) => {
            let cleanup = revocation.revoke().err();
            Err(BindingWrapperError::ManagedCompletion { primary, cleanup })
        }
        Err(CompletionFailure::PreserveAttempt(primary)) => {
            revocation.disarm();
            Err(BindingWrapperError::ManagedCompletion {
                primary,
                cleanup: None,
            })
        }
    }
}

fn complete_managed_attempt_inner(
    managed: ManagedAttempt,
    parent_invocation: Option<&ParentRustcInvocationCustody>,
    execution_readiness: Option<&ParentCompilerExecutionReadinessCustodyV1>,
) -> Result<(), CompletionFailure> {
    let ManagedAttempt {
        output_dir,
        producer,
        attempt,
        source_isa_observer,
        production_build,
        ..
    } = managed;
    let mut transaction = ManagedProductionAttempt {
        output_dir,
        producer,
        attempt,
        source_isa_observer,
    };
    complete_managed_production_build(
        &mut transaction,
        production_build,
        parent_invocation,
        execution_readiness,
    )
}

fn complete_managed_production_build(
    managed: &mut ManagedProductionAttempt,
    build: ManagedProductionBuild,
    parent_invocation: Option<&ParentRustcInvocationCustody>,
    execution_readiness: Option<&ParentCompilerExecutionReadinessCustodyV1>,
) -> Result<(), CompletionFailure> {
    match build {
        ManagedProductionBuild::Fresh {
            config,
            compiler_closure,
        } => complete_fresh_production_artifact(
            managed,
            &config,
            compiler_closure,
            parent_invocation.ok_or_else(|| {
                CompletionFailure::Uncommitted(
                    "production V3 completion lost exact parent rustc invocation custody"
                        .to_owned(),
                )
            })?,
            execution_readiness.ok_or_else(|| {
                CompletionFailure::Uncommitted(
                    "production V3 completion lost exact compiler-execution readiness custody"
                        .to_owned(),
                )
            })?,
        ),
        ManagedProductionBuild::Recovered {
            recovered,
            compiler_closure,
            compiler_execution,
        } if parent_invocation.is_none() && execution_readiness.is_none() => {
            complete_recovered_production_artifact(
                managed,
                *recovered,
                compiler_closure,
                *compiler_execution,
            )
        }
        ManagedProductionBuild::Ready { envelope }
            if parent_invocation.is_none() && execution_readiness.is_none() =>
        {
            complete_ready_production_artifact(managed, *envelope)
        }
        ManagedProductionBuild::Recovered { .. } | ManagedProductionBuild::Ready { .. } => {
            Err(CompletionFailure::Uncommitted(
                "recovered production completion unexpectedly retained fresh rustc custody"
                    .to_owned(),
            ))
        }
    }
}

fn complete_fresh_production_artifact(
    managed: &mut ManagedProductionAttempt,
    worker: &PreparedProductionBuildConfig,
    compiler_closure: CompilerClosureV2,
    parent_invocation: &ParentRustcInvocationCustody,
    execution_readiness: &ParentCompilerExecutionReadinessCustodyV1,
) -> Result<(), CompletionFailure> {
    parent_invocation.revalidate().map_err(|error| {
        CompletionFailure::Uncommitted(format!(
            "exact parent rustc invocation custody changed before fresh publication: {error}"
        ))
    })?;
    execution_readiness.revalidate().map_err(|error| {
        CompletionFailure::Uncommitted(format!(
            "compiler-execution readiness changed before fresh publication: {error}"
        ))
    })?;
    debug_assert_ne!(execution_readiness.profile_identity().as_bytes(), &[0; 32]);
    let intake = ProductionCompilerModuleHandoffIntake::new();
    let (consumed, preflight) = intake
        .consume_after_preflight(
            &managed.output_dir,
            &managed.producer,
            managed.attempt,
            parent_invocation,
            execution_readiness,
            |handoff, receipt, observed_closure| {
                if observed_closure != compiler_closure {
                    return Err(
                        fe2o3_hsaco_finalize::ProtectedFirstBuildWorkerV3Error::ReplayValidation {
                            field: "Cargo compiler closure changed before V3 worker preflight",
                        },
                    );
                }
                worker.preflight_production(handoff, receipt, observed_closure)
            },
        )
        .map_err(|error| {
            CompletionFailure::Uncommitted(format!(
                "strict V3 compiler-module preflight/consumption failed: {error}"
            ))
        })?;
    let (evidence, compiler_execution) = worker
        .execute_preflighted_production(consumed, preflight)
        .map_err(|error| {
            CompletionFailure::Uncommitted(format!(
                "strict V3 reproducible worker execution failed: {error}"
            ))
        })?;
    let inspected = inspect_protected_worker_v3_hsaco_v1(evidence).map_err(|error| {
        CompletionFailure::Uncommitted(format!(
            "independent strict V3 raw-HSACO inspection failed: {error}"
        ))
    })?;
    let finalized = finalize_protected_worker_v3_hsaco_v1(inspected).map_err(|error| {
        CompletionFailure::Uncommitted(format!(
            "strict V3 canonical HSACO finalization failed: {error}"
        ))
    })?;
    managed.emit_finalized_source_isa_observation(&finalized);
    let prepared = prepare_protected_worker_v3_hsaco_publication_v1(&managed.producer, finalized)
        .map_err(|error| {
        CompletionFailure::Uncommitted(format!(
            "strict V3 durable publication preparation failed: {error}"
        ))
    })?;
    let recovered = persist_prepared_protected_worker_v3_hsaco_publication_v1(
        &managed.output_dir,
        &managed.producer,
        prepared,
    )
    .map_err(|error| {
        CompletionFailure::PreserveAttempt(format!(
            "strict V3 durable publication persistence failed: {error}"
        ))
    })?;
    complete_recovered_production_artifact(managed, recovered, compiler_closure, compiler_execution)
}

fn complete_recovered_production_artifact(
    managed: &mut ManagedProductionAttempt,
    recovered: RecoveredProtectedWorkerV3HsacoPublicationV1,
    compiler_closure: CompilerClosureV2,
    compiler_execution: CompilerExecutionReceiptCarriageV1,
) -> Result<(), CompletionFailure> {
    managed.emit_finalized_source_isa_observation(recovered.finalized_evidence());
    let published = publish_recovered_protected_worker_v3_hsaco_v1(
        &managed.output_dir,
        &managed.producer,
        compiler_closure,
        recovered,
    )
    .map_err(|error| {
        CompletionFailure::PreserveAttempt(format!(
            "strict V3 finalized-HSACO publication failed: {error}"
        ))
    })?;
    complete_published_production_artifact(managed, published, compiler_execution)
}

fn complete_published_production_artifact(
    managed: &ManagedProductionAttempt,
    published: PublishedProtectedWorkerV3HsacoV1,
    compiler_execution: CompilerExecutionReceiptCarriageV1,
) -> Result<(), CompletionFailure> {
    let intent_identity = published.recovered_evidence().storage_record().identity();
    let envelope = WorkerV3LoadEnvelopeV2::from_published_hsaco_v1(published, compiler_execution)
        .map_err(|error| {
        CompletionFailure::PreserveAttempt(format!(
            "receipt-bearing strict V3 load-envelope custody construction failed: {error}"
        ))
    })?;
    let readiness = envelope
        .persist_durable_replay_custody_v2(&managed.output_dir)
        .map_err(|error| {
            CompletionFailure::PreserveAttempt(format!(
                "receipt-bearing strict V3 load-envelope custody persistence failed: {error}"
            ))
        })?;
    retire_worker_v3_publication_intent_after_load_readiness_v1(
        &managed.output_dir,
        &managed.producer,
        managed.attempt,
        intent_identity,
        readiness.receipt(),
    )
    .map_err(|error| {
        CompletionFailure::PreserveAttempt(format!(
            "strict V3 publication-intent retirement failed: {error}"
        ))
    })?;
    drop(envelope);
    finish_build_attempt(&managed.output_dir, &managed.producer, managed.attempt).map_err(|error| {
        CompletionFailure::PreserveAttempt(format!(
            "strict V3 build-attempt completion failed: {error}"
        ))
    })
}

fn complete_ready_production_artifact(
    managed: &mut ManagedProductionAttempt,
    envelope: RecoveredWorkerV3LoadEnvelopeV2,
) -> Result<(), CompletionFailure> {
    let record = envelope.wire().replay().publication_intent_record();
    let intent_identity = record.identity();
    managed.emit_ready_source_isa_observation(
        record.attempt(),
        *record.plan().finalization().as_bytes(),
    );
    match retire_worker_v3_publication_intent_after_load_readiness_v1(
        &managed.output_dir,
        &managed.producer,
        managed.attempt,
        intent_identity,
        envelope.receipt(),
    ) {
        Ok(()) | Err(WorkerV3PublicationIntentErrorV1::NotFound) => {}
        Err(error) => {
            return Err(CompletionFailure::PreserveAttempt(format!(
                "recovered strict V3 publication-intent retirement failed: {error}"
            )));
        }
    }
    drop(envelope);
    finish_build_attempt(&managed.output_dir, &managed.producer, managed.attempt).map_err(|error| {
        CompletionFailure::PreserveAttempt(format!(
            "recovered strict V3 build-attempt completion failed: {error}"
        ))
    })
}

#[cfg(unix)]
fn success_exit_status() -> ExitStatus {
    use std::os::unix::process::ExitStatusExt;
    ExitStatus::from_raw(0)
}

fn derive_build_attempt_input_with_config_identity(
    argv: &[OsString],
    config_identity: Option<BuildConfigIdentity>,
    current_dir: &std::path::Path,
    compiler_closure_sha256: [u8; 32],
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
    if let Some(config_identity) = config_identity {
        digest.update(BUILD_CONFIG_ID_DOMAIN);
        digest.update(config_identity.as_bytes());
    }
    BuildInvocation::from_bytes(digest.finalize().into())
        .bind_compiler_closure_v1(compiler_closure_sha256)
}

fn hash_bytes(digest: &mut Sha256, bytes: &[u8]) {
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

pub(crate) fn exit_code(status: ExitStatus) -> u8 {
    status
        .code()
        .and_then(|code| u8::try_from(code).ok())
        .unwrap_or(1)
}

#[cfg(test)]
mod lifecycle_tests {
    use super::*;
    use std::cell::Cell;
    use std::fs;
    use std::io;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_TEST_DIRECTORY: AtomicU64 = AtomicU64::new(0);

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            fe2o3_artifact_transaction::enable_same_mount_namespace_artifact_path_guard_v1();
            loop {
                let id = NEXT_TEST_DIRECTORY.fetch_add(1, Ordering::Relaxed);
                let path = std::env::temp_dir().join(format!(
                    "cargo-fe2o3-observer-lifecycle-{}-{id}",
                    std::process::id()
                ));
                match fs::create_dir(&path) {
                    Ok(()) => return Self(path),
                    Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
                    Err(error) => panic!("failed to create observer lifecycle fixture: {error}"),
                }
            }
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn attempt(discriminator: u8) -> BuildAttempt {
        let mut session = [0; 16];
        session[15] = discriminator;
        BuildAttempt::from_env_value(&format!(
            "7:{}:{}",
            BuildSession::from_bytes(session),
            BuildInvocation::from_bytes([discriminator; 32])
        ))
        .unwrap()
    }

    #[test]
    fn spawned_rustc_cleanup_kills_and_reaps_the_child() {
        let mut child = Command::new("/bin/sleep").arg("60").spawn().unwrap();
        let started = Instant::now();
        assert!(terminate_spawned_rustc(&mut child).is_none());
        assert!(started.elapsed() <= RUSTC_TERMINATION_REAP_TIMEOUT + Duration::from_secs(1));
        assert!(child.try_wait().unwrap().is_some());
    }

    #[test]
    fn release_lifecycle_preserves_v1_and_nonselected_v2_ordering() {
        let released = Cell::new(0);
        let ordinary = release_or_retain_invocation_authority(Some(7_u8), false, |authority| {
            assert_eq!(authority, 7);
            released.set(released.get() + 1);
            Ok(())
        })
        .unwrap();
        assert!(ordinary.is_none());
        assert_eq!(released.get(), 1);

        let selection_after_release = || -> Result<(), BindingWrapperError> {
            assert_eq!(released.get(), 1);
            Err(BindingWrapperError::CurrentDirectory(io::Error::new(
                io::ErrorKind::NotFound,
                "injected cwd failure",
            )))
        };
        assert!(selection_after_release().is_err());

        assert!(!retain_source_isa_observer(
            false,
            Some(([0x11; 32], [0x12; 32]))
        ));
        let nonselected = release_or_retain_invocation_authority(Some(8_u8), false, |authority| {
            assert_eq!(authority, 8);
            released.set(released.get() + 1);
            Ok(())
        })
        .unwrap();
        assert!(nonselected.is_none());
        assert_eq!(released.get(), 2);
    }

    #[test]
    fn selected_v2_release_is_deferred_and_exact_attempt_bound() {
        let expected_attempt = attempt(3);
        let config = [0x21; 32];
        let unit = [0x22; 32];
        assert!(retain_source_isa_observer(true, Some((config, unit))));
        let retained = release_or_retain_invocation_authority(
            Some(9_u8),
            true,
            |_authority| -> Result<(), String> { panic!("selected V2 authority released early") },
        )
        .unwrap()
        .unwrap();
        let sink = release_selected_source_isa_invocation_authority(
            retained,
            config,
            unit,
            expected_attempt,
            |authority, actual_config, actual_unit, actual_attempt| {
                assert_eq!(authority, 9);
                assert_eq!(actual_config, config);
                assert_eq!(actual_unit, unit);
                assert_eq!(actual_attempt, expected_attempt);
                Ok::<_, String>("acknowledged sink")
            },
        )
        .unwrap();
        assert_eq!(sink, "acknowledged sink");
    }

    #[test]
    fn selected_v2_release_failure_revokes_the_real_managed_attempt() {
        let temp = TestDirectory::new();
        let output = temp.0.join("output");
        let producer = ProducerIdentity::from_codegen(
            "observer_release_failure",
            Some(Path::new("/workspace/src/lib.rs")),
        )
        .unwrap();
        let session = BuildSession::from_bytes([0x31; 16]);
        let invocation = BuildInvocation::from_bytes([0x32; 32]);
        let attempt = begin_build_attempt(&output, &producer, invocation, session).unwrap();
        let mut guard = ManagedAttemptRevocationGuard {
            output_dir: output.clone(),
            producer: producer.clone(),
            attempt,
            armed: true,
        };
        let release_failure = release_selected_source_isa_invocation_authority(
            (),
            [0x41; 32],
            [0x42; 32],
            attempt,
            |_authority, _config, _unit, _attempt| -> Result<(), String> {
                Err("injected V2 ACK failure".to_owned())
            },
        )
        .unwrap_err();
        let failure = pre_spawn_failure(Some(&mut guard), release_failure);
        assert!(matches!(
            failure,
            BindingWrapperError::ManagedCompletion { .. }
        ));
        assert!(!guard.armed);
        assert!(begin_build_attempt(&output, &producer, invocation, session).is_err());
        assert!(fs::read_dir(&output).unwrap().all(|entry| {
            entry
                .unwrap()
                .path()
                .extension()
                .is_none_or(|ext| ext != "hsaco")
        }));
    }

    #[test]
    fn observation_emission_is_one_shot_for_repeated_lifecycle_entries() {
        let expected_attempt = attempt(4);
        let map_calls = Cell::new(0);
        let submit_calls = Cell::new(0);
        let mut sink = Some(17_u8);
        let first = emit_source_isa_observation_once(
            &mut sink,
            expected_attempt,
            expected_attempt,
            || {
                map_calls.set(map_calls.get() + 1);
                ready_source_isa_observation_frame_v1(
                    [0x41; 32],
                    [0x42; 32],
                    expected_attempt,
                    [0x43; 32],
                )
            },
            |actual_sink, frame| {
                assert_eq!(actual_sink, 17);
                assert_eq!(frame.context().attempt(), expected_attempt);
                assert_eq!(
                    frame.outcome(),
                    crate::source_isa_observation::SourceIsaObservationOutcomeV1::Unavailable(
                        crate::source_isa_observation::SourceIsaObservationUnavailableReasonV1::FinalizedEvidenceUnavailableFromReadyState
                    )
                );
                submit_calls.set(submit_calls.get() + 1);
                Ok::<(), String>(())
            },
        );
        assert_eq!(first, SourceIsaObservationEmissionTelemetryV1::Submitted);
        let second = emit_source_isa_observation_once(
            &mut sink,
            expected_attempt,
            expected_attempt,
            || {
                map_calls.set(map_calls.get() + 1);
                ready_source_isa_observation_frame_v1(
                    [0x41; 32],
                    [0x42; 32],
                    expected_attempt,
                    [0x43; 32],
                )
            },
            |_sink, _frame| {
                submit_calls.set(submit_calls.get() + 1);
                Ok::<(), String>(())
            },
        );
        assert_eq!(
            second,
            SourceIsaObservationEmissionTelemetryV1::AlreadyConsumed
        );
        assert_eq!(map_calls.get(), 1);
        assert_eq!(submit_calls.get(), 1);
    }

    #[test]
    fn observation_emission_failures_are_nonfatal_and_still_consume_the_sink() {
        let expected_attempt = attempt(5);

        let mut mismatch_sink = Some(());
        assert_eq!(
            emit_source_isa_observation_once(
                &mut mismatch_sink,
                expected_attempt,
                attempt(6),
                || -> Result<_, String> { panic!("mismatched attempt must not map") },
                |_sink, _frame| -> Result<(), String> {
                    panic!("mismatched attempt must not submit")
                },
            ),
            SourceIsaObservationEmissionTelemetryV1::AttemptMismatch
        );
        assert!(mismatch_sink.is_none());

        let mut mapping_sink = Some(());
        assert_eq!(
            emit_source_isa_observation_once(
                &mut mapping_sink,
                expected_attempt,
                expected_attempt,
                || Err::<crate::source_isa_observation::SourceIsaObservationFrameV1, _>("map"),
                |_sink, _frame| Ok::<(), String>(()),
            ),
            SourceIsaObservationEmissionTelemetryV1::MappingFailed("map".to_owned())
        );
        assert!(mapping_sink.is_none());

        let mut submission_sink = Some(());
        assert_eq!(
            emit_source_isa_observation_once(
                &mut submission_sink,
                expected_attempt,
                expected_attempt,
                || {
                    ready_source_isa_observation_frame_v1(
                        [0x51; 32],
                        [0x52; 32],
                        expected_attempt,
                        [0x53; 32],
                    )
                },
                |_sink, _frame| Err::<(), _>("submit"),
            ),
            SourceIsaObservationEmissionTelemetryV1::SubmissionFailed("submit".to_owned())
        );
        assert!(submission_sink.is_none());
    }
}
