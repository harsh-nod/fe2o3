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

use crate::production_release::{
    AdmittedRowSoftmaxV1WorkloadV1, admit_row_softmax_v1_source_tested_artifact_v1,
    execute_row_softmax_v1_production_workload_v1, preflight_row_softmax_v1_workload_v1,
};
use fe2o3_artifact_transaction::{
    AttemptScopedHsacoPublicationErrorV1, AttemptScopedHsacoPublicationErrorV2,
    BackendPublicationReceiptV1, BackendPublicationReceiptV2, BrokeredInvocationCapabilityClaimV1,
    BuildAttempt, BuildInvocation, BuildSession, DurablePublishedHsacoClaimV2, EmitError,
    PersistedBackendReceiptV1, PersistedBackendReceiptV2, ProducerIdentity,
    RecoveredWorkerV2PublicationIntentV1, RecoveredWorkerV2PublicationIntentV2,
    WorkerV2PublicationIntentErrorV1, WorkerV2PublicationIntentErrorV2,
    WorkerV2PublicationIntentRecordV2, WorkerV3PublicationIntentErrorV1, begin_build_attempt,
    clear_worker_v2_publication_intent_v1, clear_worker_v2_publication_intent_v2,
    complete_simulation_kernel_ir_attempt_v1, consume_compiler_module_handoff_v1,
    consume_simulation_kernel_ir_handoff_v1, fail_build_attempt, finish_build_attempt,
    publish_exact_hsaco_evidence_for_attempt_v1, publish_exact_hsaco_evidence_for_attempt_v2,
    read_backend_publication_receipt_v1, read_backend_publication_receipt_v2,
    recover_published_hsaco_claim_for_attempt_v1, recover_published_hsaco_claim_for_attempt_v2,
    retire_worker_v3_publication_intent_after_load_readiness_v1,
};
use fe2o3_build_authority::CompilerClosureV2;
use fe2o3_compiler_ffi::{CompilerModuleHandoffV2, decode_row_softmax_compiler_sections_v1};
use fe2o3_hsaco_finalize::{
    CanonicalDescriptorSectionObservationV1, PublishedProtectedWorkerV3HsacoV1,
    ROW_SOFTMAX_V1_PROVIDER_ITEM_COUNT, RecoveredProtectedWorkerV3HsacoPublicationV1,
    RowSoftmaxV1AuthorityPolicyV1, RowSoftmaxV1CompilerClosurePolicyV1,
    RowSoftmaxV1DirectWorkerExpectationV1, RowSoftmaxV1ProviderManifestV1,
    WorkerV3HsacoPublicationErrorV1, derive_row_softmax_v1_provider_source_identity_v1,
    finalize_inspected_protected_worker_v2_hsaco_v2,
    finalize_inspected_protected_worker_v3_hsaco_v1, inspect_production_v1_worker_v2_raw_hsaco_v1,
    inspect_protected_production_v1_worker_v2_raw_hsaco_v1,
    inspect_protected_production_v1_worker_v3_raw_hsaco_v1,
    inspect_protected_worker_v2_raw_hsaco_v1, inspect_worker_v2_raw_hsaco_v1,
    persist_prepared_protected_worker_v3_hsaco_publication_v1,
    prepare_finalized_protected_worker_v2_hsaco_publication_v2,
    prepare_protected_worker_v2_hsaco_publication_v2,
    prepare_protected_worker_v3_hsaco_publication_v1,
    publish_recovered_protected_worker_v3_hsaco_v1,
    recover_protected_worker_v3_hsaco_publication_v1,
};
use fe2o3_kernel_descriptor::CodeObjectVersion;
use fe2o3_process_identity::{
    LinuxObjectIdentityV3, ParentPreparedProcessConsistencyV3, PinnedWorkingDirectoryV3,
    parent_prepared_process_consistency_digest_v3,
};
use fe2o3_rustc_invocation::{
    CARGO_METADATA_BUILD_OBSERVATION_ENV_V2, CargoMetadataBuildObservationV2, RustcArgsErrorV2,
    RustcCodegenMetadataErrorV1, RustcCompileInvocationV2, RustcInvocationV2,
    classify_rustc_invocation_v2, derive_cargo_metadata_build_observation_v2,
    is_rustc_codegen_backend_selector_v2, is_rustc_option_terminator_v2,
    ordered_rustc_codegen_metadata_v1,
};
use fe2o3_worker_v2_bundle::{
    RecoveredWorkerV3LoadEnvelopeV1, WorkerV2EnvelopeInputsV1, WorkerV2ProducerBindingV2,
    WorkerV3LoadEnvelopeErrorV1, WorkerV3LoadEnvelopeV1, recover_worker_v3_load_envelope_v1,
};
use reserved_fe2o3_symbols::{
    CRATE_BINDING_ID_ENV_V1, CrateBindingIdV1, derive_crate_binding_id_v1,
};
use sha2::{Digest, Sha256};

use crate::capability_broker;
use crate::inert_rustc_invocation_capture::{
    InertPreparedRustcInvocationCapture, InertRustcInvocationCaptureV2,
};
use crate::pinned_codegen_backend::PinnedCodegenBackend;
use crate::pinned_executable::{PinExecutableError, PinnedExecutable};
use crate::project::PinnedDirectory;
use crate::protected_compiler_handoff_v3::{
    ParentRustcInvocationCustody, ProtectedCompilerModuleHandoffIntake,
};
use crate::worker_v2::{
    GENERAL_GEMM_RUNTIME_CLOSURE_V2_MANIFEST_SHA256_ENV, GENERAL_GEMM_RUNTIME_CLOSURE_V2_ROOT_ENV,
    OBSOLETE_PRODUCTION_SELECTOR, PreparedWorkerV2Config, WORKER_V2_CONFIG_ENV,
    WORKER_V2_EXPECTED_ID_ENV, WORKER_V2_SOURCE_DEBUG_PROFILE_ENV, WorkerV2BuildObservation,
    WorkerV2CompileEnvironmentProfileV1, WorkerV2ConfigError, WorkerV2ConfigIdentity,
    WorkerV2SourceDebugProfileV1, production_compilation_selected,
};
use crate::worker_v2_artifact_container::{
    assemble_recovered_worker_v2_load_envelope_v1, assemble_recovered_worker_v2_load_envelope_v2,
    derive_required_worker_v2_publication_plan_v1,
};
#[cfg(feature = "worker-v2-fault-injection-test-only")]
use crate::worker_v2_restart::injected_fault_point_v1;
use crate::worker_v2_restart::{
    PersistedAdmittedWorkerV2IntentV1, RestartIntentErrorV1, RestartIntentErrorV2,
    ResumeMarkerErrorV1, ResumeMarkerStateV1, ResumeMarkerStateV2, WorkerV2PublicationKindV1,
    WorkerV2ResumeStoreV1, WorkerV2ResumeStoreV2, persist_admitted_worker_v2_intent_v1,
    persist_admitted_worker_v2_intent_v2, recover_worker_v2_intent_v1, recover_worker_v2_intent_v2,
    restart_admission_commitment_with_inputs_v1,
};
use crate::{
    ARTIFACT_CHILD_FD, BACKEND_CHILD_FD, MANAGED_RUSTC_ARGS_ENV, RUSTC_CHILD_FD,
    RUSTC_INVOCATION_CHILD_FD, RUSTC_LIBRARY_CHILD_FD,
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
const CARGO_METADATA_MUTATION_TEST_ONLY_ENV_V1: &str = "FE2O3_CARGO_METADATA_MUTATION_TEST_ONLY_V1";
const QUALIFICATION_RELEASE_ACTION_ENV: &str = "FE2O3_PROTECTED_RELEASE_ACTION_V1";
const ROW_SOFTMAX_V1_PROVISION_VALUE: &str = "row-softmax-v1-provision";
const ROW_SOFTMAX_V1_RUN_VALUE: &str = "row-softmax-v1-run";
const ROW_SOFTMAX_V1_PROVISION_PREFIX: &str = "FE2O3_ROW_SOFTMAX_V1_PROVIDER_OBSERVATION=";
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
const MAX_BUILD_EXECUTABLE_BYTES: u64 = 512 * 1024 * 1024;
const MAX_PROC_STAT_BYTES: usize = 4096;
const PROCESS_CONSISTENCY_EXPECTATION_FD_V3: std::os::fd::RawFd =
    fe2o3_process_identity::S09_PROCESS_CONSISTENCY_EXPECTATION_FD_V3;
const BUILD_ATTEMPT_INPUT_DOMAIN: &[u8] = b"FE2O3/BUILD-ATTEMPT-INPUT/V2\0";
const ROW_SOFTMAX_EFFECTIVE_RUSTC_ARGV_DOMAIN_V1: &[u8] =
    b"FE2O3/ROW-SOFTMAX/EFFECTIVE-RUSTC-ARGV/V1\0";
const WORKER_V2_CONFIG_ID_DOMAIN: &[u8] = b"FE2O3/WORKER-V2-CONFIG-ID/V1\0";

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
            Self::CodegenMetadata(error) => Some(error),
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
            | Self::MissingManagedEnvironment(_)
            | Self::InvalidBuildSession
            | Self::InvalidManagedRustcArguments(_)
            | Self::InvalidCargoPrimaryPackage
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
            let managed_rustc_args = managed_rustc_args_from_environment()?;
            let metadata = ordered_rustc_codegen_metadata_v1(compile)?;
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
                || std::env::var_os("FE2O3_QUALIFICATION_ORACLE_V1").as_deref()
                    == Some(OsStr::new(ROW_SOFTMAX_V1_PIPELINE))
            {
                reject_authority_linker_arguments(compile.argv())?;
            }
            let capability_binding =
                capability_broker::CapabilityBindingV3::from_environment_for_client(
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
            let provisioning_selected = row_softmax_provisioning_selected(&compile);
            let selected_kernel_root = selected_kernel_root(
                worker_v2.as_ref().map(|config| {
                    config.selects(compile.crate_name(), compile.source_path(), &current_dir)
                }),
                std::env::var_os(crate::CARGO_PRIMARY_PACKAGE_ENV).as_deref(),
            )?;
            let managed = if !selected_kernel_root
                || (row_softmax_provisioning_requested() && !provisioning_selected)
            {
                None
            } else {
                Some(prepare_managed_attempt(
                    compile,
                    worker_v2,
                    &current_dir,
                    compiler_capabilities.output_dir(),
                    &managed_rustc_args,
                    &compiler_capabilities,
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
    let protected_kernel_root =
        managed_attempt.is_some() && selected_compilation_requires_protected_invocation();

    if managed_attempt
        .as_ref()
        .is_some_and(ManagedAttempt::is_worker_v2_recovery)
    {
        complete_managed_attempt(managed_attempt.expect("managed recovery exists"), None)?;
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
                capabilities.prepare_unmanaged_dependency_command(command.as_command_mut())?;
            } else if protected_kernel_root {
                capabilities.prepare_protected_command(command.as_command_mut())?;
            } else {
                capabilities.prepare_qualification_command(
                    command.as_command_mut(),
                    qualification_requires_compiler_closure_observation(
                        std::env::var_os("FE2O3_QUALIFICATION_ORACLE_V1").as_deref(),
                    ),
                )?;
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
            managed_attempt
                .as_ref()
                .and_then(ManagedAttempt::general_gemm_child_pins),
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
                debug_assert_eq!(capture.amd_target(), "gfx942:xnack-");
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
        if std::env::var_os("FE2O3_QUALIFICATION_ORACLE_V1").as_deref()
            == Some(OsStr::new(ROW_SOFTMAX_V1_PIPELINE))
            && let Some(managed) = managed_attempt.as_ref()
        {
            let mut effective_argv = Vec::with_capacity(command.as_command().get_args().len() + 1);
            effective_argv.push(command.configured_argv0().to_owned());
            effective_argv.extend(command.as_command().get_args().map(OsString::from));
            let capabilities = compiler_capabilities.as_ref().ok_or_else(|| {
                BindingWrapperError::CapabilityBroker(
                    "row-softmax invocation has no brokered compiler capabilities".to_owned(),
                )
            })?;
            let observed = row_softmax_effective_rustc_argv_identity(
                &effective_argv,
                capabilities.compiler_closure_sha256(),
            );
            if managed.attempt.invocation() != observed {
                return Err(BindingWrapperError::BuildObservation(
                    "row-softmax build attempt does not bind the exact prepared rustc argv"
                        .to_owned(),
                ));
            }
            let claim =
                BrokeredInvocationCapabilityClaimV1::new(managed.attempt, *observed.as_bytes())
                    .map_err(|error| BindingWrapperError::CapabilityBroker(error.to_string()))?;
            capabilities.prepare_invocation_authority(claim)?;
        }
        let parent_rustc_invocation_custody = ParentRustcInvocationCustody::retain(
            inert_rustc_invocation,
            rustc_invocation_capability,
        )
        .map_err(|error| BindingWrapperError::ChildCapability(error.to_string()))?;
        let status = command.status();
        Ok((status, parent_rustc_invocation_custody))
    })();
    let (status, parent_rustc_invocation_custody) = match pre_spawn_result {
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
            complete_managed_attempt(managed, parent_rustc_invocation_custody)?;
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

fn selected_compilation_requires_protected_invocation() -> bool {
    qualification_selection_requires_protected_invocation(
        std::env::var_os("FE2O3_QUALIFICATION_ORACLE_V1").as_deref(),
        cfg!(debug_assertions)
            && std::env::var_os(crate::NON_PRODUCTION_AUTHORITY_VALIDATION_ENV).as_deref()
                == Some(OsStr::new("1")),
    )
}

fn qualification_selection_requires_protected_invocation(
    qualification_oracle: Option<&OsStr>,
    explicit_unprotected_qualification: bool,
) -> bool {
    production_compilation_selected(qualification_oracle)
        || qualification_oracle == Some(OsStr::new(OBSOLETE_PRODUCTION_SELECTOR))
        || (qualification_oracle == Some(OsStr::new(ROW_SOFTMAX_V1_PIPELINE))
            && !explicit_unprotected_qualification)
}

fn qualification_requires_compiler_closure_observation(
    qualification_oracle: Option<&OsStr>,
) -> bool {
    qualification_oracle == Some(OsStr::new(ROW_SOFTMAX_V1_PIPELINE))
}

fn selected_kernel_root(
    worker_v2_selection: Option<bool>,
    cargo_primary_package: Option<&OsStr>,
) -> Result<bool, BindingWrapperError> {
    if let Some(selected) = worker_v2_selection {
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
    #[cfg(feature = "compiler-handoff-observation-test-only")]
    let metadata_mutation = std::env::var_os(CARGO_METADATA_MUTATION_TEST_ONLY_ENV_V1);
    #[cfg(not(feature = "compiler-handoff-observation-test-only"))]
    let metadata_mutation: Option<OsString> = None;
    configure_build_observation_environment_with_test_mutation(
        command,
        observation,
        metadata_mutation.as_deref(),
    );
}

fn configure_build_observation_environment_with_test_mutation(
    command: &mut Command,
    observation: Option<CompileBuildObservationV2>,
    metadata_mutation: Option<&OsStr>,
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
    match metadata_mutation {
        Some(mutation) => {
            command.env(CARGO_METADATA_MUTATION_TEST_ONLY_ENV_V1, mutation);
        }
        None => {
            command.env_remove(CARGO_METADATA_MUTATION_TEST_ONLY_ENV_V1);
        }
    }
}

fn reject_dynamic_loader_environment() -> Result<(), BindingWrapperError> {
    let authority_sensitive = std::env::var_os("FE2O3_QUALIFICATION_ORACLE_V1").as_deref()
        == Some(OsStr::new(ROW_SOFTMAX_V1_PIPELINE))
        || std::env::var_os(crate::worker_v2::WORKER_V2_CONFIG_ENV).is_some();
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
enum FixedReviewedInheritedInputV2 {
    CargoManifestDir,
    QualificationOracle,
    Target,
}

impl FixedReviewedInheritedInputV2 {
    const ALL: [Self; 3] = [
        Self::CargoManifestDir,
        Self::QualificationOracle,
        Self::Target,
    ];

    const fn name(self) -> &'static str {
        match self {
            Self::CargoManifestDir => "CARGO_MANIFEST_DIR",
            Self::QualificationOracle => "FE2O3_QUALIFICATION_ORACLE_V1",
            Self::Target => "FE2O3_TARGET",
        }
    }

    fn accepts(self, value: &OsStr, qualification_oracle: Option<&OsStr>) -> bool {
        match self {
            Self::CargoManifestDir => {
                let path = Path::new(value);
                path.is_absolute() && os_bytes(value).len() <= 4096
            }
            Self::QualificationOracle => qualification_oracle == Some(value),
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
    general_gemm_pins: Option<GeneralGemmChildPinsV1<'_>>,
) -> Result<Option<CompleteReviewedChildEnvironmentV2>, BindingWrapperError> {
    match profile {
        Some(WorkerV2CompileEnvironmentProfileV1::ProductionGfx942) => {
            materialize_closed_child_environment(command, inherited, None, "production").map(Some)
        }
        Some(WorkerV2CompileEnvironmentProfileV1::S09AlphaGfx942O0) => {
            materialize_closed_child_environment(
                command,
                inherited,
                Some(OsStr::new("kernel-ir-worker-v2")),
                "S09",
            )
            .map(Some)
        }
        Some(WorkerV2CompileEnvironmentProfileV1::ScalarGemmV1Gfx942) => {
            materialize_scalar_gemm_v1_child_environment(command, inherited).map(Some)
        }
        Some(WorkerV2CompileEnvironmentProfileV1::RowSoftmaxV1Gfx942) => {
            materialize_row_softmax_v1_child_environment(command, inherited).map(Some)
        }
        Some(WorkerV2CompileEnvironmentProfileV1::GeneralGemmV1Gfx942) => {
            let pins = general_gemm_pins.ok_or_else(|| {
                BindingWrapperError::BuildObservation(
                    "general GEMM child environment has no parent-authenticated Worker V2 pins"
                        .to_owned(),
                )
            })?;
            materialize_general_gemm_v1_child_environment(command, inherited, pins).map(Some)
        }
        None => Ok(None),
    }
}

#[derive(Clone, Copy)]
struct GeneralGemmChildPinsV1<'a> {
    manifest_path: &'a Path,
    expected_identity: WorkerV2ConfigIdentity,
    runtime_closure_v2_root: &'a Path,
    runtime_closure_v2_manifest_sha256: [u8; 32],
}

fn materialize_row_softmax_v1_child_environment(
    command: &mut Command,
    inherited: impl IntoIterator<Item = (OsString, OsString)>,
) -> Result<CompleteReviewedChildEnvironmentV2, BindingWrapperError> {
    let inherited = inherited.into_iter().collect::<BTreeMap<_, _>>();
    for name in inherited.keys() {
        if rejected_reviewed_inherited_environment(name) {
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
    if required("FE2O3_QUALIFICATION_ORACLE_V1")? != ROW_SOFTMAX_V1_PIPELINE {
        return Err(BindingWrapperError::BuildObservation(
            "row-softmax child environment has changed FE2O3_QUALIFICATION_ORACLE_V1".to_owned(),
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
            OsString::from("FE2O3_QUALIFICATION_ORACLE_V1"),
            OsString::from(ROW_SOFTMAX_V1_PIPELINE),
        ),
        (OsString::from(TARGET_ENV), OsString::from("gfx942:xnack-")),
        (
            OsString::from(crate::EXPECTED_COMPILER_CLOSURE_SHA256_ENV),
            required(crate::EXPECTED_COMPILER_CLOSURE_SHA256_ENV)?.clone(),
        ),
    ]);
    if cfg!(debug_assertions)
        && inherited
            .get(OsStr::new(crate::NON_PRODUCTION_AUTHORITY_VALIDATION_ENV))
            .is_some_and(|value| value == "1")
    {
        final_environment.insert(
            OsString::from(crate::NON_PRODUCTION_AUTHORITY_VALIDATION_ENV),
            OsString::from("1"),
        );
    }
    let explicit = command
        .get_envs()
        .map(|(name, value)| (name.to_owned(), value.map(OsString::from)))
        .collect::<Vec<_>>();
    for (name, value) in explicit {
        if apply_managed_loader_environment(&mut final_environment, &name, value.as_deref())? {
            continue;
        }
        if name == OsStr::new(crate::NON_PRODUCTION_AUTHORITY_VALIDATION_ENV) {
            let existing = final_environment.get(name.as_os_str());
            let idempotent = matches!(
                (value.as_deref(), existing),
                (Some(value), Some(existing)) if value == "1" && existing == "1"
            ) || matches!((value.as_deref(), existing), (None, None));
            if !idempotent {
                return Err(BindingWrapperError::BuildObservation(format!(
                    "row-softmax command changed the exact qualification marker {name:?}"
                )));
            }
            continue;
        }
        if !managed_reviewed_child_environment(&name) {
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
    let unprotected_qualification = final_environment
        .get(OsStr::new(crate::NON_PRODUCTION_AUTHORITY_VALIDATION_ENV))
        .is_some_and(|value| value == "1");
    let backend_name = if unprotected_qualification {
        QUALIFICATION_CODEGEN_BACKEND_SHA256_ENV_V1
    } else {
        CODEGEN_BACKEND_BUILD_OBSERVATION_ENV_V2
    };
    require_canonical_sha256_environment(&final_environment, backend_name, "row-softmax")?;
    require_canonical_sha256_environment(
        &final_environment,
        crate::EXPECTED_COMPILER_CLOSURE_SHA256_ENV,
        "row-softmax compiler closure",
    )?;
    command.env_clear();
    command.envs(&final_environment);
    Ok(CompleteReviewedChildEnvironmentV2 {
        entries: final_environment.into_iter().collect(),
    })
}

fn materialize_closed_child_environment(
    command: &mut Command,
    inherited: impl IntoIterator<Item = (OsString, OsString)>,
    qualification_oracle: Option<&OsStr>,
    profile: &str,
) -> Result<CompleteReviewedChildEnvironmentV2, BindingWrapperError> {
    let inherited = inherited.into_iter().collect::<BTreeMap<_, _>>();
    for name in inherited.keys() {
        if rejected_reviewed_inherited_environment(name) {
            return Err(BindingWrapperError::BuildObservation(format!(
                "{profile} child environment rejects inherited variable {name:?}"
            )));
        }
    }

    let mut final_environment = BTreeMap::new();
    for input in FixedReviewedInheritedInputV2::ALL {
        if input == FixedReviewedInheritedInputV2::QualificationOracle
            && qualification_oracle.is_none()
        {
            continue;
        }
        let name = input.name();
        let value = inherited.get(OsStr::new(name)).ok_or_else(|| {
            BindingWrapperError::BuildObservation(format!(
                "{profile} fixed environment is missing required {name}"
            ))
        })?;
        if !input.accepts(value, qualification_oracle) {
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
        if rejected_reviewed_inherited_environment(name) {
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
    if required("FE2O3_QUALIFICATION_ORACLE_V1")? != SCALAR_GEMM_V1_PIPELINE {
        return Err(BindingWrapperError::BuildObservation(
            "scalar GEMM child environment has changed FE2O3_QUALIFICATION_ORACLE_V1".to_owned(),
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
            OsString::from("FE2O3_QUALIFICATION_ORACLE_V1"),
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
        if !managed_reviewed_child_environment(&name) {
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

fn materialize_general_gemm_v1_child_environment(
    command: &mut Command,
    inherited: impl IntoIterator<Item = (OsString, OsString)>,
    pins: GeneralGemmChildPinsV1<'_>,
) -> Result<CompleteReviewedChildEnvironmentV2, BindingWrapperError> {
    let inherited = inherited.into_iter().collect::<BTreeMap<_, _>>();
    for (name, value) in &inherited {
        let Some(name_text) = name.to_str() else {
            return Err(BindingWrapperError::BuildObservation(
                "general GEMM child environment contains a non-UTF-8 variable name".to_owned(),
            ));
        };
        if value.to_str().is_none() {
            return Err(BindingWrapperError::BuildObservation(format!(
                "general GEMM child environment variable {name_text} has a non-UTF-8 value"
            )));
        }
        if rejected_reviewed_inherited_environment(name) {
            return Err(BindingWrapperError::BuildObservation(format!(
                "general GEMM child environment rejects inherited variable {name_text}"
            )));
        }
        if name_text.starts_with("FE2O3_") && !reviewed_scalar_inherited_environment(name) {
            return Err(BindingWrapperError::BuildObservation(format!(
                "general GEMM child environment rejects unreviewed inherited variable {name_text}"
            )));
        }
    }

    let required = |name: &'static str| {
        inherited.get(OsStr::new(name)).ok_or_else(|| {
            BindingWrapperError::BuildObservation(format!(
                "general GEMM child environment is missing required {name}"
            ))
        })
    };
    let manifest_dir = required("CARGO_MANIFEST_DIR")?;
    if !canonical_absolute_utf8_path(manifest_dir) {
        return Err(BindingWrapperError::BuildObservation(
            "general GEMM child environment has invalid CARGO_MANIFEST_DIR".to_owned(),
        ));
    }
    if required("FE2O3_QUALIFICATION_ORACLE_V1")? != crate::worker_v2::GENERAL_GEMM_V1_PIPELINE {
        return Err(BindingWrapperError::BuildObservation(
            "general GEMM child environment has changed FE2O3_QUALIFICATION_ORACLE_V1".to_owned(),
        ));
    }
    if required(TARGET_ENV)? != "gfx942:xnack-" {
        return Err(BindingWrapperError::BuildObservation(
            "general GEMM child environment has missing or changed FE2O3_TARGET".to_owned(),
        ));
    }
    if required(WORKER_V2_CONFIG_ENV)? != pins.manifest_path.as_os_str() {
        return Err(BindingWrapperError::BuildObservation(
            "general GEMM child environment has changed FE2O3_WORKER_V2_CONFIG_V2".to_owned(),
        ));
    }
    let expected_identity = pins.expected_identity.to_hex();
    if required(WORKER_V2_EXPECTED_ID_ENV)? != OsStr::new(&expected_identity) {
        return Err(BindingWrapperError::BuildObservation(
            "general GEMM child environment has changed FE2O3_WORKER_V2_EXPECTED_ID_V1".to_owned(),
        ));
    }
    let runtime_closure_v2_manifest_sha256 = hex(&pins.runtime_closure_v2_manifest_sha256);
    for (name, expected) in [
        (
            GENERAL_GEMM_RUNTIME_CLOSURE_V2_ROOT_ENV,
            pins.runtime_closure_v2_root.as_os_str(),
        ),
        (
            GENERAL_GEMM_RUNTIME_CLOSURE_V2_MANIFEST_SHA256_ENV,
            OsStr::new(&runtime_closure_v2_manifest_sha256),
        ),
    ] {
        if inherited
            .get(OsStr::new(name))
            .is_some_and(|actual| actual != expected)
        {
            return Err(BindingWrapperError::BuildObservation(format!(
                "general GEMM child environment has changed {name}"
            )));
        }
    }
    let verification = inherited
        .get(OsStr::new(VERIFY_KERNEL_IR_ENV))
        .map_or(OsStr::new("0"), OsString::as_os_str);
    if !matches!(verification.to_str(), Some("0" | "1")) {
        return Err(BindingWrapperError::BuildObservation(format!(
            "general GEMM child environment has invalid {VERIFY_KERNEL_IR_ENV}"
        )));
    }

    let mut final_environment = BTreeMap::from([
        (OsString::from("CARGO_MANIFEST_DIR"), manifest_dir.clone()),
        (
            OsString::from("FE2O3_QUALIFICATION_ORACLE_V1"),
            OsString::from(crate::worker_v2::GENERAL_GEMM_V1_PIPELINE),
        ),
        (OsString::from(TARGET_ENV), OsString::from("gfx942:xnack-")),
        (
            OsString::from(VERIFY_KERNEL_IR_ENV),
            verification.to_owned(),
        ),
        (
            OsString::from(WORKER_V2_CONFIG_ENV),
            pins.manifest_path.as_os_str().to_owned(),
        ),
        (
            OsString::from(WORKER_V2_EXPECTED_ID_ENV),
            OsString::from(expected_identity),
        ),
        (
            OsString::from(GENERAL_GEMM_RUNTIME_CLOSURE_V2_ROOT_ENV),
            pins.runtime_closure_v2_root.as_os_str().to_owned(),
        ),
        (
            OsString::from(GENERAL_GEMM_RUNTIME_CLOSURE_V2_MANIFEST_SHA256_ENV),
            OsString::from(runtime_closure_v2_manifest_sha256),
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
        if !managed_reviewed_child_environment(&name) {
            return Err(BindingWrapperError::BuildObservation(format!(
                "general GEMM command has unreviewed explicit environment mutation {name:?}"
            )));
        }
        if name == WORKER_V2_SOURCE_DEBUG_PROFILE_ENV && value.is_some() {
            return Err(BindingWrapperError::BuildObservation(
                "general GEMM command cannot select an S09 source-debug profile".to_owned(),
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
    validate_general_gemm_v1_final_environment(&final_environment)?;
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
        b"FE2O3_QUALIFICATION_ORACLE_V1"
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
            | b"FE2O3_GENERAL_GEMM_RUNTIME_CLOSURE_V2_ROOT"
            | b"FE2O3_GENERAL_GEMM_RUNTIME_CLOSURE_V2_MANIFEST_SHA256"
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
    require_canonical_sha256_environment(
        environment,
        QUALIFICATION_CODEGEN_BACKEND_SHA256_ENV_V1,
        "scalar GEMM",
    )?;
    Ok(())
}

fn validate_general_gemm_v1_final_environment(
    environment: &BTreeMap<OsString, OsString>,
) -> Result<(), BindingWrapperError> {
    let required = |name: &'static str| {
        environment
            .get(OsStr::new(name))
            .and_then(|value| value.to_str())
            .ok_or_else(|| {
                BindingWrapperError::BuildObservation(format!(
                    "general GEMM final environment is missing valid {name}"
                ))
            })
    };
    if required(HSACO_DIR_ENV)? != format!("/proc/self/fd/{ARTIFACT_CHILD_FD}") {
        return Err(BindingWrapperError::BuildObservation(
            "general GEMM final environment has changed FE2O3_HSACO_DIR".to_owned(),
        ));
    }
    if !canonical_absolute_utf8_path(OsStr::new(required(WORKER_V2_CONFIG_ENV)?)) {
        return Err(BindingWrapperError::BuildObservation(
            "general GEMM final environment has invalid FE2O3_WORKER_V2_CONFIG_V2".to_owned(),
        ));
    }
    let expected_worker = required(WORKER_V2_EXPECTED_ID_ENV)?;
    if expected_worker.len() != 64
        || expected_worker
            .bytes()
            .any(|byte| !(byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)))
    {
        return Err(BindingWrapperError::BuildObservation(
            "general GEMM final environment has invalid FE2O3_WORKER_V2_EXPECTED_ID_V1".to_owned(),
        ));
    }
    if !canonical_absolute_utf8_path(OsStr::new(required(
        GENERAL_GEMM_RUNTIME_CLOSURE_V2_ROOT_ENV,
    )?)) {
        return Err(BindingWrapperError::BuildObservation(
            "general GEMM final environment has invalid runtime-closure V2 root".to_owned(),
        ));
    }
    let runtime_manifest = required(GENERAL_GEMM_RUNTIME_CLOSURE_V2_MANIFEST_SHA256_ENV)?;
    if runtime_manifest != hex(&fe2o3_verifier::GENERAL_GEMM_RUNTIME_CLOSURE_V2_MANIFEST_SHA256) {
        return Err(BindingWrapperError::BuildObservation(
            "general GEMM final environment has changed the runtime-closure V2 manifest".to_owned(),
        ));
    }
    let attempt = BuildAttempt::from_env_value(required(BUILD_ATTEMPT_ENV)?).map_err(|_| {
        BindingWrapperError::BuildObservation(
            "general GEMM final environment has invalid FE2O3_BUILD_ATTEMPT_V1".to_owned(),
        )
    })?;
    if attempt.session() == BuildSession::DIRECT {
        return Err(BindingWrapperError::BuildObservation(
            "general GEMM final environment has invalid FE2O3_BUILD_ATTEMPT_V1".to_owned(),
        ));
    }
    require_canonical_sha256_environment(
        environment,
        QUALIFICATION_CODEGEN_BACKEND_SHA256_ENV_V1,
        "general GEMM",
    )?;
    Ok(())
}

fn require_canonical_sha256_environment(
    environment: &BTreeMap<OsString, OsString>,
    name: &'static str,
    profile: &'static str,
) -> Result<(), BindingWrapperError> {
    let value = environment
        .get(OsStr::new(name))
        .and_then(|value| value.to_str())
        .ok_or_else(|| {
            BindingWrapperError::BuildObservation(format!(
                "{profile} final environment is missing valid {name}"
            ))
        })?;
    if value.len() != 64
        || value.bytes().all(|byte| byte == b'0')
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(BindingWrapperError::BuildObservation(format!(
            "{profile} final environment has invalid {name}"
        )));
    }
    Ok(())
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
    matches!(
        os_bytes(name),
        b"LANG"
            | b"PATH"
            | b"TMPDIR"
            | b"FE2O3_HSACO_DIR"
            | b"FE2O3_BUILD_ATTEMPT_V1"
            | b"FE2O3_CARGO_METADATA_BUILD_OBSERVATION_V2"
            | b"FE2O3_CARGO_METADATA_MUTATION_TEST_ONLY_V1"
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
    binding: capability_broker::CapabilityBindingV3,
    backend: PinnedCodegenBackend,
    artifact: PinnedDirectory,
    compiler_closure: Option<fe2o3_compiler_closure_capability::CompilerClosureCapabilityV1>,
    invocation_authority: Option<capability_broker::BrokeredInvocationAuthorityV1>,
    output_dir: PathBuf,
    pinned_cargo_image_sha256: Option<[u8; 32]>,
}

impl CompilerCapabilities {
    fn from_environment(
        binding: capability_broker::CapabilityBindingV3,
    ) -> Result<Self, BindingWrapperError> {
        let mut transferred = capability_broker::receive(managed_build_session()?, binding)
            .map_err(BindingWrapperError::CapabilityBroker)?;
        let invocation_authority = transferred.invocation_authority.take().ok_or_else(|| {
            BindingWrapperError::CapabilityBroker(
                "capability broker omitted invocation authority".to_owned(),
            )
        })?;
        let invocation_authority = if std::env::var_os("FE2O3_QUALIFICATION_ORACLE_V1").as_deref()
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
        let compiler_closure = transferred.compiler_closure.take();
        if binding.requires_compiler_closure_v2() != compiler_closure.is_some() {
            return Err(BindingWrapperError::CapabilityBroker(
                "brokered compiler-closure descriptor presence differs from the authenticated binding"
                    .to_owned(),
            ));
        }
        if let Some(capability) = &compiler_closure {
            capability
                .revalidate()
                .map_err(BindingWrapperError::CapabilityBroker)?;
            let closure = capability.closure();
            if closure.identity_sha256() != binding.compiler_closure_sha256()
                || closure.rustc_executable_sha256() != binding.rustc_executable_sha256()
                || closure.codegen_backend_sha256() != *transferred.backend.sha256()
                || pinned_cargo_image_sha256
                    .is_some_and(|cargo| closure.cargo_executable_sha256() != cargo)
            {
                return Err(BindingWrapperError::CapabilityBroker(
                    "brokered compiler closure differs from the retained compiler capabilities"
                        .to_owned(),
                ));
            }
        }
        let output_dir = transferred.artifact.child_path();
        Ok(Self {
            binding,
            backend: transferred.backend,
            artifact: transferred.artifact,
            compiler_closure,
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
        self.inherit_invocation_authority(command)?;
        Ok(())
    }

    fn prepare_qualification_command(
        &self,
        command: &mut Command,
        compiler_closure_observation: bool,
    ) -> Result<(), BindingWrapperError> {
        self.prepare_artifact_command(command)?;
        configure_qualification_route_marker(
            command,
            cfg!(debug_assertions) && compiler_closure_observation,
        );
        command.env_remove(CODEGEN_BACKEND_BUILD_OBSERVATION_ENV_V2);
        if compiler_closure_observation {
            command
                .env(
                    QUALIFICATION_CODEGEN_BACKEND_SHA256_ENV_V1,
                    hex(&self.backend.sha256()[..]),
                )
                .env(
                    crate::EXPECTED_COMPILER_CLOSURE_SHA256_ENV,
                    hex(&self.compiler_closure_sha256()),
                );
        } else {
            command
                .env_remove(QUALIFICATION_CODEGEN_BACKEND_SHA256_ENV_V1)
                .env_remove(crate::EXPECTED_COMPILER_CLOSURE_SHA256_ENV);
        }
        self.inherit_invocation_authority(command)?;
        Ok(())
    }

    fn prepare_unmanaged_dependency_command(
        &self,
        command: &mut Command,
    ) -> Result<(), BindingWrapperError> {
        self.prepare_backend_command(command)?;
        scope_unmanaged_dependency_environment(command);
        Ok(())
    }

    fn prepare_backend_command(&self, command: &mut Command) -> Result<(), BindingWrapperError> {
        self.backend
            .replace_for_child_at(command, BACKEND_CHILD_FD)
            .map_err(|error| BindingWrapperError::ChildCapability(error.to_string()))
    }

    fn inherit_invocation_authority(
        &self,
        command: &mut Command,
    ) -> Result<(), BindingWrapperError> {
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

    fn row_softmax_authority_policy(
        &self,
        attempt: BuildAttempt,
        provider: RowSoftmaxV1ProviderManifestV1,
    ) -> Result<RowSoftmaxV1AuthorityPolicyV1, BindingWrapperError> {
        let cargo = self.pinned_cargo_image_sha256.ok_or_else(|| {
            BindingWrapperError::BuildObservation(
                "row-softmax release has no broker-authenticated Cargo image".to_owned(),
            )
        })?;
        let rustc = self.binding.rustc_executable_sha256();
        let backend = self.backend_sha256();
        let runtime = measure_inherited_rustc_runtime_tree()?;
        let compiler = RowSoftmaxV1CompilerClosurePolicyV1::new(cargo, rustc, runtime, backend)
            .map_err(|error| BindingWrapperError::BuildObservation(error.to_string()))?;
        if compiler.identity_sha256() != self.compiler_closure_sha256()
            || crate::compiler_toolchain::compiler_closure_sha256_v1(
                &cargo, &rustc, &runtime, &backend,
            ) != self.compiler_closure_sha256()
        {
            return Err(BindingWrapperError::BuildObservation(
                "row-softmax measured compiler inputs differ from the broker-authenticated closure"
                    .to_owned(),
            ));
        }
        let broker = measure_build_executable("/proc/self/exe", "cargo-fe2o3 broker image")?;
        RowSoftmaxV1AuthorityPolicyV1::new(provider, attempt, broker, compiler)
            .map_err(|error| BindingWrapperError::BuildObservation(error.to_string()))
    }
}

fn configure_qualification_route_marker(command: &mut Command, debug_build: bool) {
    if debug_build {
        command.env(crate::NON_PRODUCTION_AUTHORITY_VALIDATION_ENV, "1");
    } else {
        command.env_remove(crate::NON_PRODUCTION_AUTHORITY_VALIDATION_ENV);
    }
}

fn scope_unmanaged_dependency_environment(command: &mut Command) {
    // Production is the compiler default, so an unselected dependency needs an
    // explicit non-authoritative route rather than an absent selector. This route
    // can diagnose forged kernel providers but cannot publish artifacts.
    command.env("FE2O3_QUALIFICATION_ORACLE_V1", "kernel-ir-v1");
    command.env_remove("FE2O3_CODEGEN_PIPELINE");
    for name in [
        HSACO_DIR_ENV,
        CODEGEN_BACKEND_BUILD_OBSERVATION_ENV_V2,
        QUALIFICATION_CODEGEN_BACKEND_SHA256_ENV_V1,
        crate::EXPECTED_COMPILER_CLOSURE_SHA256_ENV,
        crate::NON_PRODUCTION_AUTHORITY_VALIDATION_ENV,
        WORKER_V2_CONFIG_ENV,
        WORKER_V2_EXPECTED_ID_ENV,
        QUALIFICATION_RELEASE_ACTION_ENV,
    ] {
        command.env_remove(name);
    }
}

fn measure_inherited_rustc_runtime_tree() -> Result<[u8; 32], BindingWrapperError> {
    // SAFETY: the fixed descriptor is inherited from the authenticated Cargo boundary and is
    // borrowed only long enough to create a close-on-exec duplicate.
    let inherited = unsafe { BorrowedFd::borrow_raw(RUSTC_LIBRARY_CHILD_FD) };
    let duplicate = rustix::io::fcntl_dupfd_cloexec(inherited, 200).map_err(|error| {
        BindingWrapperError::BuildObservation(format!(
            "cannot duplicate retained rustc runtime-tree descriptor: {error}"
        ))
    })?;
    let directory = PinnedDirectory::from_transferred_file(
        File::from(duplicate),
        "row-softmax retained rustc runtime tree",
    )
    .map_err(BindingWrapperError::BuildObservation)?;
    let runtime = crate::rustc_lib_tree::PinnedRustcLibTree::pin(directory)
        .map_err(BindingWrapperError::BuildObservation)?;
    runtime
        .revalidate()
        .map_err(BindingWrapperError::BuildObservation)?;
    Ok(*runtime.sha256())
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
    row_softmax_release: Option<RowSoftmaxReleaseContext>,
    row_softmax_provision: bool,
    #[cfg(feature = "compiler-handoff-observation-test-only")]
    compiler_handoff_observation: Option<crate::compiler_handoff_observation::Request>,
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
            eprintln!(
                "[cargo-fe2o3] failed to revoke managed build attempt after pre-spawn error: {error}"
            );
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

struct RowSoftmaxReleaseContext {
    authority: RowSoftmaxV1AuthorityPolicyV1,
    workload: Option<AdmittedRowSoftmaxV1WorkloadV1>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum WorkerV2BindingSchema {
    OrdinaryV1,
    ProtectedV2,
    ProductionV3,
}

impl WorkerV2BindingSchema {
    const fn select(
        compiler_closure: Option<CompilerClosureV2>,
        production_v1: bool,
    ) -> Result<Self, &'static str> {
        match (compiler_closure, production_v1) {
            (Some(_), true) => Ok(Self::ProductionV3),
            (Some(_), false) => Ok(Self::ProtectedV2),
            (None, false) => Ok(Self::OrdinaryV1),
            (None, true) => Err(
                "production-v1 requires protected V3 compiler-closure custody before route selection",
            ),
        }
    }

    const fn is_protected(self) -> bool {
        matches!(self, Self::ProtectedV2 | Self::ProductionV3)
    }
}

fn worker_v3_readiness_is_absent(error: &WorkerV3LoadEnvelopeErrorV1) -> bool {
    matches!(
        error,
        WorkerV3LoadEnvelopeErrorV1::LoadReadiness(
            fe2o3_artifact_transaction::WorkerV3LoadReadinessErrorV1::AttemptState
                | fe2o3_artifact_transaction::WorkerV3LoadReadinessErrorV1::MissingEnvelope
                | fe2o3_artifact_transaction::WorkerV3LoadReadinessErrorV1::MissingClaim
                | fe2o3_artifact_transaction::WorkerV3LoadReadinessErrorV1::MissingReceipt
        )
    )
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ProtectedWorkerV2TransitionBlocker {
    RowSoftmax,
    InRustcExecution,
}

const fn protected_worker_v2_transition_blocker(
    schema: WorkerV2BindingSchema,
    row_softmax: bool,
    in_rustc_execution: bool,
) -> Option<ProtectedWorkerV2TransitionBlocker> {
    if !schema.is_protected() {
        return None;
    }
    if row_softmax {
        Some(ProtectedWorkerV2TransitionBlocker::RowSoftmax)
    } else if in_rustc_execution {
        Some(ProtectedWorkerV2TransitionBlocker::InRustcExecution)
    } else {
        None
    }
}

enum ManagedWorkerV2 {
    InProcessGeneralGemm {
        config: Box<PreparedWorkerV2Config>,
    },
    FreshV1 {
        config: Box<PreparedWorkerV2Config>,
        envelope_inputs: Option<WorkerV2EnvelopeInputsV1>,
        resume: WorkerV2ResumeStoreV1,
    },
    RecoveryV1 {
        resume: WorkerV2ResumeStoreV1,
        state: Box<ResumeMarkerStateV1>,
    },
    FreshV2 {
        config: Box<PreparedWorkerV2Config>,
        envelope_inputs: Option<WorkerV2EnvelopeInputsV1>,
        resume: WorkerV2ResumeStoreV2,
        compiler_closure: CompilerClosureV2,
        producer_binding: WorkerV2ProducerBindingV2,
    },
    RecoveryV2 {
        resume: WorkerV2ResumeStoreV2,
        state: Box<ResumeMarkerStateV2>,
        compiler_closure: CompilerClosureV2,
        producer_binding: WorkerV2ProducerBindingV2,
    },
    FreshV3 {
        config: Box<PreparedWorkerV2Config>,
        compiler_closure: CompilerClosureV2,
    },
    RecoveryV3 {
        recovered: Box<RecoveredProtectedWorkerV3HsacoPublicationV1>,
        compiler_closure: CompilerClosureV2,
    },
    RecoveryReadyV3 {
        envelope: Box<RecoveredWorkerV3LoadEnvelopeV1>,
    },
}

enum CompletionFailure {
    Uncommitted(String),
    PreserveAttempt(String),
}

impl ManagedAttempt {
    fn is_worker_v2_recovery(&self) -> bool {
        matches!(
            self.worker_v2,
            Some(
                ManagedWorkerV2::RecoveryV1 { .. }
                    | ManagedWorkerV2::RecoveryV2 { .. }
                    | ManagedWorkerV2::RecoveryV3 { .. }
                    | ManagedWorkerV2::RecoveryReadyV3 { .. }
            )
        )
    }

    fn source_debug_profile(&self) -> Option<WorkerV2SourceDebugProfileV1> {
        match &self.worker_v2 {
            Some(ManagedWorkerV2::InProcessGeneralGemm { config, .. }) => {
                config.source_debug_profile()
            }
            Some(
                ManagedWorkerV2::FreshV1 { config, .. }
                | ManagedWorkerV2::FreshV2 { config, .. }
                | ManagedWorkerV2::FreshV3 { config, .. },
            ) => config.source_debug_profile(),
            Some(
                ManagedWorkerV2::RecoveryV1 { .. }
                | ManagedWorkerV2::RecoveryV2 { .. }
                | ManagedWorkerV2::RecoveryV3 { .. }
                | ManagedWorkerV2::RecoveryReadyV3 { .. },
            )
            | None => None,
        }
    }

    const fn compile_environment_profile(&self) -> Option<WorkerV2CompileEnvironmentProfileV1> {
        self.compile_environment_profile
    }

    fn general_gemm_child_pins(&self) -> Option<GeneralGemmChildPinsV1<'_>> {
        match &self.worker_v2 {
            Some(ManagedWorkerV2::InProcessGeneralGemm { config }) => {
                let pair = config
                    .general_gemm_v1()
                    .expect("in-process general GEMM has runtime-closure pins");
                Some(GeneralGemmChildPinsV1 {
                    manifest_path: config.manifest_path(),
                    expected_identity: config.identity(),
                    runtime_closure_v2_root: pair.runtime_closure_v2_root(),
                    runtime_closure_v2_manifest_sha256: pair.runtime_closure_v2_manifest_sha256(),
                })
            }
            Some(
                ManagedWorkerV2::FreshV1 { .. }
                | ManagedWorkerV2::RecoveryV1 { .. }
                | ManagedWorkerV2::FreshV2 { .. }
                | ManagedWorkerV2::RecoveryV2 { .. }
                | ManagedWorkerV2::FreshV3 { .. }
                | ManagedWorkerV2::RecoveryV3 { .. }
                | ManagedWorkerV2::RecoveryReadyV3 { .. },
            )
            | None => None,
        }
    }

    fn protected_source_path(&self) -> Option<&Path> {
        self.protected_source_path.as_deref()
    }

    fn worker_build_observation(
        &self,
        pinned_cargo_image_sha256: [u8; 32],
    ) -> Result<Option<WorkerV2BuildObservation<'_>>, BindingWrapperError> {
        match &self.worker_v2 {
            Some(
                ManagedWorkerV2::FreshV1 { config, .. }
                | ManagedWorkerV2::FreshV2 { config, .. }
                | ManagedWorkerV2::FreshV3 { config, .. },
            ) if config.source_debug_profile().is_some() => {
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
            Some(ManagedWorkerV2::InProcessGeneralGemm { .. })
            | Some(ManagedWorkerV2::FreshV1 { .. })
            | Some(ManagedWorkerV2::RecoveryV1 { .. })
            | Some(ManagedWorkerV2::FreshV2 { .. })
            | Some(ManagedWorkerV2::RecoveryV2 { .. })
            | Some(ManagedWorkerV2::FreshV3 { .. })
            | Some(ManagedWorkerV2::RecoveryV3 { .. })
            | Some(ManagedWorkerV2::RecoveryReadyV3 { .. })
            | None => Ok(None),
        }
    }
}

fn prepare_managed_attempt(
    compile: RustcCompileInvocationV2<'_>,
    worker_v2: Option<PreparedWorkerV2Config>,
    current_dir: &std::path::Path,
    output_dir: &Path,
    managed_rustc_args: &[OsString],
    compiler_capabilities: &CompilerCapabilities,
) -> Result<ManagedAttempt, BindingWrapperError> {
    #[cfg(feature = "compiler-handoff-observation-test-only")]
    let compiler_handoff_observation = {
        let ordered_metadata = ordered_rustc_codegen_metadata_v1(compile)?;
        crate::compiler_handoff_observation::Request::for_compile(
            compile.crate_name(),
            compile.source_path(),
            &ordered_metadata,
        )
        .map_err(BindingWrapperError::BuildObservation)?
    };
    let compile_environment_profile = if std::env::var_os("FE2O3_QUALIFICATION_ORACLE_V1")
        .as_deref()
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
    let invocation = if std::env::var_os("FE2O3_QUALIFICATION_ORACLE_V1").as_deref()
        == Some(OsStr::new(ROW_SOFTMAX_V1_PIPELINE))
    {
        let mut effective_argv =
            Vec::with_capacity(compile.argv().len() + managed_rustc_args.len());
        effective_argv.extend_from_slice(compile.argv());
        effective_argv.extend_from_slice(managed_rustc_args);
        row_softmax_effective_rustc_argv_identity(
            &effective_argv,
            compiler_capabilities.compiler_closure_sha256(),
        )
    } else {
        derive_build_attempt_input(
            compile.argv(),
            worker_v2.as_ref(),
            current_dir,
            compiler_capabilities.compiler_closure_sha256(),
        )
    };
    let protected_compiler_closure = compiler_capabilities.protected_compiler_closure()?;
    let production_v1_worker = worker_v2
        .as_ref()
        .is_some_and(PreparedWorkerV2Config::is_production_compilation);
    let worker_v2_binding_schema =
        WorkerV2BindingSchema::select(protected_compiler_closure, production_v1_worker)
            .map_err(|error| BindingWrapperError::BuildObservation(error.to_owned()))?;
    let release_action = std::env::var_os(QUALIFICATION_RELEASE_ACTION_ENV);
    let row_softmax_provision =
        release_action.as_deref() == Some(OsStr::new(ROW_SOFTMAX_V1_PROVISION_VALUE));
    let row_softmax_release = worker_v2
        .as_ref()
        .and_then(PreparedWorkerV2Config::row_softmax_v1)
        .map(|row| {
            if release_action.as_deref() != Some(OsStr::new(ROW_SOFTMAX_V1_RUN_VALUE)) {
                return Err(BindingWrapperError::BuildObservation(
                    "row-softmax production pin contract requires cargo fe2o3 authority release run"
                        .to_owned(),
                ));
            }
            let workload = preflight_row_softmax_v1_workload_v1(row.workload())
                .map_err(|error| BindingWrapperError::BuildObservation(error.to_string()))?;
            Ok((row.provider(), workload))
        })
        .transpose()?;
    if row_softmax_provision && worker_v2.is_some() {
        return Err(BindingWrapperError::BuildObservation(
            "row-softmax provider provisioning rejects a Worker V2 configuration".to_owned(),
        ));
    }
    if row_softmax_release.is_none() && release_action.is_some() && !row_softmax_provision {
        return Err(BindingWrapperError::BuildObservation(
            "protected row-softmax release action has no exact row pin contract".to_owned(),
        ));
    }
    let blocker = protected_worker_v2_transition_blocker(
        worker_v2_binding_schema,
        row_softmax_release.is_some() || row_softmax_provision,
        worker_v2
            .as_ref()
            .is_some_and(PreparedWorkerV2Config::executes_worker_in_rustc),
    );
    if let Some(blocker) = blocker {
        let message = match blocker {
            ProtectedWorkerV2TransitionBlocker::RowSoftmax => {
                "protected row-softmax requires the closure-bound V2 load envelope and runtime transition; V1 handoff and execution fallback is forbidden"
            }
            ProtectedWorkerV2TransitionBlocker::InRustcExecution => {
                "protected in-rustc Worker V2 execution has no closure-bound restart/publication transition"
            }
        };
        return Err(BindingWrapperError::BuildObservation(message.to_owned()));
    }
    let (attempt, worker_v2, began_attempt) = if let Some(config) = worker_v2 {
        if config.executes_worker_in_rustc() {
            let pair = config
                .general_gemm_v1()
                .expect("in-process general GEMM has parsed qualification-pair pins");
            debug_assert!(pair.runtime_closure_v2_root().is_absolute());
            debug_assert_eq!(
                pair.runtime_closure_v2_manifest_sha256(),
                fe2o3_verifier::GENERAL_GEMM_RUNTIME_CLOSURE_V2_MANIFEST_SHA256
            );
            debug_assert_ne!(pair.proof_timeout_seconds(), 0);
            let attempt = begin_build_attempt(output_dir, &producer, invocation, session)
                .map_err(BindingWrapperError::Artifact)?;
            (
                attempt,
                Some(ManagedWorkerV2::InProcessGeneralGemm {
                    config: Box::new(config),
                }),
                true,
            )
        } else if worker_v2_binding_schema == WorkerV2BindingSchema::ProductionV3 {
            let compiler_closure = protected_compiler_closure
                .expect("production V3 schema retains its exact compiler closure");
            let attempt = begin_build_attempt(output_dir, &producer, invocation, session)
                .map_err(BindingWrapperError::Artifact)?;
            let recovered_envelope = match recover_worker_v3_load_envelope_v1(output_dir, attempt) {
                Ok(envelope) => Some(envelope),
                Err(error) if worker_v3_readiness_is_absent(&error) => None,
                Err(error) => {
                    return Err(BindingWrapperError::BuildObservation(format!(
                        "production V3 load-readiness recovery failed closed: {error}"
                    )));
                }
            };
            if let Some(envelope) = recovered_envelope {
                (
                    attempt,
                    Some(ManagedWorkerV2::RecoveryReadyV3 {
                        envelope: Box::new(envelope),
                    }),
                    false,
                )
            } else {
                match recover_protected_worker_v3_hsaco_publication_v1(
                    output_dir, &producer, attempt,
                ) {
                    Ok(recovered) => (
                        attempt,
                        Some(ManagedWorkerV2::RecoveryV3 {
                            recovered: Box::new(recovered),
                            compiler_closure,
                        }),
                        false,
                    ),
                    Err(WorkerV3HsacoPublicationErrorV1::Storage(
                        WorkerV3PublicationIntentErrorV1::NotFound,
                    )) => (
                        attempt,
                        Some(ManagedWorkerV2::FreshV3 {
                            config: Box::new(config),
                            compiler_closure,
                        }),
                        true,
                    ),
                    Err(error) => {
                        return Err(BindingWrapperError::BuildObservation(format!(
                            "production V3 restart recovery failed closed: {error}"
                        )));
                    }
                }
            }
        } else if worker_v2_binding_schema == WorkerV2BindingSchema::ProtectedV2 {
            let compiler_closure = protected_compiler_closure
                .expect("protected schema retains its exact compiler closure");
            let producer_binding = WorkerV2ProducerBindingV2::from_codegen(
                compile.crate_name(),
                Some(compile.source_path()),
            )
            .map_err(|error| {
                BindingWrapperError::BuildObservation(format!(
                    "protected Worker V2 producer binding is invalid: {error}"
                ))
            })?;
            let resume = WorkerV2ResumeStoreV2::open(output_dir, &producer)
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
                (
                    attempt,
                    Some(ManagedWorkerV2::RecoveryV2 {
                        resume,
                        state: Box::new(state),
                        compiler_closure,
                        producer_binding,
                    }),
                    false,
                )
            } else {
                let envelope_inputs = config
                    .load_envelope_inputs()
                    .map_err(BindingWrapperError::WorkerV2Configuration)?;
                let attempt = begin_build_attempt(output_dir, &producer, invocation, session)
                    .map_err(BindingWrapperError::Artifact)?;
                (
                    attempt,
                    Some(ManagedWorkerV2::FreshV2 {
                        config: Box::new(config),
                        envelope_inputs,
                        resume,
                        compiler_closure,
                        producer_binding,
                    }),
                    true,
                )
            }
        } else {
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
                (
                    attempt,
                    Some(ManagedWorkerV2::RecoveryV1 {
                        resume,
                        state: Box::new(state),
                    }),
                    false,
                )
            } else {
                let envelope_inputs = config
                    .load_envelope_inputs()
                    .map_err(BindingWrapperError::WorkerV2Configuration)?;
                let attempt = begin_build_attempt(output_dir, &producer, invocation, session)
                    .map_err(BindingWrapperError::Artifact)?;
                (
                    attempt,
                    Some(ManagedWorkerV2::FreshV1 {
                        config: Box::new(config),
                        envelope_inputs,
                        resume,
                    }),
                    true,
                )
            }
        }
    } else {
        let attempt = begin_build_attempt(output_dir, &producer, invocation, session)
            .map_err(BindingWrapperError::Artifact)?;
        (attempt, None, true)
    };
    let mut begin_attempt_guard = began_attempt.then(|| ManagedAttemptRevocationGuard {
        output_dir: output_dir.to_path_buf(),
        producer: producer.clone(),
        attempt,
        armed: true,
    });
    let row_softmax_release = row_softmax_release
        .map(|(provider, workload)| {
            compiler_capabilities
                .row_softmax_authority_policy(attempt, provider)
                .map(|authority| RowSoftmaxReleaseContext {
                    authority,
                    workload: Some(workload),
                })
        })
        .transpose()?;
    let managed = ManagedAttempt {
        output_dir: output_dir.to_path_buf(),
        producer,
        attempt,
        protected_source_path,
        compile_environment_profile,
        worker_v2,
        row_softmax_release,
        row_softmax_provision,
        #[cfg(feature = "compiler-handoff-observation-test-only")]
        compiler_handoff_observation,
    };
    if let Some(guard) = begin_attempt_guard.as_mut() {
        guard.disarm();
    }
    Ok(managed)
}

fn complete_managed_attempt(
    mut managed: ManagedAttempt,
    parent_rustc_invocation_custody: Option<ParentRustcInvocationCustody>,
) -> Result<(), BindingWrapperError> {
    let completion = match parent_rustc_invocation_custody {
        Some(custody) => custody.retain_through(|custody| {
            custody.revalidate().map_err(|error| {
                CompletionFailure::Uncommitted(format!(
                    "parent protected rustc invocation custody failed before managed completion: {error}"
                ))
            })?;
            debug_assert!(!custody.grants_compiler_authority());
            complete_managed_attempt_inner(&mut managed, Some(custody))
        }),
        None => complete_managed_attempt_inner(&mut managed, None),
    };

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

fn complete_managed_attempt_inner(
    managed: &mut ManagedAttempt,
    parent_custody: Option<&ParentRustcInvocationCustody>,
) -> Result<(), CompletionFailure> {
    if simulation_mode_selected() {
        return complete_simulation_attempt(managed);
    }
    if managed.row_softmax_provision {
        return complete_row_softmax_v1_provision(managed);
    }
    if let Some(worker_v2) = managed.worker_v2.take() {
        return match worker_v2 {
            ManagedWorkerV2::InProcessGeneralGemm { config } => {
                debug_assert!(config.executes_worker_in_rustc());
                debug_assert!(config.general_gemm_v1().is_some());
                Err(CompletionFailure::Uncommitted(
                    "in-process general-GEMM qualification remains inert until rustc's private frontend correspondence and final join are connected"
                        .to_owned(),
                ))
            }
            ManagedWorkerV2::FreshV1 {
                config,
                envelope_inputs,
                resume,
            } => complete_fresh_worker_v2(managed, &config, envelope_inputs.as_ref(), &resume),
            ManagedWorkerV2::RecoveryV1 { resume, state } => {
                complete_recovered_worker_v2(managed, &resume, *state)
            }
            ManagedWorkerV2::FreshV2 {
                config,
                envelope_inputs,
                resume,
                compiler_closure,
                producer_binding,
            } => complete_fresh_protected_worker_v2(
                managed,
                &config,
                envelope_inputs.as_ref(),
                &resume,
                compiler_closure,
                &producer_binding,
            ),
            ManagedWorkerV2::RecoveryV2 {
                resume,
                state,
                compiler_closure,
                producer_binding,
            } => complete_recovered_protected_worker_v2(
                managed,
                &resume,
                *state,
                compiler_closure,
                &producer_binding,
            ),
            ManagedWorkerV2::FreshV3 {
                config,
                compiler_closure,
            } => complete_fresh_production_worker_v3(
                managed,
                &config,
                compiler_closure,
                parent_custody.ok_or_else(|| {
                    CompletionFailure::Uncommitted(
                        "production V3 completion lost exact parent rustc invocation custody"
                            .to_owned(),
                    )
                })?,
            ),
            ManagedWorkerV2::RecoveryV3 {
                recovered,
                compiler_closure,
            } => complete_recovered_production_worker_v3(managed, *recovered, compiler_closure),
            ManagedWorkerV2::RecoveryReadyV3 { envelope } => {
                complete_ready_production_worker_v3(managed, *envelope)
            }
        };
    }
    finish_build_attempt(&managed.output_dir, &managed.producer, managed.attempt).map_err(|error| {
        CompletionFailure::Uncommitted(format!("build-attempt completion failed: {error}"))
    })
}

fn simulation_mode_selected() -> bool {
    std::env::var_os(crate::SIMULATION_MODE_ENV).as_deref() == Some(OsStr::new("1"))
        && std::env::var_os("FE2O3_QUALIFICATION_ORACLE_V1").as_deref()
            == Some(OsStr::new("simulation-v1"))
        && std::env::var(crate::SIMULATION_ATTEMPT_ENV)
            .ok()
            .and_then(|attempt| BuildSession::from_hex(&attempt).ok())
            .is_some_and(|attempt| attempt != BuildSession::DIRECT)
}

fn complete_simulation_attempt(managed: &ManagedAttempt) -> Result<(), CompletionFailure> {
    if managed.worker_v2.is_some()
        || managed.row_softmax_provision
        || managed.row_softmax_release.is_some()
    {
        return Err(CompletionFailure::Uncommitted(
            "simulation-v1 cannot enter a Worker V2 or protected release completion path"
                .to_owned(),
        ));
    }
    let captured = match consume_simulation_kernel_ir_handoff_v1(
        &managed.output_dir,
        &managed.producer,
        managed.attempt,
    ) {
        Ok(captured) => Some(captured),
        Err(fe2o3_artifact_transaction::CompilerModuleHandoffErrorV1::NotPublished) => None,
        Err(fe2o3_artifact_transaction::CompilerModuleHandoffErrorV1::AttemptNotClaimable) => {
            // A host-only rustc invocation has an ordinary backend receipt, so
            // the simulation-only BackendClaimed/no-receipt slot rejects it.
            None
        }
        Err(error) => {
            return Err(CompletionFailure::Uncommitted(format!(
                "simulation-v1 could not consume its exact canonical KIR V7 handoff: {error}"
            )));
        }
    };
    if let Some(captured) = captured {
        crate::simulation_capture::publish(&managed.output_dir, managed.attempt, captured.bytes())
            .map_err(CompletionFailure::Uncommitted)?;
        complete_simulation_kernel_ir_attempt_v1(&managed.output_dir, &managed.producer, &captured)
            .map_err(|error| {
                CompletionFailure::Uncommitted(format!(
                    "simulation-v1 could not retire its exact KIR custody: {error}"
                ))
            })?;
        return Ok(());
    }
    finish_build_attempt(&managed.output_dir, &managed.producer, managed.attempt).map_err(|error| {
        CompletionFailure::Uncommitted(format!(
            "simulation-v1 build-attempt completion failed: {error}"
        ))
    })
}

fn row_softmax_provisioning_requested() -> bool {
    std::env::var_os(QUALIFICATION_RELEASE_ACTION_ENV).as_deref()
        == Some(OsStr::new(ROW_SOFTMAX_V1_PROVISION_VALUE))
}

fn row_softmax_provisioning_selected(compile: &RustcCompileInvocationV2<'_>) -> bool {
    compile.crate_name() == "fe2o3_collected_row_softmax_v1_fixture"
        && compile.source_path() == Path::new("src/lib.rs")
}

fn complete_row_softmax_v1_provision(managed: &ManagedAttempt) -> Result<(), CompletionFailure> {
    let consumed =
        consume_compiler_module_handoff_v1(&managed.output_dir, &managed.producer, managed.attempt)
            .map_err(|error| {
                CompletionFailure::Uncommitted(format!(
                    "stage=compiler-handoff: provider provisioning could not consume the exact handoff: {error}"
                ))
            })?;
    let handoff = CompilerModuleHandoffV2::decode(consumed.bytes()).map_err(|error| {
        CompletionFailure::Uncommitted(format!(
            "stage=compiler-handoff: provider provisioning could not decode the exact handoff: {error}"
        ))
    })?;
    let sections =
        decode_row_softmax_compiler_sections_v1(handoff.module_bytes()).map_err(|error| {
            CompletionFailure::Uncommitted(format!(
                "stage=compiler-handoff: provider provisioning rejected compiler sections: {error}"
            ))
        })?;
    if Sha256::digest(sections.authority_transcript()).as_slice() != sections.authority() {
        return Err(CompletionFailure::Uncommitted(
            "stage=compiler-handoff: provider provisioning rejected the frontend-authority commitment"
                .to_owned(),
        ));
    }
    let observation = row_softmax_provider_observation_json(sections.authority_transcript())
        .map_err(|error| {
            CompletionFailure::Uncommitted(format!(
                "stage=provider-provision: provider provisioning rejected the authority transcript: {error}"
            ))
        })?;
    eprintln!("{ROW_SOFTMAX_V1_PROVISION_PREFIX}{observation}");
    eprintln!(
        "cargo fe2o3: row-softmax provider observation is non-authoritative; no worker, artifact, runtime, or GPU authority was minted"
    );
    finish_build_attempt(&managed.output_dir, &managed.producer, managed.attempt).map_err(|error| {
        CompletionFailure::Uncommitted(format!(
            "stage=attempt-completion: provider provisioning could not finish the exact attempt: {error}"
        ))
    })
}

fn row_softmax_provider_observation_json(transcript: &[u8]) -> Result<String, String> {
    let fields = decode_framed_row_softmax_authority_fields(transcript)?;
    if fields.len() != 49 || fields[21] != b"fe2o3_device" {
        return Err("authority transcript field closure differs".to_owned());
    }
    let stable_crate_id = u64::from_le_bytes(
        fields[22]
            .try_into()
            .map_err(|_| "provider stable crate ID has the wrong width".to_owned())?,
    );
    let crate_hash: [u8; 16] = fields[23]
        .try_into()
        .map_err(|_| "provider crate hash has the wrong width".to_owned())?;
    let definition_identities: [[u8; 16]; ROW_SOFTMAX_V1_PROVIDER_ITEM_COUNT] = fields[26..34]
        .iter()
        .map(|field| {
            (*field)
                .try_into()
                .map_err(|_| "provider definition identity has the wrong width".to_owned())
        })
        .collect::<Result<Vec<_>, _>>()?
        .try_into()
        .map_err(|_| "provider definition identity count differs".to_owned())?;
    let source_identities: [[u8; 32]; ROW_SOFTMAX_V1_PROVIDER_ITEM_COUNT] = fields[34..42]
        .iter()
        .map(|field| {
            (*field)
                .try_into()
                .map_err(|_| "provider source identity has the wrong width".to_owned())
        })
        .collect::<Result<Vec<_>, _>>()?
        .try_into()
        .map_err(|_| "provider source identity count differs".to_owned())?;
    let expected_sources = [
        provider_source_identity("lib.rs", include_bytes!("../../fe2o3-device/src/lib.rs"))?,
        provider_source_identity(
            "thread.rs",
            include_bytes!("../../fe2o3-device/src/thread.rs"),
        )?,
        provider_source_identity("math.rs", include_bytes!("../../fe2o3-device/src/math.rs"))?,
    ];
    let expected_mapping = [
        expected_sources[0],
        expected_sources[1],
        expected_sources[1],
        expected_sources[1],
        expected_sources[0],
        expected_sources[2],
        expected_sources[2],
        expected_sources[2],
    ];
    if source_identities != expected_mapping {
        return Err("provider source identities differ from the reviewed source files".to_owned());
    }
    RowSoftmaxV1ProviderManifestV1::new(
        stable_crate_id,
        crate_hash,
        definition_identities,
        source_identities,
    )
    .map_err(|error| error.to_string())?;
    serde_json::to_string(&serde_json::json!({
        "provider_crate_hash": hex(&crate_hash),
        "provider_definition_identities": definition_identities.map(|identity| hex(&identity)),
        "provider_source_identities": source_identities.map(|identity| hex(&identity)),
        "provider_stable_crate_id": stable_crate_id,
    }))
    .map_err(|error| format!("cannot encode canonical provider observation: {error}"))
}

fn provider_source_identity(relative_path: &str, source: &[u8]) -> Result<[u8; 32], String> {
    derive_row_softmax_v1_provider_source_identity_v1(relative_path, source)
        .map_err(|error| error.to_string())
}

fn decode_framed_row_softmax_authority_fields(transcript: &[u8]) -> Result<Vec<&[u8]>, String> {
    if transcript.is_empty() || transcript.len() > 4096 {
        return Err("authority transcript length is invalid".to_owned());
    }
    let mut fields = Vec::new();
    let mut remaining = transcript;
    while !remaining.is_empty() {
        let length_bytes: [u8; 8] = remaining
            .get(..8)
            .ok_or_else(|| "authority transcript has a truncated field length".to_owned())?
            .try_into()
            .expect("checked field length");
        remaining = &remaining[8..];
        let length = usize::try_from(u64::from_le_bytes(length_bytes))
            .map_err(|_| "authority transcript field length exceeds usize".to_owned())?;
        let field = remaining
            .get(..length)
            .ok_or_else(|| "authority transcript has a truncated field".to_owned())?;
        fields.push(field);
        remaining = &remaining[length..];
    }
    Ok(fields)
}

fn complete_fresh_worker_v2(
    managed: &mut ManagedAttempt,
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
    if managed.row_softmax_release.is_some() {
        return complete_row_softmax_v1_release(managed, worker_v2, consumed);
    }
    let evidence = worker_v2.execute(consumed).map_err(|error| {
        CompletionFailure::Uncommitted(format!("reproducible Worker V2 execution failed: {error}"))
    })?;
    debug_assert_eq!(evidence.attempt(), managed.attempt);
    let canonical_request = evidence.authorized_request_bytes().to_vec();
    let canonical_response = evidence.authorized().response().canonical_bytes().to_vec();
    let raw_output = evidence.output_bytes().to_vec();
    let worker_v2_request_identity = *evidence.authorized_request_identity();
    let inspected = if worker_v2.is_production_compilation() {
        inspect_production_v1_worker_v2_raw_hsaco_v1(evidence)
    } else {
        inspect_worker_v2_raw_hsaco_v1(evidence)
    }
    .map_err(|error| {
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

fn complete_fresh_production_worker_v3(
    managed: &ManagedAttempt,
    worker: &PreparedWorkerV2Config,
    compiler_closure: CompilerClosureV2,
    parent_custody: &ParentRustcInvocationCustody,
) -> Result<(), CompletionFailure> {
    if !worker.is_production_compilation() {
        return Err(CompletionFailure::Uncommitted(
            "strict V3 compiler intake accepts only the preselected production-v1 worker"
                .to_owned(),
        ));
    }
    let intake = ProtectedCompilerModuleHandoffIntake::protected_v3();
    let (consumed, preflight) = intake
        .consume_v3_after_preflight(
            &managed.output_dir,
            &managed.producer,
            managed.attempt,
            parent_custody,
            |handoff, receipt, observed_closure| {
                if observed_closure != compiler_closure {
                    return Err(
                        fe2o3_hsaco_finalize::ProtectedFirstBuildWorkerV3Error::ReplayValidation {
                            field: "Cargo compiler closure changed before V3 worker preflight",
                        },
                    );
                }
                worker.preflight_protected_v3(handoff, receipt, observed_closure)
            },
        )
        .map_err(|error| {
            CompletionFailure::Uncommitted(format!(
                "strict V3 compiler-module preflight/consumption failed: {error}"
            ))
        })?;
    let evidence = worker
        .execute_preflighted_protected_v3(consumed, preflight)
        .map_err(|error| {
            CompletionFailure::Uncommitted(format!(
                "strict V3 reproducible worker execution failed: {error}"
            ))
        })?;
    let inspected =
        inspect_protected_production_v1_worker_v3_raw_hsaco_v1(evidence).map_err(|error| {
            CompletionFailure::Uncommitted(format!(
                "independent strict V3 raw-HSACO inspection failed: {error}"
            ))
        })?;
    let finalized =
        finalize_inspected_protected_worker_v3_hsaco_v1(inspected).map_err(|error| {
            CompletionFailure::Uncommitted(format!(
                "strict V3 canonical HSACO finalization failed: {error}"
            ))
        })?;
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
    complete_recovered_production_worker_v3(managed, recovered, compiler_closure)
}

fn complete_recovered_production_worker_v3(
    managed: &ManagedAttempt,
    recovered: RecoveredProtectedWorkerV3HsacoPublicationV1,
    compiler_closure: CompilerClosureV2,
) -> Result<(), CompletionFailure> {
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
    complete_published_production_worker_v3(managed, published)
}

fn complete_published_production_worker_v3(
    managed: &ManagedAttempt,
    published: PublishedProtectedWorkerV3HsacoV1,
) -> Result<(), CompletionFailure> {
    let intent_identity = published.recovered_evidence().storage_record().identity();
    let envelope = WorkerV3LoadEnvelopeV1::from_published_hsaco_v1(published).map_err(|error| {
        CompletionFailure::PreserveAttempt(format!(
            "strict V3 load-envelope custody construction failed: {error}"
        ))
    })?;
    let readiness = envelope
        .persist_durable_replay_custody_v1(&managed.output_dir)
        .map_err(|error| {
            CompletionFailure::PreserveAttempt(format!(
                "strict V3 load-envelope custody persistence failed: {error}"
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

fn complete_ready_production_worker_v3(
    managed: &ManagedAttempt,
    envelope: RecoveredWorkerV3LoadEnvelopeV1,
) -> Result<(), CompletionFailure> {
    let intent_identity = envelope.wire().publication_intent_record().identity();
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

fn complete_fresh_protected_worker_v2(
    managed: &ManagedAttempt,
    worker_v2: &PreparedWorkerV2Config,
    envelope_inputs: Option<&WorkerV2EnvelopeInputsV1>,
    resume: &WorkerV2ResumeStoreV2,
    compiler_closure: CompilerClosureV2,
    producer_binding: &WorkerV2ProducerBindingV2,
) -> Result<(), CompletionFailure> {
    if managed.row_softmax_release.is_some() {
        return Err(CompletionFailure::Uncommitted(
            "protected row-softmax requires the V2 load-envelope and runtime transition; V1 direct execution fallback is forbidden"
                .to_owned(),
        ));
    }
    if worker_v2.envelope_mode().is_required() != envelope_inputs.is_some() {
        return Err(CompletionFailure::Uncommitted(
            "protected Worker V2 envelope mode disagrees with its exact input capsule".to_owned(),
        ));
    }

    let intake = ProtectedCompilerModuleHandoffIntake::protected_v2(compiler_closure);
    let consumed = intake
        .consume_v2(&managed.output_dir, &managed.producer, managed.attempt)
        .map_err(|error| {
            CompletionFailure::Uncommitted(format!(
                "protected compiler-module V2 handoff consumption failed: {error}"
            ))
        })?;
    let evidence = worker_v2.execute_protected(consumed).map_err(|error| {
        CompletionFailure::Uncommitted(format!(
            "closure-bound reproducible Worker V2 execution failed: {error}"
        ))
    })?;
    let inspected = if worker_v2.is_production_compilation() {
        inspect_protected_production_v1_worker_v2_raw_hsaco_v1(evidence)
    } else {
        inspect_protected_worker_v2_raw_hsaco_v1(evidence)
    }
    .map_err(|error| {
        CompletionFailure::Uncommitted(format!(
            "independent protected Worker V2 raw-HSACO inspection failed: {error}"
        ))
    })?;
    if inspected.attempt() != managed.attempt || inspected.compiler_closure() != compiler_closure {
        return Err(CompletionFailure::Uncommitted(
            "protected Worker V2 inspection changed its exact attempt or compiler closure"
                .to_owned(),
        ));
    }

    let persisted = match (
        inspected.code_object_version(),
        inspected.canonical_descriptor_section(),
    ) {
        (CodeObjectVersion::V5, CanonicalDescriptorSectionObservationV1::Missing) => {
            if worker_v2.envelope_mode().is_required() {
                return Err(CompletionFailure::Uncommitted(
                    "protected required-envelope publication accepts only canonical COV6 output"
                        .to_owned(),
                ));
            }
            let prepared =
                prepare_protected_worker_v2_hsaco_publication_v2(&managed.producer, inspected)
                    .map_err(|error| {
                        CompletionFailure::Uncommitted(format!(
                            "protected raw Worker V2 publication preparation failed: {error}"
                        ))
                    })?;
            let sealed = prepared.publication_intent();
            if prepared.attempt() != managed.attempt
                || prepared.compiler_closure() != compiler_closure
                || sealed.compiler_closure() != compiler_closure
                || !sealed.matches_exact_retained_output(prepared.exact_retained_output())
            {
                return Err(CompletionFailure::Uncommitted(
                    "protected raw Worker V2 publication preparation changed exact lineage"
                        .to_owned(),
                ));
            }
            persist_admitted_worker_v2_intent_v2(
                resume,
                &managed.producer,
                WorkerV2PublicationKindV1::Raw,
                sealed.durable_plan(),
                sealed.upstream_evidence(),
                prepared.exact_retained_output(),
                envelope_inputs,
                compiler_closure,
            )
            .map_err(|error| preserve_protected_restart_error("persistence", error))?
        }
        (
            CodeObjectVersion::V6,
            CanonicalDescriptorSectionObservationV1::PresentButNotFinalizedByThisInspection,
        ) => {
            let finalized =
                finalize_inspected_protected_worker_v2_hsaco_v2(inspected).map_err(|error| {
                    CompletionFailure::Uncommitted(format!(
                        "protected Worker V2 canonical HSACO finalization failed: {error}"
                    ))
                })?;
            let prepared = prepare_finalized_protected_worker_v2_hsaco_publication_v2(
                &managed.producer,
                finalized,
            )
            .map_err(|error| {
                CompletionFailure::Uncommitted(format!(
                    "protected finalized Worker V2 publication preparation failed: {error}"
                ))
            })?;
            let sealed = prepared.publication_intent();
            if prepared.attempt() != managed.attempt
                || prepared.compiler_closure() != compiler_closure
                || sealed.compiler_closure() != compiler_closure
                || !sealed.matches_exact_retained_output(prepared.exact_retained_output())
            {
                return Err(CompletionFailure::Uncommitted(
                    "protected finalized Worker V2 publication preparation changed exact lineage"
                        .to_owned(),
                ));
            }
            let (publication, plan, upstream) = if let Some(inputs) = envelope_inputs {
                if !sealed
                    .raw_linked_snapshot_identity()
                    .matches(inputs.raw_hsaco().bytes())
                {
                    return Err(CompletionFailure::Uncommitted(
                        "protected required-envelope capsule changed the exact raw HSACO lineage"
                            .to_owned(),
                    ));
                }
                let (plan, upstream) = derive_required_worker_v2_publication_plan_v1(
                    &managed.producer,
                    managed.attempt,
                    prepared.exact_retained_output(),
                    inputs,
                )
                .map_err(|error| {
                    CompletionFailure::Uncommitted(format!(
                        "protected required-envelope publication plan derivation failed: {error}"
                    ))
                })?;
                (
                    WorkerV2PublicationKindV1::FinalizedEnvelopeRequired,
                    plan,
                    upstream,
                )
            } else {
                (
                    WorkerV2PublicationKindV1::Finalized,
                    sealed.durable_plan(),
                    sealed.upstream_evidence(),
                )
            };
            persist_admitted_worker_v2_intent_v2(
                resume,
                &managed.producer,
                publication,
                plan,
                upstream,
                prepared.exact_retained_output(),
                envelope_inputs,
                compiler_closure,
            )
            .map_err(|error| preserve_protected_restart_error("persistence", error))?
        }
        (code_object_version, descriptor) => {
            return Err(CompletionFailure::Uncommitted(format!(
                "protected Worker V2 publication rejects {code_object_version:?} with descriptor observation {descriptor:?}; no V1 publication fallback is permitted"
            )));
        }
    };

    publish_finish_and_clear_protected(
        managed,
        resume,
        persisted.publication,
        persisted.intent,
        compiler_closure,
        producer_binding,
    )
}

fn complete_row_softmax_v1_release(
    managed: &mut ManagedAttempt,
    worker_v2: &PreparedWorkerV2Config,
    consumed: fe2o3_artifact_transaction::ConsumedCompilerModuleHandoffV1,
) -> Result<(), CompletionFailure> {
    let release = managed
        .row_softmax_release
        .as_mut()
        .expect("row-softmax completion has release context");
    let workload = release
        .workload
        .take()
        .expect("row-softmax workload is consumed exactly once");
    let handoff = CompilerModuleHandoffV2::decode(consumed.bytes()).map_err(|error| {
        CompletionFailure::Uncommitted(format!(
            "stage=compiler-handoff: row-softmax handoff decode failed: {error}"
        ))
    })?;
    let worker_pins = worker_v2.row_softmax_v1_worker_pins().map_err(|error| {
        CompletionFailure::Uncommitted(format!(
            "stage=worker-policy: row-softmax worker pin contract failed: {error}"
        ))
    })?;
    let frontend_authority = *decode_row_softmax_compiler_sections_v1(handoff.module_bytes())
        .map_err(|error| {
            CompletionFailure::Uncommitted(format!(
                "stage=compiler-handoff: row-softmax authority sections failed: {error}"
            ))
        })?
        .authority();
    let expectation = RowSoftmaxV1DirectWorkerExpectationV1::from_pinned_rustc_handoff(
        &handoff,
        *handoff.identity().sha256(),
        frontend_authority,
        release.authority,
        worker_pins,
    )
    .map_err(|error| {
        CompletionFailure::Uncommitted(format!(
            "stage=authority-policy: row-softmax compiler/provider policy rejected before worker execution: {error}"
        ))
    })?;
    let evidence = worker_v2.execute(consumed).map_err(|error| {
        CompletionFailure::Uncommitted(format!(
            "stage=llvm-finalizer: reproducible direct Worker V2 execution failed: {error}"
        ))
    })?;
    let token =
        admit_row_softmax_v1_source_tested_artifact_v1(evidence, expectation).map_err(|error| {
            CompletionFailure::Uncommitted(format!(
                "stage=artifact-admission: row-softmax artifact was rejected before launch: {error}"
            ))
        })?;
    let receipt =
        execute_row_softmax_v1_production_workload_v1(token, workload).map_err(|error| {
            CompletionFailure::Uncommitted(format!(
                "stage=typed-launch: row-softmax production execution failed: {error}"
            ))
        })?;
    if receipt.unload_identity() == &[0; 32]
        || receipt.proves_masked_execution()
        || receipt.proves_verus_refinement()
    {
        return Err(CompletionFailure::Uncommitted(
            "stage=terminal-receipt: row-softmax receipt is empty or overclaims authority"
                .to_owned(),
        ));
    }
    eprintln!(
        "FE2O3_PROTECTED_ROW_SOFTMAX_V1_OK case={:?} width=64 mask=unmasked target=gfx942:xnack- pins=25 source_tested=true verus_refinement=false unload={}",
        receipt.case(),
        hex(receipt.unload_identity())
    );
    finish_build_attempt(&managed.output_dir, &managed.producer, managed.attempt).map_err(|error| {
        CompletionFailure::Uncommitted(format!(
            "stage=attempt-completion: row-softmax build-attempt completion failed: {error}"
        ))
    })
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

fn complete_recovered_protected_worker_v2(
    managed: &ManagedAttempt,
    resume: &WorkerV2ResumeStoreV2,
    state: ResumeMarkerStateV2,
    compiler_closure: CompilerClosureV2,
    producer_binding: &WorkerV2ProducerBindingV2,
) -> Result<(), CompletionFailure> {
    if matches!(state, ResumeMarkerStateV2::Completed { .. }) {
        return reconcile_completed_protected_worker_v2(
            managed,
            resume,
            state,
            compiler_closure,
            producer_binding,
        );
    }
    let intent = match recover_worker_v2_intent_v2(
        resume,
        &managed.producer,
        state,
        compiler_closure,
    ) {
        Ok(intent) => intent,
        Err(RestartIntentErrorV2::Intent(WorkerV2PublicationIntentErrorV2::NotFound))
            if matches!(state, ResumeMarkerStateV2::Pending { .. }) =>
        {
            resume.clear_abandoned_pending(state).map_err(|error| {
                preserve_marker_error("protected abandoned-pending cleanup", error)
            })?;
            return Err(CompletionFailure::Uncommitted(
                "protected Worker V2 process stopped before its V2 publication intent became durable"
                    .to_owned(),
            ));
        }
        Err(error) => {
            return Err(preserve_protected_restart_error("recovery", error));
        }
    };
    publish_finish_and_clear_protected(
        managed,
        resume,
        state.publication(),
        intent,
        compiler_closure,
        producer_binding,
    )
}

fn publish_finish_and_clear_protected(
    managed: &ManagedAttempt,
    resume: &WorkerV2ResumeStoreV2,
    publication: WorkerV2PublicationKindV1,
    intent: RecoveredWorkerV2PublicationIntentV2,
    compiler_closure: CompilerClosureV2,
    producer_binding: &WorkerV2ProducerBindingV2,
) -> Result<(), CompletionFailure> {
    let record = intent.record();
    if record.compiler_closure() != compiler_closure
        || intent.compiler_closure() != compiler_closure
    {
        return Err(CompletionFailure::PreserveAttempt(
            "protected Worker V2 intent changed its exact compiler closure".to_owned(),
        ));
    }
    let intent_identity = record.identity();
    let receipt = publish_recovered_protected_worker_v2(managed, &intent, compiler_closure)?;
    let claim = recover_and_validate_protected_claim(managed, record, receipt, compiler_closure)?;
    #[cfg(feature = "worker-v2-fault-injection-test-only")]
    injected_fault_point_v1("protected-published");
    let completed = if publication.requires_envelope() {
        let inputs = resume
            .recover_envelope_inputs(managed.attempt)
            .map_err(|error| {
                preserve_marker_error("protected required envelope input recovery", error)
            })?;
        let prepared = assemble_recovered_worker_v2_load_envelope_v2(
            &managed.producer,
            record.plan(),
            record.upstream_evidence(),
            intent.exact_output(),
            claim,
            &inputs,
            record,
            producer_binding.clone(),
            receipt,
            compiler_closure,
        )
        .map_err(|error| {
            CompletionFailure::PreserveAttempt(format!(
                "protected Worker V2 canonical V2 envelope assembly failed: {error}"
            ))
        })?;
        if prepared.compiler_closure() != compiler_closure
            || prepared.publication_intent_identity() != intent_identity
            || prepared.backend_receipt() != receipt
            || prepared.grants_compiler_authority()
            || prepared.grants_proof_authority()
            || prepared.grants_publication_authority()
            || prepared.grants_load_authority()
            || prepared.grants_launch_authority()
        {
            return Err(CompletionFailure::PreserveAttempt(
                "protected Worker V2 envelope assembly changed exact lineage or overclaimed authority"
                    .to_owned(),
            ));
        }
        resume.persist_envelope_and_completed(
            publication,
            managed.attempt,
            intent_identity,
            receipt,
            compiler_closure,
            prepared.envelope(),
        )
    } else {
        resume.persist_completed(
            publication,
            managed.attempt,
            intent_identity,
            receipt,
            compiler_closure,
        )
    }
    .map_err(|error| preserve_marker_error("protected completion persistence", error))?;
    #[cfg(feature = "worker-v2-fault-injection-test-only")]
    injected_fault_point_v1("protected-completed");
    if !publication.requires_envelope() {
        clear_worker_v2_publication_intent_v2(
            &managed.output_dir,
            &managed.producer,
            managed.attempt,
            compiler_closure,
            intent_identity,
        )
        .map_err(|error| preserve_protected_intent_error("cleanup", error))?;
        #[cfg(feature = "worker-v2-fault-injection-test-only")]
        injected_fault_point_v1("protected-intent-cleared");
    }
    finish_worker_v2_attempt(managed)?;
    #[cfg(feature = "worker-v2-fault-injection-test-only")]
    injected_fault_point_v1("protected-finished");
    resume
        .clear_completed_and_envelope_inputs(completed, receipt, compiler_closure)
        .map_err(|error| preserve_marker_error("protected cleanup", error))
}

fn reconcile_completed_protected_worker_v2(
    managed: &ManagedAttempt,
    resume: &WorkerV2ResumeStoreV2,
    completed: ResumeMarkerStateV2,
    compiler_closure: CompilerClosureV2,
    producer_binding: &WorkerV2ProducerBindingV2,
) -> Result<(), CompletionFailure> {
    let ResumeMarkerStateV2::Completed {
        attempt,
        intent: intent_identity,
        receipt: expected_receipt,
        ..
    } = completed
    else {
        return Err(CompletionFailure::PreserveAttempt(
            "protected Worker V2 completion reconciliation received a non-completed marker"
                .to_owned(),
        ));
    };
    debug_assert_eq!(attempt, managed.attempt);
    let receipt = read_backend_publication_receipt_v2(
        &managed.output_dir,
        &managed.producer,
        managed.attempt,
    )
    .map_err(|error| {
        CompletionFailure::PreserveAttempt(format!(
            "protected Worker V2 completed-recovery V2 receipt inspection failed: {error}"
        ))
    })?;
    let PersistedBackendReceiptV2::Provenance(receipt) = receipt else {
        return Err(CompletionFailure::PreserveAttempt(
            "protected Worker V2 completed marker has no exact durable V2 publication receipt"
                .to_owned(),
        ));
    };
    if receipt.compiler_closure() != compiler_closure || !expected_receipt.matches(receipt) {
        return Err(CompletionFailure::PreserveAttempt(
            "protected Worker V2 completed marker receipt or compiler closure was substituted"
                .to_owned(),
        ));
    }
    if completed.publication().requires_envelope() {
        validate_completed_protected_worker_v2_envelope(
            managed,
            resume,
            completed,
            receipt,
            compiler_closure,
            producer_binding,
        )?;
    }
    match recover_worker_v2_intent_v2(resume, &managed.producer, completed, compiler_closure) {
        Ok(intent) => {
            let record = intent.record();
            if record.identity() != intent_identity || record.compiler_closure() != compiler_closure
            {
                return Err(CompletionFailure::PreserveAttempt(
                    "protected Worker V2 completed marker disagrees with its exact V2 journal authority"
                        .to_owned(),
                ));
            }
            recover_and_validate_protected_claim(managed, record, receipt, compiler_closure)?;
            if !completed.publication().requires_envelope() {
                clear_worker_v2_publication_intent_v2(
                    &managed.output_dir,
                    &managed.producer,
                    managed.attempt,
                    compiler_closure,
                    intent_identity,
                )
                .map_err(|error| {
                    preserve_protected_intent_error("completed recovery authorization", error)
                })?;
            }
        }
        Err(RestartIntentErrorV2::Intent(WorkerV2PublicationIntentErrorV2::NotFound)) => {}
        Err(error) => {
            return Err(preserve_protected_restart_error(
                "completed recovery validation",
                error,
            ));
        }
    }
    finish_worker_v2_attempt(managed)?;
    resume
        .clear_completed_and_envelope_inputs(completed, receipt, compiler_closure)
        .map_err(|error| preserve_marker_error("protected completed recovery cleanup", error))
}

fn validate_completed_protected_worker_v2_envelope(
    managed: &ManagedAttempt,
    resume: &WorkerV2ResumeStoreV2,
    completed: ResumeMarkerStateV2,
    receipt: BackendPublicationReceiptV2,
    compiler_closure: CompilerClosureV2,
    producer_binding: &WorkerV2ProducerBindingV2,
) -> Result<(), CompletionFailure> {
    let envelope = resume
        .recover_load_envelope(receipt, compiler_closure)
        .map_err(|error| {
            CompletionFailure::PreserveAttempt(format!(
                "protected Worker V2 completed-recovery envelope inspection failed: {error}"
            ))
        })?;
    let inputs = resume
        .recover_envelope_inputs(managed.attempt)
        .map_err(|error| {
            CompletionFailure::PreserveAttempt(format!(
                "protected Worker V2 completed-recovery capsule inspection failed: {error}"
            ))
        })?;
    let carried_inputs = WorkerV2EnvelopeInputsV1::new(
        envelope.direct_link_evidence().clone(),
        envelope.proof_records().to_vec(),
        envelope.raw_hsaco().clone(),
    )
    .map_err(|error| {
        CompletionFailure::PreserveAttempt(format!(
            "protected Worker V2 completed-recovery envelope inputs are invalid: {error}"
        ))
    })?;
    let evidence = envelope.final_artifact_evidence();
    let intent = evidence.publication_intent_transcript();
    let claim = evidence.published_claim();
    if completed.attempt() != managed.attempt
        || completed.intent() != Some(intent.source_record_identity())
        || completed.envelope_inputs() != inputs.identity().as_bytes()
        || completed.envelope() != envelope.identity().as_bytes()
        || carried_inputs != inputs
        || intent.attempt() != managed.attempt
        || intent.producer_binding() != producer_binding
        || intent.compiler_closure() != compiler_closure
        || evidence.compiler_closure() != compiler_closure
        || evidence.backend_receipt() != receipt
        || claim.receipt() != receipt
        || claim.plan().attempt() != managed.attempt
        || claim.compiler_closure() != compiler_closure
        || envelope.grants_compiler_authority()
        || envelope.grants_proof_authority()
        || envelope.grants_currentness_authority()
        || envelope.grants_load_authority()
        || envelope.grants_launch_authority()
    {
        return Err(CompletionFailure::PreserveAttempt(
            "protected Worker V2 completed marker disagrees with its canonical inert V2 envelope"
                .to_owned(),
        ));
    }
    Ok(())
}

fn recover_and_validate_protected_claim(
    managed: &ManagedAttempt,
    record: WorkerV2PublicationIntentRecordV2,
    receipt: BackendPublicationReceiptV2,
    compiler_closure: CompilerClosureV2,
) -> Result<DurablePublishedHsacoClaimV2, CompletionFailure> {
    let claim = recover_published_hsaco_claim_for_attempt_v2(
        &managed.output_dir,
        &managed.producer,
        managed.attempt,
        record.plan(),
        record.upstream_evidence(),
        compiler_closure,
        receipt,
    )
    .map_err(|error| {
        CompletionFailure::PreserveAttempt(format!(
            "protected Worker V2 V2 published-claim recovery failed: {error}"
        ))
    })?;
    if claim.plan() != record.plan()
        || claim.upstream_evidence() != record.upstream_evidence()
        || claim.receipt() != receipt
        || claim.compiler_closure() != compiler_closure
        || claim.grants_compiler_authority()
        || claim.grants_proof_authority()
        || claim.grants_publication_authority()
        || claim.grants_load_authority()
        || claim.grants_launch_authority()
    {
        return Err(CompletionFailure::PreserveAttempt(
            "protected Worker V2 recovered V2 claim changed lineage or overclaimed authority"
                .to_owned(),
        ));
    }
    Ok(claim)
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

fn publish_recovered_protected_worker_v2(
    managed: &ManagedAttempt,
    intent: &RecoveredWorkerV2PublicationIntentV2,
    compiler_closure: CompilerClosureV2,
) -> Result<BackendPublicationReceiptV2, CompletionFailure> {
    const MAX_EXACT_RECONCILIATION_ATTEMPTS: usize = 3;
    let record = intent.record();
    if record.compiler_closure() != compiler_closure {
        return Err(CompletionFailure::PreserveAttempt(
            "protected Worker V2 publication intent has a different compiler closure".to_owned(),
        ));
    }
    for attempt in 1..=MAX_EXACT_RECONCILIATION_ATTEMPTS {
        match publish_exact_hsaco_evidence_for_attempt_v2(
            &managed.output_dir,
            &managed.producer,
            managed.attempt,
            record.plan(),
            record.upstream_evidence(),
            compiler_closure,
            intent.exact_output(),
        ) {
            Ok(published) => {
                if published.compiler_closure() != compiler_closure {
                    return Err(CompletionFailure::PreserveAttempt(
                        "protected Worker V2 publisher changed the exact compiler closure"
                            .to_owned(),
                    ));
                }
                return Ok(published.receipt());
            }
            Err(AttemptScopedHsacoPublicationErrorV2::ReceiptAlreadyPersisted { receipt }) => {
                if receipt.compiler_closure() != compiler_closure {
                    return Err(CompletionFailure::PreserveAttempt(
                        "protected Worker V2 recovered receipt has a different compiler closure"
                            .to_owned(),
                    ));
                }
                return Ok(*receipt);
            }
            Err(
                AttemptScopedHsacoPublicationErrorV2::PublicationInterrupted(_)
                | AttemptScopedHsacoPublicationErrorV2::PublicationCommittedWithoutReceipt { .. },
            ) if attempt < MAX_EXACT_RECONCILIATION_ATTEMPTS => {}
            Err(error) => {
                return Err(CompletionFailure::PreserveAttempt(format!(
                    "protected Worker V2 V2 journal publication failed after {attempt} attempts: {error}"
                )));
            }
        }
    }
    unreachable!("protected publication retry loop always returns")
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

fn preserve_protected_restart_error(
    context: &str,
    error: RestartIntentErrorV2,
) -> CompletionFailure {
    CompletionFailure::PreserveAttempt(format!(
        "protected Worker V2 V2 publication-intent {context} failed: {error}"
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

fn preserve_protected_intent_error(
    context: &str,
    error: WorkerV2PublicationIntentErrorV2,
) -> CompletionFailure {
    CompletionFailure::PreserveAttempt(format!(
        "protected Worker V2 V2 publication-intent {context} failed: {error}"
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
    compiler_closure_sha256: [u8; 32],
) -> BuildInvocation {
    derive_build_attempt_input_with_config_identity(
        argv,
        worker_v2.map(PreparedWorkerV2Config::identity),
        current_dir,
        compiler_closure_sha256,
    )
}

fn derive_build_attempt_input_with_config_identity(
    argv: &[OsString],
    worker_v2_identity: Option<WorkerV2ConfigIdentity>,
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
    if let Some(worker_v2_identity) = worker_v2_identity {
        digest.update(WORKER_V2_CONFIG_ID_DOMAIN);
        digest.update(worker_v2_identity.as_bytes());
    }
    BuildInvocation::from_bytes(digest.finalize().into())
        .bind_compiler_closure_v1(compiler_closure_sha256)
}

fn hash_bytes(digest: &mut Sha256, bytes: &[u8]) {
    digest.update((bytes.len() as u64).to_le_bytes());
    digest.update(bytes);
}

fn row_softmax_effective_rustc_argv_identity(
    argv: &[OsString],
    compiler_closure_sha256: [u8; 32],
) -> BuildInvocation {
    let mut digest = Sha256::new();
    digest.update(ROW_SOFTMAX_EFFECTIVE_RUSTC_ARGV_DOMAIN_V1);
    digest.update((argv.len() as u64).to_le_bytes());
    for argument in argv {
        hash_bytes(&mut digest, os_bytes(argument));
    }
    BuildInvocation::from_bytes(digest.finalize().into())
        .bind_compiler_closure_v1(compiler_closure_sha256)
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
#[allow(clippy::duplicate_mod, dead_code)]
#[path = "worker_v2_artifact_container_test_fixture.rs"]
mod binding_wrapper_test_fixture;

#[cfg(test)]
mod tests {
    use super::binding_wrapper_test_fixture::{ProfileMutation, alpha_zeta_fixture};
    use super::{
        BindingWrapperError, BuildExecutableSnapshot,
        CARGO_FE2O3_EXECUTABLE_BUILD_OBSERVATION_ENV_V2, CARGO_METADATA_BUILD_OBSERVATION_ENV_V2,
        CARGO_METADATA_MUTATION_TEST_ONLY_ENV_V1, CODEGEN_BACKEND_BUILD_OBSERVATION_ENV_V2,
        CompileBuildObservationV2, CompilerCapabilities, CompleteReviewedChildEnvironmentV2,
        CompletionFailure, DECLARED_CARGO_EXECUTABLE_BUILD_OBSERVATION_ENV_V2,
        GeneralGemmChildPinsV1, LLVM_BUILD_IDENTITY_OBSERVATION_ENV_V2, LinuxObjectIdentityV3,
        ManagedAttempt, ManagedAttemptRevocationGuard,
        OBSERVED_PARENT_PID_BUILD_OBSERVATION_ENV_V2,
        OBSERVED_PARENT_START_TIME_BUILD_OBSERVATION_ENV_V2, OBSOLETE_PRODUCTION_SELECTOR,
        PINNED_CARGO_IMAGE_BUILD_OBSERVATION_ENV_V2, PreparedRustcConsistencyExpectation,
        ProtectedWorkerV2TransitionBlocker, QUALIFICATION_CODEGEN_BACKEND_SHA256_ENV_V1,
        QUALIFICATION_RELEASE_ACTION_ENV, ROW_SOFTMAX_EFFECTIVE_RUSTC_ARGV_DOMAIN_V1,
        ROW_SOFTMAX_V1_PIPELINE, ROW_SOFTMAX_V1_RUN_VALUE, RustcCodegenMetadataErrorV1,
        RustcInvocationV2, WORKER_BUILD_IDENTITY_OBSERVATION_ENV_V2,
        WORKER_CONFIG_BUILD_OBSERVATION_ENV_V2, WORKER_EXECUTABLE_BUILD_OBSERVATION_ENV_V2,
        WorkerV2BindingSchema, append_prepared_rustc_arguments, canonicalize_rustc_metadata,
        classify_rustc_invocation_v2, complete_recovered_protected_worker_v2,
        complete_recovered_worker_v2, configure_build_observation_environment,
        configure_build_observation_environment_with_test_mutation,
        configure_qualification_route_marker, configure_worker_build_observation_environment,
        decode_managed_rustc_args, derive_build_attempt_input_with_config_identity, hex,
        is_cargo_stdin_probe, materialize_closed_child_environment,
        materialize_general_gemm_v1_child_environment, materialize_reviewed_child_environment,
        materialize_row_softmax_v1_child_environment, materialize_scalar_gemm_v1_child_environment,
        measure_build_executable, observe_pinned_cargo_image_and_parent,
        ordered_rustc_codegen_metadata_v1, os_bytes, pre_spawn_failure, prepare_managed_attempt,
        prepared_rustc_command_sha256, process_start_time_ticks,
        protected_worker_v2_transition_blocker, publish_finish_and_clear,
        publish_finish_and_clear_protected, qualification_requires_compiler_closure_observation,
        qualification_selection_requires_protected_invocation, reject_authority_linker_arguments,
        reject_uninspectable_rustc_args, resolve_command_executable_with_path,
        row_softmax_effective_rustc_argv_identity, row_softmax_provider_observation_json,
        scope_unmanaged_dependency_environment, selected_kernel_root,
        worker_v3_readiness_is_absent,
    };
    use crate::inert_rustc_invocation_capture::InertRustcInvocationCaptureV2;
    use crate::pinned_codegen_backend::PinnedCodegenBackend;
    use crate::pinned_executable::PinnedExecutable;
    use crate::project::PinnedDirectory;
    use crate::worker_v2::{
        GENERAL_GEMM_RUNTIME_CLOSURE_V2_MANIFEST_SHA256_ENV,
        GENERAL_GEMM_RUNTIME_CLOSURE_V2_ROOT_ENV, PreparedWorkerV2Config, WorkerV2BuildObservation,
        WorkerV2CompileEnvironmentProfileV1, WorkerV2ConfigIdentity,
    };
    use crate::worker_v2_artifact_container::{
        assemble_recovered_worker_v2_load_envelope_v2,
        canonical_worker_v2_container_for_fixture_v1,
        derive_required_worker_v2_publication_plan_v1,
    };
    use crate::worker_v2_restart::{
        ResumeMarkerStateV2, WorkerV2PublicationKindV1, WorkerV2ResumeStoreV1,
        WorkerV2ResumeStoreV2, persist_admitted_worker_v2_intent_v2,
        restart_admission_commitment_with_inputs_v1,
    };
    use fe2o3_artifact_transaction::{
        AtomicPublicationIdentityV1, BackendPublicationReceiptV2, BuildInvocation, BuildSession,
        CanonicalLinkRequestIdentityV1, DurableLinkPublicationPlanV1, FinalizationIdentityV1,
        FinalizedOutputIdentityV1, KernelSetIdentityV1, LinkPublicationScopeV1,
        LinkedOutputIdentityV1, PackageIdentityV1, PersistedBackendReceiptV1,
        PersistedBackendReceiptV2, PinnedWorkerIdentityV1, ProducerIdentity,
        RecoveredWorkerV2PublicationIntentV2, TargetIdentityV1,
        UpstreamCodeObjectEvidenceIdentityV1, ValidatedResponseIdentityV1, begin_build_attempt,
        finish_build_attempt, persist_worker_v2_publication_intent_v1,
        persist_worker_v2_publication_intent_v2, publish_compiler_module_handoff_v1,
        publish_compiler_module_handoff_v2, publish_exact_hsaco_evidence_for_attempt_v1,
        publish_exact_hsaco_evidence_for_attempt_v2, read_backend_publication_receipt_v1,
        read_backend_publication_receipt_v2, recover_worker_v2_publication_intent_v1,
        recover_worker_v2_publication_intent_v2,
    };
    use fe2o3_artifacts::{
        BundleIndexV1, DigestAlgorithm, DigestBytes, DirectLinkBindingExpectationV1,
        DirectLinkBindingSourceV1, DirectLinkBundleEvidenceV1, DirectLinkFfiClosureIdentityV1,
        DirectLinkFinalizationIdentityV1, DirectLinkFinalizedPayloadIdentityV1,
        DirectLinkLinkedOutputIdentityV1, DirectLinkRequestIdentityV1,
        DirectLinkResponseIdentityV1, DirectLinkToolchainConfigurationIdentityV1,
        DirectLinkToolchainExecutableIdentityV1, DirectLinkToolchainIdentityV1,
        DirectLinkTransformationIdentityV1, DirectLinkWorkerConfigurationIdentityV1,
        DirectLinkWorkerExecutableIdentityV1, DirectLinkWorkerIdentityV1, IdentityText,
        MeasuredToolIdentity, PayloadDigest, ProofArtifactIdentity, ProofExecutionIdentity,
        ProofOutcome, ProofRecordV1, ProofTargetIdentity, SourceContractIdentity,
        VerificationModelIdentity,
    };
    use fe2o3_build_authority::CompilerClosureV2;
    use fe2o3_hsaco_finalize::{
        ContentIdentityV1, ROW_SOFTMAX_V1_PROVIDER_ITEM_COUNT,
        ROW_SOFTMAX_V1_UPSTREAM_LLVM_BUILD_IDENTITY_V1,
    };
    use fe2o3_worker_v2_bundle::WorkerV3LoadEnvelopeErrorV1;
    use fe2o3_worker_v2_bundle::{
        ExactRawHsacoV1, WorkerV2EnvelopeInputsV1, WorkerV2ProducerBindingV2,
    };
    use reserved_fe2o3_symbols::{CRATE_BINDING_ID_ENV_V1, derive_crate_binding_id_v1};
    use serde_json::json;
    use sha2::{Digest, Sha256};
    use std::collections::BTreeMap;
    use std::ffi::{OsStr, OsString};
    use std::fs;
    #[cfg(target_os = "linux")]
    use std::path::{Path, PathBuf};
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

    fn row_softmax_provider_transcript() -> Vec<u8> {
        let mut fields = vec![vec![0xa5]; 49];
        fields[21] = b"fe2o3_device".to_vec();
        fields[22] = 7_u64.to_le_bytes().to_vec();
        fields[23] = vec![0x41; 16];
        for (index, field) in fields[26..34].iter_mut().enumerate() {
            *field = vec![u8::try_from(index + 1).unwrap(); 16];
        }
        let source = |name, bytes| {
            super::provider_source_identity(name, bytes)
                .unwrap()
                .to_vec()
        };
        let lib = source("lib.rs", include_bytes!("../../fe2o3-device/src/lib.rs"));
        let thread = source(
            "thread.rs",
            include_bytes!("../../fe2o3-device/src/thread.rs"),
        );
        let math = source("math.rs", include_bytes!("../../fe2o3-device/src/math.rs"));
        for (field, identity) in fields[34..42]
            .iter_mut()
            .zip([&lib, &thread, &thread, &thread, &lib, &math, &math, &math])
        {
            *field = identity.clone();
        }
        let mut transcript = Vec::new();
        for field in fields {
            transcript.extend_from_slice(&(field.len() as u64).to_le_bytes());
            transcript.extend_from_slice(&field);
        }
        transcript
    }

    #[test]
    fn row_softmax_provisioning_observes_exact_reviewed_provider_sources() {
        let transcript = row_softmax_provider_transcript();
        let json = row_softmax_provider_observation_json(&transcript).unwrap();
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(value["provider_stable_crate_id"], 7);
        assert_eq!(
            value["provider_definition_identities"]
                .as_array()
                .unwrap()
                .len(),
            8
        );
        assert_eq!(
            value["provider_source_identities"]
                .as_array()
                .unwrap()
                .len(),
            8
        );

        let mut fields = super::decode_framed_row_softmax_authority_fields(&transcript)
            .unwrap()
            .into_iter()
            .map(<[u8]>::to_vec)
            .collect::<Vec<_>>();
        fields[34][0] ^= 1;
        let mut hostile = Vec::new();
        for field in fields {
            hostile.extend_from_slice(&(field.len() as u64).to_le_bytes());
            hostile.extend_from_slice(&field);
        }
        assert!(
            row_softmax_provider_observation_json(&hostile)
                .unwrap_err()
                .contains("reviewed source files")
        );
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
        let RustcInvocationV2::Compile(compile) = classify_rustc_invocation_v2(&argv).unwrap()
        else {
            panic!("fixture must be a compile invocation");
        };

        assert_eq!(
            ordered_rustc_codegen_metadata_v1(compile).unwrap(),
            ["first", "second", "third", "fourth"]
        );
    }

    #[test]
    fn canonicalizes_every_supported_rustc_metadata_form() {
        let mut argv = args(&[
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
        canonicalize_rustc_metadata(&mut argv);
        let RustcInvocationV2::Compile(compile) = classify_rustc_invocation_v2(&argv).unwrap()
        else {
            panic!("fixture must be a compile invocation");
        };
        assert_eq!(
            ordered_rustc_codegen_metadata_v1(compile).unwrap(),
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
                (CARGO_METADATA_MUTATION_TEST_ONLY_ENV_V1.to_owned(), None),
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
                (
                    OsString::from(CARGO_METADATA_MUTATION_TEST_ONLY_ENV_V1),
                    None
                ),
                (OsString::from(CRATE_BINDING_ID_ENV_V1), None),
            ]
        );
    }

    #[test]
    fn metadata_mutation_hook_preserves_the_genuine_provider_observation() {
        let observation = CompileBuildObservationV2::from_ordered_metadata(
            "row_softmax",
            &[
                "0123456789abcdef".to_owned(),
                "fe2o3-row-softmax-v1-reviewed".to_owned(),
            ],
        )
        .expect("canonical row-softmax metadata observation");
        let mut command = Command::new("rustc");
        configure_build_observation_environment_with_test_mutation(
            &mut command,
            Some(observation),
            Some(OsStr::new("omit")),
        );
        let environment = command
            .get_envs()
            .map(|(name, value)| (name.to_owned(), value.map(OsString::from)))
            .collect::<BTreeMap<_, _>>();

        assert_eq!(
            environment.get(OsStr::new(CARGO_METADATA_BUILD_OBSERVATION_ENV_V2)),
            Some(&Some(OsString::from(
                observation.cargo_metadata_digest_hex()
            )))
        );
        assert_eq!(
            environment.get(OsStr::new(CARGO_METADATA_MUTATION_TEST_ONLY_ENV_V1)),
            Some(&Some(OsString::from("omit")))
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
        assert!(
            !crate::process_execution::status(&mut Command::new(disagreed))
                .unwrap()
                .success()
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_empty_metadata() {
        let argv = args(&["rustc", "--crate-name", "unit", "unit.rs", "-Cmetadata="]);
        let RustcInvocationV2::Compile(compile) = classify_rustc_invocation_v2(&argv).unwrap()
        else {
            panic!("fixture must be a compile invocation");
        };
        let error = ordered_rustc_codegen_metadata_v1(compile).unwrap_err();
        assert!(matches!(
            error,
            RustcCodegenMetadataErrorV1::EmptyMetadata { argument_index: 4 }
        ));
    }

    #[cfg(unix)]
    #[test]
    fn rejects_non_utf8_metadata_without_rendering_its_bytes() {
        use std::os::unix::ffi::OsStringExt;

        let invalid = OsString::from_vec(b"metadata=private-\xff-value".to_vec());
        let argv = vec![
            OsString::from("rustc"),
            OsString::from("--crate-name"),
            OsString::from("unit"),
            OsString::from("unit.rs"),
            OsString::from("-C"),
            invalid,
        ];
        let RustcInvocationV2::Compile(compile) = classify_rustc_invocation_v2(&argv).unwrap()
        else {
            panic!("fixture must be a compile invocation");
        };
        let error = ordered_rustc_codegen_metadata_v1(compile).unwrap_err();
        assert!(matches!(
            error,
            RustcCodegenMetadataErrorV1::NonUtf8CodegenOption { argument_index: 5 }
        ));
        assert_eq!(
            error.to_string(),
            "rustc codegen option at argv[5] is not valid UTF-8"
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
        let compiler_closure = [0x31; 32];
        assert_eq!(
            derive_build_attempt_input_with_config_identity(
                &first,
                None,
                &current_dir,
                compiler_closure,
            ),
            derive_build_attempt_input_with_config_identity(
                &first,
                None,
                &current_dir,
                compiler_closure,
            )
        );
        assert_ne!(
            derive_build_attempt_input_with_config_identity(
                &first,
                None,
                &current_dir,
                compiler_closure,
            ),
            derive_build_attempt_input_with_config_identity(
                &second,
                None,
                &current_dir,
                compiler_closure,
            )
        );
        assert_ne!(
            derive_build_attempt_input_with_config_identity(
                &first,
                None,
                &current_dir,
                compiler_closure,
            ),
            derive_build_attempt_input_with_config_identity(&first, None, &current_dir, [0x32; 32],)
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
        let compiler_closure = [0xa7; 32];
        let identity = row_softmax_effective_rustc_argv_identity(&argv, compiler_closure);

        let mut argv_oracle = sha2::Sha256::new();
        argv_oracle.update(ROW_SOFTMAX_EFFECTIVE_RUSTC_ARGV_DOMAIN_V1);
        argv_oracle.update((argv.len() as u64).to_le_bytes());
        for argument in &argv {
            let bytes = os_bytes(argument);
            argv_oracle.update((bytes.len() as u64).to_le_bytes());
            argv_oracle.update(bytes);
        }
        let oracle = BuildInvocation::from_bytes(argv_oracle.finalize().into())
            .bind_compiler_closure_v1(compiler_closure);
        assert_eq!(identity, oracle);

        for index in 0..argv.len() {
            let mut changed = argv.clone();
            changed[index].push("-changed");
            assert_ne!(
                row_softmax_effective_rustc_argv_identity(&changed, compiler_closure),
                identity,
                "argv[{index}] was not bound"
            );
        }
        let mut reordered = argv.clone();
        reordered.swap(1, 2);
        assert_ne!(
            row_softmax_effective_rustc_argv_identity(&reordered, compiler_closure),
            identity
        );
        assert_ne!(
            row_softmax_effective_rustc_argv_identity(&argv, [0xa8; 32]),
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
            derive_build_attempt_input_with_config_identity(
                &argv,
                Some(first),
                &current_dir,
                [0x21; 32],
            ),
            derive_build_attempt_input_with_config_identity(
                &argv,
                Some(second),
                &current_dir,
                [0x21; 32],
            )
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
            let complete_environment = materialize_closed_child_environment(
                &mut command,
                inherited
                    .iter()
                    .map(|(name, value)| (OsString::from(name), OsString::from(value))),
                Some(OsStr::new("kernel-ir-worker-v2")),
                "S09",
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
            ("FE2O3_QUALIFICATION_ORACLE_V1", "kernel-ir-worker-v2"),
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
                "FE2O3_QUALIFICATION_ORACLE_V1",
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
    fn protected_invocation_authority_is_scoped_to_production_routes() {
        assert!(qualification_selection_requires_protected_invocation(
            Some(OsStr::new(OBSOLETE_PRODUCTION_SELECTOR)),
            false,
        ));
        assert!(qualification_selection_requires_protected_invocation(
            None, false
        ));
        assert!(qualification_selection_requires_protected_invocation(
            Some(OsStr::new(ROW_SOFTMAX_V1_PIPELINE)),
            false,
        ));
        assert!(!qualification_selection_requires_protected_invocation(
            Some(OsStr::new(ROW_SOFTMAX_V1_PIPELINE)),
            true,
        ));
        for pipeline in [
            Some(OsStr::new("kernel-ir-v1")),
            Some(OsStr::new("kernel-ir-worker-v2")),
            Some(OsStr::new("collected-general-gemm-v1")),
        ] {
            assert!(!qualification_selection_requires_protected_invocation(
                pipeline, false
            ));
            assert!(!qualification_requires_compiler_closure_observation(
                pipeline
            ));
        }
        assert!(qualification_requires_compiler_closure_observation(Some(
            OsStr::new(ROW_SOFTMAX_V1_PIPELINE)
        )));
    }

    #[test]
    fn kernel_root_routing_prefers_exact_worker_selection_and_validates_cargo_fallback() {
        assert!(selected_kernel_root(Some(true), None).unwrap());
        assert!(!selected_kernel_root(Some(false), Some(OsStr::new("1"))).unwrap());
        assert!(selected_kernel_root(None, Some(OsStr::new("1"))).unwrap());
        assert!(!selected_kernel_root(None, None).unwrap());
        assert!(matches!(
            selected_kernel_root(None, Some(OsStr::new("true"))),
            Err(BindingWrapperError::InvalidCargoPrimaryPackage)
        ));
    }

    #[test]
    fn unmanaged_dependencies_do_not_inherit_compiler_authority_signals() {
        let mut command = Command::new("/proc/self/fd/194");
        for (name, value) in [
            ("FE2O3_CODEGEN_PIPELINE", "production-v1"),
            ("FE2O3_QUALIFICATION_ORACLE_V1", "production-v1"),
            ("FE2O3_HSACO_DIR", "/proc/self/fd/197"),
            ("FE2O3_CODEGEN_BACKEND_BUILD_OBSERVATION_V2", "44"),
            ("FE2O3_QUALIFICATION_CODEGEN_BACKEND_SHA256_V1", "45"),
            ("FE2O3_EXPECTED_COMPILER_CLOSURE_SHA256_V1", "55"),
            (
                "FE2O3_NON_PRODUCTION_UNPROTECTED_AUTHORITY_VALIDATION_V1",
                "1",
            ),
            ("FE2O3_WORKER_V2_CONFIG_V2", "/workspace/worker.json"),
            ("FE2O3_WORKER_V2_EXPECTED_ID_V1", "66"),
            ("FE2O3_PROTECTED_RELEASE_ACTION_V1", "row-softmax-v1-run"),
        ] {
            command.env(name, value);
        }
        command
            .env("FE2O3_TARGET", "gfx942")
            .env("FE2O3_CRATE_BINDING_ID_V1", "77");

        scope_unmanaged_dependency_environment(&mut command);
        let overrides = command
            .get_envs()
            .collect::<std::collections::BTreeMap<_, _>>();
        assert_eq!(
            overrides.get(OsStr::new("FE2O3_QUALIFICATION_ORACLE_V1")),
            Some(&Some(OsStr::new("kernel-ir-v1")))
        );
        for name in [
            "FE2O3_CODEGEN_PIPELINE",
            "FE2O3_HSACO_DIR",
            "FE2O3_CODEGEN_BACKEND_BUILD_OBSERVATION_V2",
            "FE2O3_QUALIFICATION_CODEGEN_BACKEND_SHA256_V1",
            "FE2O3_EXPECTED_COMPILER_CLOSURE_SHA256_V1",
            "FE2O3_NON_PRODUCTION_UNPROTECTED_AUTHORITY_VALIDATION_V1",
            "FE2O3_WORKER_V2_CONFIG_V2",
            "FE2O3_WORKER_V2_EXPECTED_ID_V1",
            "FE2O3_PROTECTED_RELEASE_ACTION_V1",
        ] {
            assert_eq!(overrides.get(OsStr::new(name)), Some(&None));
        }
        assert_eq!(
            overrides.get(OsStr::new("FE2O3_TARGET")),
            Some(&Some(OsStr::new("gfx942")))
        );
        assert_eq!(
            overrides.get(OsStr::new("FE2O3_CRATE_BINDING_ID_V1")),
            Some(&Some(OsStr::new("77")))
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
                "FE2O3_QUALIFICATION_CODEGEN_BACKEND_SHA256_V1",
                "44".repeat(32),
            );
        let inherited = [
            ("CARGO_MANIFEST_DIR", "/workspace"),
            ("FE2O3_QUALIFICATION_ORACLE_V1", "kernel-ir-worker-v2"),
            ("FE2O3_TARGET", "gfx942:xnack-"),
        ]
        .map(|(name, value)| (OsString::from(name), OsString::from(value)));
        let complete = materialize_reviewed_child_environment(
            Some(WorkerV2CompileEnvironmentProfileV1::S09AlphaGfx942O0),
            &mut command,
            inherited,
            None,
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
            ("FE2O3_QUALIFICATION_ORACLE_V1", "collected-scalar-gemm-v1"),
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
                "FE2O3_QUALIFICATION_CODEGEN_BACKEND_SHA256_V1",
                "44".repeat(32),
            )
            .env("FE2O3_CRATE_BINDING_ID_V1", "55".repeat(32))
            .env_remove("FE2O3_WORKER_V2_SOURCE_DEBUG_PROFILE_V1");
        command
    }

    fn general_gemm_environment() -> Vec<(OsString, OsString)> {
        let expected = WorkerV2ConfigIdentity::for_test([0x66; 32]).to_hex();
        let runtime_manifest =
            hex(&fe2o3_verifier::GENERAL_GEMM_RUNTIME_CLOSURE_V2_MANIFEST_SHA256);
        [
            ("CARGO_MANIFEST_DIR", "/workspace/general"),
            (
                "FE2O3_QUALIFICATION_ORACLE_V1",
                crate::worker_v2::GENERAL_GEMM_V1_PIPELINE,
            ),
            ("FE2O3_TARGET", "gfx942:xnack-"),
            ("FE2O3_VERIFY_KERNEL_IR", "1"),
            (
                "FE2O3_WORKER_V2_CONFIG_V2",
                "/workspace/general/worker-v2.json",
            ),
            ("FE2O3_WORKER_V2_EXPECTED_ID_V1", expected.as_str()),
            (
                GENERAL_GEMM_RUNTIME_CLOSURE_V2_ROOT_ENV,
                "/opt/fe2o3/verus-runtime-v2/0.2026.08.02",
            ),
            (
                GENERAL_GEMM_RUNTIME_CLOSURE_V2_MANIFEST_SHA256_ENV,
                runtime_manifest.as_str(),
            ),
        ]
        .map(|(name, value)| (OsString::from(name), OsString::from(value)))
        .into()
    }

    fn general_gemm_command() -> Command {
        let attempt = format!("1:{}:{}", "11".repeat(16), "22".repeat(32));
        let mut command = command_with_production_managed_arguments(&[
            "--crate-name",
            "tiled_gemm_general_v1_gpu",
            "/workspace/general/src/lib.rs",
        ]);
        command
            .env("LANG", "C.UTF-8")
            .env("PATH", "/usr/bin")
            .env("TMPDIR", "/proc/self/fd/197/private")
            .env("FE2O3_HSACO_DIR", "/proc/self/fd/197")
            .env("FE2O3_BUILD_ATTEMPT_V1", attempt)
            .env(
                "FE2O3_QUALIFICATION_CODEGEN_BACKEND_SHA256_V1",
                "44".repeat(32),
            )
            .env_remove("FE2O3_WORKER_V2_SOURCE_DEBUG_PROFILE_V1");
        command
    }

    fn general_gemm_pins() -> GeneralGemmChildPinsV1<'static> {
        GeneralGemmChildPinsV1 {
            manifest_path: Path::new("/workspace/general/worker-v2.json"),
            expected_identity: WorkerV2ConfigIdentity::for_test([0x66; 32]),
            runtime_closure_v2_root: Path::new("/opt/fe2o3/verus-runtime-v2/0.2026.08.02"),
            runtime_closure_v2_manifest_sha256:
                fe2o3_verifier::GENERAL_GEMM_RUNTIME_CLOSURE_V2_MANIFEST_SHA256,
        }
    }

    #[test]
    fn general_gemm_environment_retains_only_parent_authenticated_worker_pins() {
        let mut inherited = general_gemm_environment();
        inherited.push((OsString::from("HOME"), OsString::from("/discarded")));
        let mut command = general_gemm_command();
        let complete = materialize_general_gemm_v1_child_environment(
            &mut command,
            inherited,
            general_gemm_pins(),
        )
        .unwrap();
        let effective = complete.entries.into_iter().collect::<BTreeMap<_, _>>();

        assert_eq!(
            effective.get(OsStr::new("FE2O3_WORKER_V2_CONFIG_V2")),
            Some(&OsString::from("/workspace/general/worker-v2.json"))
        );
        assert_eq!(
            effective.get(OsStr::new("FE2O3_WORKER_V2_EXPECTED_ID_V1")),
            Some(&OsString::from("66".repeat(32)))
        );
        assert_eq!(
            effective.get(OsStr::new(GENERAL_GEMM_RUNTIME_CLOSURE_V2_ROOT_ENV)),
            Some(&OsString::from("/opt/fe2o3/verus-runtime-v2/0.2026.08.02"))
        );
        assert_eq!(
            effective.get(OsStr::new(
                GENERAL_GEMM_RUNTIME_CLOSURE_V2_MANIFEST_SHA256_ENV
            )),
            Some(&OsString::from(hex(
                &fe2o3_verifier::GENERAL_GEMM_RUNTIME_CLOSURE_V2_MANIFEST_SHA256
            )))
        );
        assert!(!effective.contains_key(OsStr::new("HOME")));
    }

    #[test]
    fn general_gemm_environment_rejects_missing_or_substituted_worker_pins() {
        for name in [
            "FE2O3_WORKER_V2_CONFIG_V2",
            "FE2O3_WORKER_V2_EXPECTED_ID_V1",
        ] {
            let mut inherited = general_gemm_environment();
            inherited.retain(|(candidate, _)| candidate != name);
            assert!(
                materialize_general_gemm_v1_child_environment(
                    &mut general_gemm_command(),
                    inherited,
                    general_gemm_pins(),
                )
                .is_err()
            );
        }

        for (name, value) in [
            ("FE2O3_WORKER_V2_CONFIG_V2", "/workspace/other.json"),
            ("FE2O3_WORKER_V2_EXPECTED_ID_V1", "77"),
            (
                GENERAL_GEMM_RUNTIME_CLOSURE_V2_ROOT_ENV,
                "/opt/fe2o3/verus-runtime-v2/substituted",
            ),
            (GENERAL_GEMM_RUNTIME_CLOSURE_V2_MANIFEST_SHA256_ENV, "77"),
        ] {
            let mut inherited = general_gemm_environment();
            inherited
                .iter_mut()
                .find(|(candidate, _)| candidate == name)
                .unwrap()
                .1 = OsString::from(value);
            assert!(
                materialize_general_gemm_v1_child_environment(
                    &mut general_gemm_command(),
                    inherited,
                    general_gemm_pins(),
                )
                .is_err()
            );
        }
    }

    #[test]
    fn general_gemm_pre_spawn_materialization_failure_revokes_attempt() {
        fe2o3_artifact_transaction::enable_same_mount_namespace_artifact_path_guard_v1();
        let directory = std::env::temp_dir().join(format!(
            "cargo-fe2o3-general-gemm-pre-spawn-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = fs::remove_dir_all(&directory);
        fs::create_dir_all(&directory).unwrap();
        let producer = ProducerIdentity::from_codegen(
            "tiled_gemm_general_v1_gpu",
            Some(Path::new("/workspace/general/src/lib.rs")),
        )
        .unwrap();
        let attempt = begin_build_attempt(
            &directory,
            &producer,
            BuildInvocation::from_bytes([0x81; 32]),
            BuildSession::from_bytes([0x82; 16]),
        )
        .unwrap();
        let guard = ManagedAttemptRevocationGuard {
            output_dir: directory.clone(),
            producer: producer.clone(),
            attempt,
            armed: true,
        };
        let mut inherited = general_gemm_environment();
        inherited.retain(|(name, _)| name != "FE2O3_TARGET");

        assert!(
            materialize_general_gemm_v1_child_environment(
                &mut general_gemm_command(),
                inherited,
                general_gemm_pins(),
            )
            .is_err()
        );
        drop(guard);

        assert!(finish_build_attempt(&directory, &producer, attempt).is_err());
        let _ = fs::remove_dir_all(directory);
    }

    #[test]
    fn general_gemm_pre_spawn_cleanup_failure_is_reported() {
        fe2o3_artifact_transaction::enable_same_mount_namespace_artifact_path_guard_v1();
        let directory = std::env::temp_dir().join(format!(
            "cargo-fe2o3-general-gemm-cleanup-error-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = fs::remove_dir_all(&directory);
        fs::create_dir_all(&directory).unwrap();
        let producer = ProducerIdentity::from_codegen(
            "tiled_gemm_general_v1_gpu",
            Some(Path::new("/workspace/general/src/lib.rs")),
        )
        .unwrap();
        let attempt = begin_build_attempt(
            &directory,
            &producer,
            BuildInvocation::from_bytes([0x91; 32]),
            BuildSession::from_bytes([0x92; 16]),
        )
        .unwrap();
        let mut guard = ManagedAttemptRevocationGuard {
            output_dir: directory.clone(),
            producer,
            attempt,
            armed: true,
        };
        fs::remove_dir_all(&directory).unwrap();
        fs::write(&directory, b"not a directory").unwrap();

        let error = pre_spawn_failure(
            Some(&mut guard),
            BindingWrapperError::BuildObservation("forced pre-spawn failure".to_owned()),
        );
        assert!(matches!(
            error,
            BindingWrapperError::ManagedCompletion {
                cleanup: Some(_),
                ..
            }
        ));
        assert!(!guard.armed);
        let _ = fs::remove_file(directory);
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
            effective.get(OsStr::new("FE2O3_QUALIFICATION_ORACLE_V1")),
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
        assert!(
            effective.contains_key(OsStr::new("FE2O3_QUALIFICATION_CODEGEN_BACKEND_SHA256_V1"))
        );
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
            None,
        )
        .unwrap();

        assert!(result.is_none());
        assert_eq!(format!("{command:?}"), before);
    }

    #[test]
    fn only_the_retained_rustc_loader_directory_is_managed() {
        assert!(super::is_managed_rustc_loader_environment(
            OsStr::new("LD_LIBRARY_PATH"),
            OsStr::new("/proc/self/fd/193")
        ));
        for (name, value) in [
            ("LD_LIBRARY_PATH", "/toolchain/lib"),
            ("LD_LIBRARY_PATH", "/proc/self/fd/192"),
            ("LD_PRELOAD", "/proc/self/fd/193"),
        ] {
            assert!(!super::is_managed_rustc_loader_environment(
                OsStr::new(name),
                OsStr::new(value)
            ));
        }
        assert!(super::is_cargo_augmented_validation_loader_environment(
            OsStr::new("LD_LIBRARY_PATH"),
            OsStr::new("/mutable/target/debug/deps:/proc/self/fd/193")
        ));
        assert!(!super::is_cargo_augmented_validation_loader_environment(
            OsStr::new("LD_LIBRARY_PATH"),
            OsStr::new("/proc/self/fd/193:/mutable/target/debug/deps")
        ));
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
            "FE2O3_CODEGEN_PIPELINE",
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
                    OsString::from("FE2O3_QUALIFICATION_ORACLE_V1"),
                    OsString::from("kernel-ir-worker-v2"),
                ),
                (
                    OsString::from("FE2O3_TARGET"),
                    OsString::from("gfx942:xnack-"),
                ),
            ];
            inherited.push((OsString::from(name), OsString::from("forbidden")));
            let error = materialize_closed_child_environment(
                &mut command,
                inherited,
                Some(OsStr::new("kernel-ir-worker-v2")),
                "S09",
            )
            .unwrap_err();
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
                    OsString::from("FE2O3_QUALIFICATION_ORACLE_V1"),
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
            .env(crate::NON_PRODUCTION_AUTHORITY_VALIDATION_ENV, "1")
            .env(QUALIFICATION_CODEGEN_BACKEND_SHA256_ENV_V1, "ab".repeat(32))
            .env("FE2O3_BUILD_ATTEMPT_V1", "attempt");
        let mut inherited = fixed().to_vec();
        inherited.push((
            OsString::from(crate::NON_PRODUCTION_AUTHORITY_VALIDATION_ENV),
            OsString::from("1"),
        ));
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
            OsString::from(QUALIFICATION_CODEGEN_BACKEND_SHA256_ENV_V1),
            OsString::from("ab".repeat(32)),
        )));
        assert!(complete.entries.contains(&(
            OsString::from(crate::EXPECTED_COMPILER_CLOSURE_SHA256_ENV),
            OsString::from("01".repeat(32)),
        )));
        assert!(complete.entries.contains(&(
            OsString::from(crate::NON_PRODUCTION_AUTHORITY_VALIDATION_ENV),
            OsString::from("1"),
        )));

        let mut protected_command = Command::new("/proc/self/fd/194");
        protected_command
            .env("LANG", "C.UTF-8")
            .env("PATH", "/usr/bin")
            .env("TMPDIR", "/proc/self/fd/197/private")
            .env(CODEGEN_BACKEND_BUILD_OBSERVATION_ENV_V2, "cd".repeat(32))
            .env_remove(crate::NON_PRODUCTION_AUTHORITY_VALIDATION_ENV);
        let mut protected_inherited = fixed().to_vec();
        protected_inherited.push((
            OsString::from(crate::EXPECTED_COMPILER_CLOSURE_SHA256_ENV),
            OsString::from("01".repeat(32)),
        ));
        let protected = materialize_row_softmax_v1_child_environment(
            &mut protected_command,
            protected_inherited,
        )
        .expect("materialize protected row-softmax environment");
        assert!(
            !protected
                .entries
                .iter()
                .any(|(name, _)| name == crate::NON_PRODUCTION_AUTHORITY_VALIDATION_ENV)
        );

        for (inherited_marker, explicit_marker) in [
            (Some("1"), Some("changed")),
            (Some("1"), None),
            (None, Some("1")),
        ] {
            let mut command = Command::new("/proc/self/fd/194");
            command
                .env("LANG", "C.UTF-8")
                .env("PATH", "/usr/bin")
                .env("TMPDIR", "/proc/self/fd/197/private")
                .env(QUALIFICATION_CODEGEN_BACKEND_SHA256_ENV_V1, "ab".repeat(32));
            match explicit_marker {
                Some(value) => {
                    command.env(crate::NON_PRODUCTION_AUTHORITY_VALIDATION_ENV, value);
                }
                None => {
                    command.env_remove(crate::NON_PRODUCTION_AUTHORITY_VALIDATION_ENV);
                }
            }
            let mut inherited = fixed().to_vec();
            if let Some(value) = inherited_marker {
                inherited.push((
                    OsString::from(crate::NON_PRODUCTION_AUTHORITY_VALIDATION_ENV),
                    OsString::from(value),
                ));
            }
            inherited.push((
                OsString::from(crate::EXPECTED_COMPILER_CLOSURE_SHA256_ENV),
                OsString::from("01".repeat(32)),
            ));
            let error =
                materialize_row_softmax_v1_child_environment(&mut command, inherited).unwrap_err();
            assert!(
                error.to_string().contains("exact qualification marker"),
                "{error}"
            );
        }

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
                .env(QUALIFICATION_CODEGEN_BACKEND_SHA256_ENV_V1, "ab".repeat(32));
            let mut inherited = fixed().to_vec();
            inherited.push((OsString::from(name), OsString::from("attacker")));
            let error =
                materialize_row_softmax_v1_child_environment(&mut command, inherited).unwrap_err();
            assert!(error.to_string().contains(name), "{error}");
        }
    }

    #[test]
    fn qualification_route_owns_its_exact_nonproduction_marker() {
        for (debug_build, expected) in [(true, Some("1")), (false, None)] {
            let mut command = Command::new("/toolchain/rustc");
            command.env(
                crate::NON_PRODUCTION_AUTHORITY_VALIDATION_ENV,
                "hostile-ambient-value",
            );
            configure_qualification_route_marker(&mut command, debug_build);
            let actual = command
                .get_envs()
                .find(|(name, _)| {
                    *name == OsStr::new(crate::NON_PRODUCTION_AUTHORITY_VALIDATION_ENV)
                })
                .and_then(|(_, value)| value)
                .and_then(OsStr::to_str);
            assert_eq!(actual, expected);
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
        let error = materialize_closed_child_environment(
            &mut command,
            [
                (
                    OsString::from("CARGO_MANIFEST_DIR"),
                    OsString::from("/workspace"),
                ),
                (
                    OsString::from("FE2O3_QUALIFICATION_ORACLE_V1"),
                    OsString::from("kernel-ir-worker-v2"),
                ),
                (
                    OsString::from("FE2O3_TARGET"),
                    OsString::from("gfx942:xnack-"),
                ),
            ],
            Some(OsStr::new("kernel-ir-worker-v2")),
            "S09",
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
        fe2o3_artifact_transaction::enable_same_mount_namespace_artifact_path_guard_v1();
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

    fn protected_test_closure(seed: u8) -> CompilerClosureV2 {
        CompilerClosureV2::new(
            [seed; 32],
            [seed.wrapping_add(1); 32],
            [seed.wrapping_add(2); 32],
            [seed.wrapping_add(3); 32],
            [seed.wrapping_add(4); 32],
            [seed.wrapping_add(5); 32],
        )
        .unwrap()
    }

    fn protected_test_publication_inputs(
        attempt: fe2o3_artifact_transaction::BuildAttempt,
        seed: u8,
    ) -> (
        Vec<u8>,
        DurableLinkPublicationPlanV1,
        UpstreamCodeObjectEvidenceIdentityV1,
    ) {
        let output = vec![seed; 37];
        let output_identity: [u8; 32] = sha2::Sha256::digest(&output).into();
        let plan = DurableLinkPublicationPlanV1::new(
            attempt,
            LinkPublicationScopeV1::new(
                PackageIdentityV1::from_bytes([seed.wrapping_add(1); 32]),
                KernelSetIdentityV1::from_bytes([seed.wrapping_add(2); 32]),
                TargetIdentityV1::from_bytes([seed.wrapping_add(3); 32]),
            ),
            CanonicalLinkRequestIdentityV1::from_bytes([seed.wrapping_add(4); 32]),
            PinnedWorkerIdentityV1::from_bytes([seed.wrapping_add(5); 32]),
            ValidatedResponseIdentityV1::from_bytes([seed.wrapping_add(6); 32]),
            LinkedOutputIdentityV1::from_bytes(output_identity),
            FinalizationIdentityV1::from_bytes([seed.wrapping_add(7); 32]),
            FinalizedOutputIdentityV1::from_bytes(output_identity),
            AtomicPublicationIdentityV1::from_bytes([seed.wrapping_add(8); 32]),
        );
        (
            output,
            plan,
            UpstreamCodeObjectEvidenceIdentityV1::from_bytes([seed.wrapping_add(9); 32]),
        )
    }

    fn envelope_payload_digest(bytes: [u8; 32]) -> PayloadDigest {
        PayloadDigest::new(DigestAlgorithm::Sha256, DigestBytes::from_bytes(bytes))
    }

    fn envelope_identity_text(value: &str) -> IdentityText {
        IdentityText::new(value).expect("fixture identity text")
    }

    fn protected_envelope_proof(
        kernel: &fe2o3_artifacts::KernelEntry,
    ) -> Result<ProofRecordV1, fe2o3_artifacts::ValidationError> {
        let tagged = |seed| envelope_payload_digest([seed; 32]);
        ProofRecordV1::new(
            ProofTargetIdentity::new(
                ProofArtifactIdentity::new(
                    envelope_payload_digest(*kernel.kernel_id().as_bytes()),
                    tagged(0x41),
                    envelope_payload_digest(*kernel.source_digest().as_bytes()),
                    tagged(0x42),
                    envelope_payload_digest(*kernel.executable_digest().as_bytes()),
                    tagged(0x43),
                    tagged(0x44),
                    tagged(0x45),
                ),
                SourceContractIdentity::new(
                    tagged(0x51),
                    tagged(0x52),
                    tagged(0x53),
                    tagged(0x54),
                    tagged(0x55),
                ),
            ),
            vec![],
            ProofExecutionIdentity::new(
                VerificationModelIdentity::new(
                    envelope_identity_text("binding-wrapper-model"),
                    tagged(0x61),
                ),
                MeasuredToolIdentity::new(
                    envelope_identity_text("binding-wrapper-verifier"),
                    envelope_identity_text("1"),
                    tagged(0x62),
                    tagged(0x63),
                ),
                MeasuredToolIdentity::new(
                    envelope_identity_text("binding-wrapper-solver"),
                    envelope_identity_text("1"),
                    tagged(0x64),
                    tagged(0x65),
                ),
                MeasuredToolIdentity::new(
                    envelope_identity_text("binding-wrapper-recorder"),
                    envelope_identity_text("1"),
                    tagged(0x66),
                    tagged(0x67),
                ),
                tagged(0x68),
            ),
            ProofOutcome::Failed,
            vec![],
            vec![],
        )
    }

    fn protected_required_envelope_inputs(
        finalized: &[u8],
        raw: &[u8],
        identity_seed: u8,
    ) -> WorkerV2EnvelopeInputsV1 {
        let container = canonical_worker_v2_container_for_fixture_v1(finalized).unwrap();
        let bundle = BundleIndexV1::from_containers(std::slice::from_ref(&container)).unwrap();
        let raw_hsaco = ExactRawHsacoV1::from_bytes(raw.to_vec()).unwrap();
        let finalized_identity = DigestAlgorithm::Sha256.calculate(finalized);
        let tagged = |seed: u8| envelope_payload_digest([seed.wrapping_add(identity_seed); 32]);
        let expectation = DirectLinkBindingExpectationV1::new(
            DirectLinkRequestIdentityV1::new(tagged(0x71)),
            DirectLinkWorkerIdentityV1::new(
                envelope_identity_text("binding-wrapper-worker"),
                envelope_identity_text("1"),
                DirectLinkWorkerExecutableIdentityV1::new(tagged(0x72)),
                DirectLinkWorkerConfigurationIdentityV1::new(tagged(0x73)),
            ),
            DirectLinkToolchainIdentityV1::new(
                envelope_identity_text("binding-wrapper-toolchain"),
                envelope_identity_text("1"),
                DirectLinkToolchainExecutableIdentityV1::new(tagged(0x74)),
                DirectLinkToolchainConfigurationIdentityV1::new(tagged(0x75)),
            ),
            DirectLinkResponseIdentityV1::new(tagged(0x76)),
            DirectLinkTransformationIdentityV1::new(
                DirectLinkLinkedOutputIdentityV1::new(raw_hsaco.identity()),
                DirectLinkFinalizationIdentityV1::new(tagged(0x77)),
                DirectLinkFinalizedPayloadIdentityV1::new(finalized_identity),
            ),
            DirectLinkFfiClosureIdentityV1::new(tagged(0x78)),
        );
        let direct_link = DirectLinkBundleEvidenceV1::bind(
            &bundle,
            &[&container],
            &[DirectLinkBindingSourceV1::new(&container, expectation)],
        )
        .unwrap();
        let proofs = container
            .manifest()
            .kernels()
            .iter()
            .map(protected_envelope_proof)
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        WorkerV2EnvelopeInputsV1::new(direct_link, proofs, raw_hsaco).unwrap()
    }

    struct ProtectedRequiredReadyFixture {
        directory: PathBuf,
        producer: ProducerIdentity,
        producer_binding: WorkerV2ProducerBindingV2,
        attempt: fe2o3_artifact_transaction::BuildAttempt,
        compiler_closure: CompilerClosureV2,
        output: Vec<u8>,
        plan: DurableLinkPublicationPlanV1,
        upstream: UpstreamCodeObjectEvidenceIdentityV1,
        intent: RecoveredWorkerV2PublicationIntentV2,
        store: WorkerV2ResumeStoreV2,
    }

    fn protected_required_ready_fixture(label: &str, seed: u8) -> ProtectedRequiredReadyFixture {
        let directory = test_artifact_directory(&format!("protected-required-{label}"));
        let crate_name = format!("protected_required_{}", label.replace('-', "_"));
        let source = format!("/workspace/protected-required-{label}.rs");
        let producer =
            ProducerIdentity::from_codegen(&crate_name, Some(Path::new(&source))).unwrap();
        let producer_binding =
            WorkerV2ProducerBindingV2::from_codegen(&crate_name, Some(Path::new(&source))).unwrap();
        let attempt = begin_build_attempt(
            &directory,
            &producer,
            BuildInvocation::from_bytes([seed; 32]),
            BuildSession::from_bytes([seed.wrapping_add(1); 16]),
        )
        .unwrap();
        let fixture = alpha_zeta_fixture(ProfileMutation::None);
        assert!(fixture.is_finalized);
        let output = fixture.bytes;
        let inputs = protected_required_envelope_inputs(&output, &output, seed.wrapping_add(2));
        let (plan, upstream) =
            derive_required_worker_v2_publication_plan_v1(&producer, attempt, &output, &inputs)
                .unwrap();
        let compiler_closure = protected_test_closure(seed.wrapping_add(3));
        let store = WorkerV2ResumeStoreV2::open(&directory, &producer).unwrap();
        let persisted = persist_admitted_worker_v2_intent_v2(
            &store,
            &producer,
            WorkerV2PublicationKindV1::FinalizedEnvelopeRequired,
            plan,
            upstream,
            &output,
            Some(&inputs),
            compiler_closure,
        )
        .unwrap();
        assert!(matches!(
            store.load().unwrap(),
            Some(ResumeMarkerStateV2::Ready { .. })
        ));
        ProtectedRequiredReadyFixture {
            directory,
            producer,
            producer_binding,
            attempt,
            compiler_closure,
            output,
            plan,
            upstream,
            intent: persisted.intent,
            store,
        }
    }

    fn test_artifact_directory(label: &str) -> std::path::PathBuf {
        fe2o3_artifact_transaction::enable_same_mount_namespace_artifact_path_guard_v1();
        let directory = std::env::temp_dir().join(format!(
            "cargo-fe2o3-binding-{label}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = fs::remove_dir_all(&directory);
        fs::create_dir(&directory).unwrap();
        directory
    }

    fn regular_file_snapshot(path: &Path) -> BTreeMap<OsString, Vec<u8>> {
        fs::read_dir(path)
            .unwrap()
            .map(|entry| entry.unwrap())
            .filter(|entry| entry.file_type().unwrap().is_file())
            .map(|entry| (entry.file_name(), fs::read(entry.path()).unwrap()))
            .collect()
    }

    fn recursive_file_snapshot(path: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
        fn visit(root: &Path, directory: &Path, snapshot: &mut BTreeMap<PathBuf, Vec<u8>>) {
            let mut entries = fs::read_dir(directory)
                .unwrap()
                .map(|entry| entry.unwrap())
                .collect::<Vec<_>>();
            entries.sort_by_key(|entry| entry.file_name());
            for entry in entries {
                let file_type = entry.file_type().unwrap();
                if file_type.is_dir() {
                    visit(root, &entry.path(), snapshot);
                } else if file_type.is_file() {
                    snapshot.insert(
                        entry.path().strip_prefix(root).unwrap().to_path_buf(),
                        fs::read(entry.path()).unwrap(),
                    );
                }
            }
        }

        let mut snapshot = BTreeMap::new();
        visit(path, path, &mut snapshot);
        snapshot
    }

    fn v1_coordination_snapshot(path: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
        recursive_file_snapshot(path)
            .into_iter()
            .filter(|(relative, _)| {
                relative
                    .components()
                    .next()
                    .and_then(|component| component.as_os_str().to_str())
                    .is_some_and(|name| {
                        name.starts_with(".fe2o3-compiler-module-handoff-v1-")
                            || name.starts_with(".fe2o3-worker-v2-publication-intent-v1-")
                    })
            })
            .collect()
    }

    fn managed_attempt_for_test(
        output_dir: &Path,
        producer: ProducerIdentity,
        attempt: fe2o3_artifact_transaction::BuildAttempt,
    ) -> ManagedAttempt {
        ManagedAttempt {
            output_dir: output_dir.to_path_buf(),
            producer,
            attempt,
            protected_source_path: None,
            compile_environment_profile: None,
            worker_v2: None,
            row_softmax_release: None,
            row_softmax_provision: false,
            #[cfg(feature = "compiler-handoff-observation-test-only")]
            compiler_handoff_observation: None,
        }
    }

    fn assert_completion_succeeded(result: Result<(), CompletionFailure>) {
        match result {
            Ok(()) => {}
            Err(CompletionFailure::Uncommitted(message)) => {
                panic!("completion terminated the attempt: {message}")
            }
            Err(CompletionFailure::PreserveAttempt(message)) => {
                panic!("completion preserved the attempt: {message}")
            }
        }
    }

    fn assert_completion_preserved(result: Result<(), CompletionFailure>, expected_message: &str) {
        match result {
            Err(CompletionFailure::PreserveAttempt(message)) => assert!(
                message.contains(expected_message),
                "unexpected preserved evidence: {message}"
            ),
            Err(CompletionFailure::Uncommitted(message)) => {
                panic!("completion revoked instead of preserving evidence: {message}")
            }
            Ok(()) => panic!("inconsistent protected completion unexpectedly succeeded"),
        }
    }

    fn finalized_artifact_path(path: &Path, plan: DurableLinkPublicationPlanV1) -> PathBuf {
        path.join(format!(
            ".fe2o3-link-artifact-v1-{}.bin",
            hex(plan.finalized_output().as_bytes())
        ))
    }

    fn persisted_v1_receipt(
        path: &Path,
        producer: &ProducerIdentity,
        attempt: fe2o3_artifact_transaction::BuildAttempt,
    ) -> fe2o3_artifact_transaction::BackendPublicationReceiptV1 {
        match read_backend_publication_receipt_v1(path, producer, attempt).unwrap() {
            PersistedBackendReceiptV1::Provenance(receipt) => receipt,
            receipt => panic!("expected durable ordinary V1 receipt, got {receipt:?}"),
        }
    }

    fn persisted_v2_receipt(
        path: &Path,
        producer: &ProducerIdentity,
        attempt: fe2o3_artifact_transaction::BuildAttempt,
    ) -> fe2o3_artifact_transaction::BackendPublicationReceiptV2 {
        match read_backend_publication_receipt_v2(path, producer, attempt).unwrap() {
            PersistedBackendReceiptV2::Provenance(receipt) => receipt,
            receipt => panic!("expected durable protected V2 receipt, got {receipt:?}"),
        }
    }

    fn publish_required_fixture_envelope(
        fixture: &ProtectedRequiredReadyFixture,
    ) -> (
        BackendPublicationReceiptV2,
        fe2o3_worker_v2_bundle::WorkerV2LoadEnvelopeV2,
    ) {
        let publication = publish_exact_hsaco_evidence_for_attempt_v2(
            &fixture.directory,
            &fixture.producer,
            fixture.attempt,
            fixture.plan,
            fixture.upstream,
            fixture.compiler_closure,
            &fixture.output,
        )
        .unwrap();
        let receipt = publication.receipt();
        let inputs = fixture
            .store
            .recover_envelope_inputs(fixture.attempt)
            .unwrap();
        let prepared = assemble_recovered_worker_v2_load_envelope_v2(
            &fixture.producer,
            fixture.plan,
            fixture.upstream,
            &fixture.output,
            publication.published_claim().clone(),
            &inputs,
            fixture.intent.record(),
            fixture.producer_binding.clone(),
            receipt,
            fixture.compiler_closure,
        )
        .unwrap();
        assert!(!prepared.grants_load_authority());
        assert!(!prepared.grants_launch_authority());
        let envelope = prepared.into_envelope();
        drop(publication);
        (receipt, envelope)
    }

    fn protected_envelope_path(directory: &Path) -> PathBuf {
        fs::read_dir(directory)
            .unwrap()
            .map(|entry| entry.unwrap())
            .find(|entry| {
                entry.file_name().to_str().is_some_and(|name| {
                    name.starts_with(".fe2o3-worker-v2-protected-load-envelope-v2-")
                })
            })
            .expect("durable protected envelope")
            .path()
    }

    #[test]
    fn protected_required_envelope_completion_is_production_reachable_and_inert() {
        let fixture = protected_required_ready_fixture("fresh", 0x31);
        let managed = managed_attempt_for_test(
            &fixture.directory,
            fixture.producer.clone(),
            fixture.attempt,
        );
        assert_completion_succeeded(publish_finish_and_clear_protected(
            &managed,
            &fixture.store,
            WorkerV2PublicationKindV1::FinalizedEnvelopeRequired,
            fixture.intent,
            fixture.compiler_closure,
            &fixture.producer_binding,
        ));
        assert!(fixture.store.load().unwrap().is_none());
        assert!(
            recover_worker_v2_publication_intent_v2(
                &fixture.directory,
                &fixture.producer,
                fixture.attempt,
                fixture.compiler_closure,
            )
            .is_err()
        );
        let receipt = persisted_v2_receipt(&fixture.directory, &fixture.producer, fixture.attempt);
        assert_eq!(receipt.compiler_closure(), fixture.compiler_closure);
        assert_eq!(
            fs::read(finalized_artifact_path(&fixture.directory, fixture.plan)).unwrap(),
            fixture.output
        );
        assert!(v1_coordination_snapshot(&fixture.directory).is_empty());
        drop(fixture.store);
        fs::remove_dir_all(fixture.directory).unwrap();
    }

    #[test]
    fn protected_required_envelope_substitution_preserves_exact_restart_evidence() {
        let fixture = protected_required_ready_fixture("substituted-envelope", 0x41);
        let (receipt, envelope) = publish_required_fixture_envelope(&fixture);
        fixture
            .store
            .publish_load_envelope(&envelope, receipt, fixture.compiler_closure)
            .unwrap();
        let state = fixture.store.load().unwrap().unwrap();
        let path = protected_envelope_path(&fixture.directory);
        let mut substituted = fs::read(&path).unwrap();
        *substituted.last_mut().unwrap() ^= 1;
        fs::write(&path, substituted).unwrap();
        let managed = managed_attempt_for_test(
            &fixture.directory,
            fixture.producer.clone(),
            fixture.attempt,
        );
        assert_completion_preserved(
            complete_recovered_protected_worker_v2(
                &managed,
                &fixture.store,
                state,
                fixture.compiler_closure,
                &fixture.producer_binding,
            ),
            "protected completion persistence",
        );
        assert_eq!(fixture.store.load().unwrap(), Some(state));
        assert!(
            recover_worker_v2_publication_intent_v2(
                &fixture.directory,
                &fixture.producer,
                fixture.attempt,
                fixture.compiler_closure,
            )
            .is_ok()
        );
        assert_eq!(
            persisted_v2_receipt(&fixture.directory, &fixture.producer, fixture.attempt,),
            receipt
        );
        drop(fixture.store);
        fs::remove_dir_all(fixture.directory).unwrap();
    }

    #[test]
    fn protected_required_stale_canonical_envelope_is_rejected_before_completion() {
        let fixture = protected_required_ready_fixture("stale-envelope", 0x49);
        let (receipt, envelope) = publish_required_fixture_envelope(&fixture);
        fixture
            .store
            .publish_load_envelope(&envelope, receipt, fixture.compiler_closure)
            .unwrap();
        let stale = protected_required_ready_fixture("stale-envelope-source", 0x4a);
        let (_, stale_envelope) = publish_required_fixture_envelope(&stale);
        fs::write(
            protected_envelope_path(&fixture.directory),
            stale_envelope.to_bytes(),
        )
        .unwrap();
        let state = fixture.store.load().unwrap().unwrap();
        let managed = managed_attempt_for_test(
            &fixture.directory,
            fixture.producer.clone(),
            fixture.attempt,
        );
        assert_completion_preserved(
            complete_recovered_protected_worker_v2(
                &managed,
                &fixture.store,
                state,
                fixture.compiler_closure,
                &fixture.producer_binding,
            ),
            "protected completion persistence",
        );
        assert_eq!(fixture.store.load().unwrap(), Some(state));
        assert!(
            recover_worker_v2_publication_intent_v2(
                &fixture.directory,
                &fixture.producer,
                fixture.attempt,
                fixture.compiler_closure,
            )
            .is_ok()
        );
        drop(stale.store);
        fs::remove_dir_all(stale.directory).unwrap();
        drop(fixture.store);
        fs::remove_dir_all(fixture.directory).unwrap();
    }

    #[test]
    fn protected_required_envelope_rejects_wrong_closure_attempt_producer_and_receipt() {
        let wrong_closure = protected_required_ready_fixture("wrong-closure", 0x51);
        let state = wrong_closure.store.load().unwrap().unwrap();
        let managed = managed_attempt_for_test(
            &wrong_closure.directory,
            wrong_closure.producer.clone(),
            wrong_closure.attempt,
        );
        assert_completion_preserved(
            complete_recovered_protected_worker_v2(
                &managed,
                &wrong_closure.store,
                state,
                protected_test_closure(0xf1),
                &wrong_closure.producer_binding,
            ),
            "recovery",
        );
        assert_eq!(wrong_closure.store.load().unwrap(), Some(state));
        drop(wrong_closure.store);
        fs::remove_dir_all(wrong_closure.directory).unwrap();

        let wrong_producer = protected_required_ready_fixture("wrong-producer", 0x61);
        let state = wrong_producer.store.load().unwrap().unwrap();
        let managed = managed_attempt_for_test(
            &wrong_producer.directory,
            wrong_producer.producer.clone(),
            wrong_producer.attempt,
        );
        let substituted_binding = WorkerV2ProducerBindingV2::from_codegen(
            "substituted_producer",
            Some(Path::new("/workspace/substituted-producer.rs")),
        )
        .unwrap();
        assert_completion_preserved(
            complete_recovered_protected_worker_v2(
                &managed,
                &wrong_producer.store,
                state,
                wrong_producer.compiler_closure,
                &substituted_binding,
            ),
            "canonical V2 envelope assembly",
        );
        assert_eq!(wrong_producer.store.load().unwrap(), Some(state));
        drop(wrong_producer.store);
        fs::remove_dir_all(wrong_producer.directory).unwrap();

        let wrong_attempt = protected_required_ready_fixture("wrong-attempt", 0x71);
        let other_directory = test_artifact_directory("protected-required-other-attempt");
        let other_attempt = begin_build_attempt(
            &other_directory,
            &wrong_attempt.producer,
            BuildInvocation::from_bytes([0x72; 32]),
            BuildSession::from_bytes([0x73; 16]),
        )
        .unwrap();
        let state = wrong_attempt.store.load().unwrap().unwrap();
        let managed = managed_attempt_for_test(
            &wrong_attempt.directory,
            wrong_attempt.producer.clone(),
            other_attempt,
        );
        assert_completion_preserved(
            complete_recovered_protected_worker_v2(
                &managed,
                &wrong_attempt.store,
                state,
                wrong_attempt.compiler_closure,
                &wrong_attempt.producer_binding,
            ),
            "journal publication",
        );
        assert_eq!(wrong_attempt.store.load().unwrap(), Some(state));
        drop(wrong_attempt.store);
        fs::remove_dir_all(wrong_attempt.directory).unwrap();
        fs::remove_dir_all(other_directory).unwrap();

        let wrong_receipt = protected_required_ready_fixture("wrong-receipt", 0x81);
        let (receipt, _) = publish_required_fixture_envelope(&wrong_receipt);
        let receipt_path = wrong_receipt.directory.join(".fe2o3-attempts-v1");
        let mut registry = fs::read(&receipt_path).unwrap();
        *registry.last_mut().unwrap() ^= 1;
        fs::write(&receipt_path, registry).unwrap();
        let state = wrong_receipt.store.load().unwrap().unwrap();
        let managed = managed_attempt_for_test(
            &wrong_receipt.directory,
            wrong_receipt.producer.clone(),
            wrong_receipt.attempt,
        );
        assert_completion_preserved(
            complete_recovered_protected_worker_v2(
                &managed,
                &wrong_receipt.store,
                state,
                wrong_receipt.compiler_closure,
                &wrong_receipt.producer_binding,
            ),
            "recovery",
        );
        assert_eq!(wrong_receipt.store.load().unwrap(), Some(state));
        assert_ne!(
            read_backend_publication_receipt_v2(
                &wrong_receipt.directory,
                &wrong_receipt.producer,
                wrong_receipt.attempt,
            )
            .ok(),
            Some(PersistedBackendReceiptV2::Provenance(receipt))
        );
        drop(wrong_receipt.store);
        fs::remove_dir_all(wrong_receipt.directory).unwrap();
    }

    fn prepare_v1_ready_publication(
        path: &Path,
        producer: &ProducerIdentity,
        attempt: fe2o3_artifact_transaction::BuildAttempt,
        output: &[u8],
        plan: DurableLinkPublicationPlanV1,
        upstream: UpstreamCodeObjectEvidenceIdentityV1,
    ) -> fe2o3_artifact_transaction::RecoveredWorkerV2PublicationIntentV1 {
        let publication = WorkerV2PublicationKindV1::Finalized;
        let admission =
            restart_admission_commitment_with_inputs_v1(publication, plan, upstream, output, None);
        let store = WorkerV2ResumeStoreV1::open(path, producer).unwrap();
        store
            .persist_pending(publication, attempt, admission)
            .unwrap();
        let intent = persist_worker_v2_publication_intent_v1(
            path, producer, attempt, plan, upstream, output,
        )
        .unwrap();
        store
            .persist_ready(publication, attempt, intent.record().identity())
            .unwrap();
        intent
    }

    #[cfg(feature = "worker-v2-fault-injection-test-only")]
    const PROTECTED_RESTART_CHILD_DIRECTORY_ENV: &str =
        "FE2O3_TEST_BINDING_PROTECTED_RESTART_DIRECTORY";
    #[cfg(feature = "worker-v2-fault-injection-test-only")]
    const PROTECTED_RESTART_CHILD_LABEL_ENV: &str = "FE2O3_TEST_BINDING_PROTECTED_RESTART_LABEL";
    #[cfg(feature = "worker-v2-fault-injection-test-only")]
    const PROTECTED_RESTART_CHILD_CLOSURE_SEED_ENV: &str =
        "FE2O3_TEST_BINDING_PROTECTED_RESTART_CLOSURE_SEED";

    #[cfg(feature = "worker-v2-fault-injection-test-only")]
    fn protected_restart_producer(label: &str) -> ProducerIdentity {
        ProducerIdentity::from_codegen(
            &format!("protected_restart_{label}"),
            Some(Path::new("/workspace/protected_restart.rs")),
        )
        .unwrap()
    }

    #[cfg(feature = "worker-v2-fault-injection-test-only")]
    fn protected_restart_producer_binding(label: &str) -> WorkerV2ProducerBindingV2 {
        WorkerV2ProducerBindingV2::from_codegen(
            &format!("protected_restart_{label}"),
            Some(Path::new("/workspace/protected_restart.rs")),
        )
        .unwrap()
    }

    #[cfg(feature = "worker-v2-fault-injection-test-only")]
    #[test]
    fn protected_crash_restart_child() {
        let Some(directory) = std::env::var_os(PROTECTED_RESTART_CHILD_DIRECTORY_ENV) else {
            return;
        };
        let label = std::env::var(PROTECTED_RESTART_CHILD_LABEL_ENV).unwrap();
        let seed = std::env::var(PROTECTED_RESTART_CHILD_CLOSURE_SEED_ENV)
            .unwrap()
            .parse::<u8>()
            .unwrap();
        let directory = PathBuf::from(directory);
        let producer = protected_restart_producer(&label);
        let producer_binding = protected_restart_producer_binding(&label);
        let compiler_closure = protected_test_closure(seed);
        let resume = WorkerV2ResumeStoreV2::open(&directory, &producer).unwrap();
        let state = resume.load().unwrap().expect("protected ready marker");
        let managed = managed_attempt_for_test(&directory, producer, state.attempt());
        match complete_recovered_protected_worker_v2(
            &managed,
            &resume,
            state,
            compiler_closure,
            &producer_binding,
        ) {
            Ok(()) => panic!("protected crash child crossed its configured fault point"),
            Err(CompletionFailure::Uncommitted(message)) => {
                panic!("protected crash child terminated the attempt: {message}")
            }
            Err(CompletionFailure::PreserveAttempt(message)) => {
                panic!("protected crash child preserved the attempt: {message}")
            }
        }
    }

    #[cfg(feature = "worker-v2-fault-injection-test-only")]
    fn run_protected_crash_restart_case(fault_point: &str, seed: u8, required_envelope: bool) {
        let label = format!(
            "{}_{}",
            if required_envelope {
                "required"
            } else {
                "plain"
            },
            fault_point.replace('-', "_")
        );
        let directory = test_artifact_directory(&format!("protected-restart-{label}"));
        let producer = protected_restart_producer(&label);
        let producer_binding = protected_restart_producer_binding(&label);
        let compiler_closure = protected_test_closure(seed);
        let attempt = begin_build_attempt(
            &directory,
            &producer,
            BuildInvocation::from_bytes([seed.wrapping_add(1); 32]),
            BuildSession::from_bytes([seed.wrapping_add(2); 16]),
        )
        .unwrap();
        let (output, plan, upstream, envelope_inputs, publication) = if required_envelope {
            let fixture = alpha_zeta_fixture(ProfileMutation::None);
            assert!(fixture.is_finalized);
            let output = fixture.bytes;
            let inputs = protected_required_envelope_inputs(&output, &output, seed.wrapping_add(3));
            let (plan, upstream) =
                derive_required_worker_v2_publication_plan_v1(&producer, attempt, &output, &inputs)
                    .unwrap();
            (
                output,
                plan,
                upstream,
                Some(inputs),
                WorkerV2PublicationKindV1::FinalizedEnvelopeRequired,
            )
        } else {
            let (output, plan, upstream) =
                protected_test_publication_inputs(attempt, seed.wrapping_add(3));
            (
                output,
                plan,
                upstream,
                None,
                WorkerV2PublicationKindV1::Finalized,
            )
        };

        publish_compiler_module_handoff_v1(
            &directory,
            &producer,
            attempt,
            b"ordinary-v1-handoff-canary",
        )
        .unwrap();
        persist_worker_v2_publication_intent_v1(
            &directory, &producer, attempt, plan, upstream, &output,
        )
        .unwrap();
        let v1_before = v1_coordination_snapshot(&directory);
        assert!(!v1_before.is_empty());

        let resume = WorkerV2ResumeStoreV2::open(&directory, &producer).unwrap();
        let persisted = persist_admitted_worker_v2_intent_v2(
            &resume,
            &producer,
            publication,
            plan,
            upstream,
            &output,
            envelope_inputs.as_ref(),
            compiler_closure,
        )
        .unwrap();
        let intent_identity = persisted.intent.record().identity();
        assert!(matches!(
            resume.load().unwrap(),
            Some(ResumeMarkerStateV2::Ready { .. })
        ));
        drop(resume);

        let mut command = Command::new(std::env::current_exe().unwrap());
        command
            .arg("--exact")
            .arg("binding_wrapper::tests::protected_crash_restart_child")
            .arg("--nocapture")
            .arg("--test-threads=1")
            .env(PROTECTED_RESTART_CHILD_DIRECTORY_ENV, &directory)
            .env(PROTECTED_RESTART_CHILD_LABEL_ENV, &label)
            .env(PROTECTED_RESTART_CHILD_CLOSURE_SEED_ENV, seed.to_string())
            .env("FE2O3_TEST_WORKER_V2_FAULT_POINT_V1", fault_point);
        let status = crate::process_execution::status(&mut command).unwrap();
        assert_eq!(status.code(), Some(86));
        assert_eq!(v1_coordination_snapshot(&directory), v1_before);

        let receipt_before = persisted_v2_receipt(&directory, &producer, attempt);
        assert_eq!(receipt_before.compiler_closure(), compiler_closure);
        assert_eq!(
            fs::read(finalized_artifact_path(&directory, plan)).unwrap(),
            output
        );
        let resume = WorkerV2ResumeStoreV2::open(&directory, &producer).unwrap();
        let state = resume.load().unwrap().expect("crash left a V2 marker");
        if matches!(
            fault_point,
            "protected-published"
                | "protected-envelope-v2-temp-synced"
                | "protected-envelope-v2-published"
        ) {
            assert!(matches!(state, ResumeMarkerStateV2::Ready { .. }));
        } else {
            assert!(matches!(state, ResumeMarkerStateV2::Completed { .. }));
        }
        let intent_after_crash = recover_worker_v2_publication_intent_v2(
            &directory,
            &producer,
            attempt,
            compiler_closure,
        );
        if required_envelope
            || matches!(
                fault_point,
                "protected-published"
                    | "protected-envelope-v2-temp-synced"
                    | "protected-envelope-v2-published"
                    | "protected-completed"
            )
        {
            assert_eq!(
                intent_after_crash.unwrap().record().identity(),
                intent_identity
            );
        } else {
            assert!(intent_after_crash.is_err());
        }

        let managed = managed_attempt_for_test(&directory, producer.clone(), attempt);
        assert_completion_succeeded(complete_recovered_protected_worker_v2(
            &managed,
            &resume,
            state,
            compiler_closure,
            &producer_binding,
        ));
        assert!(resume.load().unwrap().is_none());
        drop(resume);
        assert!(
            recover_worker_v2_publication_intent_v2(
                &directory,
                &producer,
                attempt,
                compiler_closure,
            )
            .is_err()
        );
        assert_eq!(
            persisted_v2_receipt(&directory, &producer, attempt),
            receipt_before
        );
        assert_eq!(
            fs::read(finalized_artifact_path(&directory, plan)).unwrap(),
            output
        );
        assert_eq!(v1_coordination_snapshot(&directory), v1_before);

        let stable = recursive_file_snapshot(&directory);
        finish_build_attempt(&directory, &producer, attempt).unwrap();
        let reopened = WorkerV2ResumeStoreV2::open(&directory, &producer).unwrap();
        assert!(reopened.load().unwrap().is_none());
        drop(reopened);
        assert_eq!(recursive_file_snapshot(&directory), stable);
        fs::remove_dir_all(directory).unwrap();
    }

    #[cfg(feature = "worker-v2-fault-injection-test-only")]
    #[test]
    fn protected_published_crash_recovers_exactly_once_without_v1_probing() {
        run_protected_crash_restart_case("protected-published", 0x71, false);
    }

    #[cfg(feature = "worker-v2-fault-injection-test-only")]
    #[test]
    fn protected_completed_crash_recovers_exactly_once_without_v1_probing() {
        run_protected_crash_restart_case("protected-completed", 0x81, false);
    }

    #[cfg(feature = "worker-v2-fault-injection-test-only")]
    #[test]
    fn protected_intent_cleared_crash_recovers_exactly_once_without_v1_probing() {
        run_protected_crash_restart_case("protected-intent-cleared", 0x91, false);
    }

    #[cfg(feature = "worker-v2-fault-injection-test-only")]
    #[test]
    fn protected_finished_crash_recovers_exactly_once_without_v1_probing() {
        run_protected_crash_restart_case("protected-finished", 0xa1, false);
    }

    #[cfg(feature = "worker-v2-fault-injection-test-only")]
    #[test]
    fn protected_required_envelope_crashes_recover_at_every_exposed_boundary() {
        for (index, fault_point) in [
            "protected-published",
            "protected-envelope-v2-temp-synced",
            "protected-envelope-v2-published",
            "protected-completed",
            "protected-finished",
        ]
        .into_iter()
        .enumerate()
        {
            run_protected_crash_restart_case(fault_point, 0xb1_u8.wrapping_add(index as u8), true);
        }
    }

    #[derive(Clone, Copy)]
    enum FailClosedCase {
        RowSoftmax,
        InvalidEnvelopeInputs,
    }

    impl FailClosedCase {
        const fn label(self) -> &'static str {
            match self {
                Self::RowSoftmax => "row-softmax",
                Self::InvalidEnvelopeInputs => "invalid-envelope-inputs",
            }
        }

        const fn pipeline(self) -> &'static str {
            match self {
                Self::RowSoftmax => ROW_SOFTMAX_V1_PIPELINE,
                Self::InvalidEnvelopeInputs => "kernel-ir-worker-v2",
            }
        }

        const fn expected_error(self) -> &'static str {
            match self {
                Self::RowSoftmax => "protected row-softmax requires",
                Self::InvalidEnvelopeInputs => "must be one private, single-link regular file",
            }
        }
    }

    fn write_fail_closed_manifest(workspace: &Path, case: FailClosedCase) -> PathBuf {
        let worker = std::env::current_exe().unwrap();
        let worker_bytes = fs::read(&worker).unwrap();
        let worker_identity = ContentIdentityV1::calculate(&worker_bytes);
        let provider = workspace.join("provider.o");
        fs::write(&provider, b"provider").unwrap();
        let provider_identity = ContentIdentityV1::calculate(b"provider");
        let mut value = json!({
            "candidate_output_max_bytes": 4096,
            "format": "fe2o3-worker-v2-config-v2",
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
                "crate_name": "protected_fail_closed",
                "source": "src/lib.rs",
                "working_directory": workspace
            }],
            "worker": {
                "byte_len": worker_identity.byte_len(),
                "llvm_build_identity": "llvm-test-v1",
                "path": worker,
                "sha256": hex(worker_identity.sha256()),
                "worker_build_identity": "worker-test-v1"
            }
        });

        match case {
            FailClosedCase::RowSoftmax => {
                value["candidate_output_max_bytes"] = json!(fe2o3_hsaco::MAX_HSACO_BYTES);
                value["link_options"][1]["value"] = json!("0");
                value["providers"] = json!([]);
                value["worker"]["llvm_build_identity"] =
                    json!(ROW_SOFTMAX_V1_UPSTREAM_LLVM_BUILD_IDENTITY_V1);
                let definitions = (1_u8..=ROW_SOFTMAX_V1_PROVIDER_ITEM_COUNT as u8)
                    .map(|byte| hex(&[byte; 16]))
                    .collect::<Vec<_>>();
                let sources = [1_u8, 2, 2, 2, 1, 3, 3, 3].map(|byte| hex(&[byte; 32]));
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
            }
            FailClosedCase::InvalidEnvelopeInputs => {
                let capsule = workspace.join("envelope-inputs.capsule");
                fs::write(&capsule, b"capsule-canary").unwrap();
                value["load_envelope"] = json!("required");
                value["load_envelope_inputs"] = json!({
                    "byte_len": 14,
                    "path": capsule,
                    "sha256": hex(&Sha256::digest(b"capsule-canary"))
                });
            }
        }

        let path = workspace.join(format!("{}-config.json", case.label()));
        fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
        path
    }

    fn protected_compiler_capabilities_for_test(
        output_dir: &Path,
        config_identity: WorkerV2ConfigIdentity,
    ) -> CompilerCapabilities {
        let backend = PinnedCodegenBackend::open(&std::env::current_exe().unwrap()).unwrap();
        let compiler_closure = CompilerClosureV2::new(
            [0xb1; 32],
            [0xb2; 32],
            [0xb3; 32],
            [0xb4; 32],
            [0xb5; 32],
            *backend.sha256(),
        )
        .unwrap();
        let binding = crate::capability_broker::CapabilityBindingV3::new_protected(
            crate::capability_broker::CapabilityProfileV1::Ordinary,
            Some(*config_identity.as_bytes()),
            compiler_closure,
            [0xb6; 32],
        )
        .unwrap();
        let artifact =
            PinnedDirectory::open_existing(output_dir.to_path_buf(), "fail-closed artifact")
                .unwrap();
        CompilerCapabilities {
            binding,
            backend,
            artifact,
            compiler_closure: Some(
                fe2o3_compiler_closure_capability::CompilerClosureCapabilityV1::create(
                    compiler_closure,
                )
                .unwrap(),
            ),
            invocation_authority: None,
            output_dir: output_dir.to_path_buf(),
            pinned_cargo_image_sha256: Some(compiler_closure.cargo_executable_sha256()),
        }
    }

    fn seed_fail_closed_artifact_canaries(path: &Path) {
        let ordinary_producer = ProducerIdentity::from_codegen(
            "fail_closed_v1_canary",
            Some(Path::new("/workspace/fail_closed_v1_canary.rs")),
        )
        .unwrap();
        let ordinary_attempt = begin_build_attempt(
            path,
            &ordinary_producer,
            BuildInvocation::from_bytes([0xc1; 32]),
            BuildSession::from_bytes([0xc2; 16]),
        )
        .unwrap();
        let (ordinary_output, ordinary_plan, ordinary_upstream) =
            protected_test_publication_inputs(ordinary_attempt, 0xc3);
        publish_compiler_module_handoff_v1(
            path,
            &ordinary_producer,
            ordinary_attempt,
            b"fail-closed-v1-handoff",
        )
        .unwrap();
        prepare_v1_ready_publication(
            path,
            &ordinary_producer,
            ordinary_attempt,
            &ordinary_output,
            ordinary_plan,
            ordinary_upstream,
        );
        publish_exact_hsaco_evidence_for_attempt_v1(
            path,
            &ordinary_producer,
            ordinary_attempt,
            ordinary_plan,
            ordinary_upstream,
            &ordinary_output,
        )
        .unwrap();

        let protected_producer = ProducerIdentity::from_codegen(
            "fail_closed_v2_canary",
            Some(Path::new("/workspace/fail_closed_v2_canary.rs")),
        )
        .unwrap();
        let protected_attempt = begin_build_attempt(
            path,
            &protected_producer,
            BuildInvocation::from_bytes([0xd1; 32]),
            BuildSession::from_bytes([0xd2; 16]),
        )
        .unwrap();
        let compiler_closure = protected_test_closure(0xd3);
        let (protected_output, protected_plan, protected_upstream) =
            protected_test_publication_inputs(protected_attempt, 0xd4);
        publish_compiler_module_handoff_v2(
            path,
            &protected_producer,
            protected_attempt,
            compiler_closure,
            b"fail-closed-v2-handoff",
        )
        .unwrap();
        let protected_store = WorkerV2ResumeStoreV2::open(path, &protected_producer).unwrap();
        persist_admitted_worker_v2_intent_v2(
            &protected_store,
            &protected_producer,
            WorkerV2PublicationKindV1::Finalized,
            protected_plan,
            protected_upstream,
            &protected_output,
            None,
            compiler_closure,
        )
        .unwrap();
        publish_exact_hsaco_evidence_for_attempt_v2(
            path,
            &protected_producer,
            protected_attempt,
            protected_plan,
            protected_upstream,
            compiler_closure,
            &protected_output,
        )
        .unwrap();
    }

    const FAIL_CLOSED_CHILD_CASE_ENV: &str = "FE2O3_TEST_BINDING_FAIL_CLOSED_CASE";
    const FAIL_CLOSED_CHILD_ARTIFACT_ENV: &str = "FE2O3_TEST_BINDING_FAIL_CLOSED_ARTIFACT";

    #[test]
    fn protected_fail_closed_preparation_child() {
        let Some(case) = std::env::var_os(FAIL_CLOSED_CHILD_CASE_ENV) else {
            return;
        };
        let case = match case.to_str().unwrap() {
            "row-softmax" => FailClosedCase::RowSoftmax,
            "invalid-envelope-inputs" => FailClosedCase::InvalidEnvelopeInputs,
            value => panic!("unknown fail-closed child case {value:?}"),
        };
        let output_dir = PathBuf::from(std::env::var_os(FAIL_CLOSED_CHILD_ARTIFACT_ENV).unwrap());
        let worker_v2 = PreparedWorkerV2Config::from_environment()
            .unwrap()
            .expect("fail-closed Worker V2 config");
        let capabilities =
            protected_compiler_capabilities_for_test(&output_dir, worker_v2.identity());
        let current_dir = std::env::current_dir().unwrap();
        let argv = args(&["rustc", "--crate-name=protected_fail_closed", "src/lib.rs"]);
        let RustcInvocationV2::Compile(compile) = classify_rustc_invocation_v2(&argv).unwrap()
        else {
            panic!("fail-closed fixture did not classify as a compile");
        };
        match prepare_managed_attempt(
            compile,
            Some(worker_v2),
            &current_dir,
            &output_dir,
            &[],
            &capabilities,
        ) {
            Err(BindingWrapperError::BuildObservation(message))
                if matches!(case, FailClosedCase::RowSoftmax) =>
            {
                assert!(message.contains(case.expected_error()), "{message}");
            }
            Err(BindingWrapperError::WorkerV2Configuration(error))
                if matches!(case, FailClosedCase::InvalidEnvelopeInputs) =>
            {
                assert!(error.to_string().contains(case.expected_error()), "{error}");
            }
            Err(error) => panic!("unexpected fail-closed error: {error}"),
            Ok(_) => panic!("protected unavailable transition created a managed attempt"),
        }
    }

    fn run_fail_closed_artifact_immutability_case(case: FailClosedCase) {
        let root = test_artifact_directory(&format!("fail-closed-{}", case.label()));
        let workspace = root.join("workspace");
        let artifacts = root.join("artifacts");
        fs::create_dir(&workspace).unwrap();
        fs::create_dir(&artifacts).unwrap();
        let manifest = write_fail_closed_manifest(&workspace, case);
        seed_fail_closed_artifact_canaries(&artifacts);
        let evidence_snapshot = |path: &Path| {
            recursive_file_snapshot(path)
                .into_iter()
                .filter(|(name, bytes)| {
                    !matches!(case, FailClosedCase::InvalidEnvelopeInputs)
                        || !bytes.is_empty()
                        || !name
                            .file_name()
                            .and_then(OsStr::to_str)
                            .is_some_and(|name| name.ends_with(".lock"))
                })
                .collect::<BTreeMap<_, _>>()
        };
        let before = evidence_snapshot(&artifacts);
        assert!(!before.is_empty());

        let mut command = Command::new(std::env::current_exe().unwrap());
        command
            .arg("--exact")
            .arg("binding_wrapper::tests::protected_fail_closed_preparation_child")
            .arg("--nocapture")
            .arg("--test-threads=1")
            .current_dir(&workspace)
            .env(FAIL_CLOSED_CHILD_CASE_ENV, case.label())
            .env(FAIL_CLOSED_CHILD_ARTIFACT_ENV, &artifacts)
            .env("FE2O3_QUALIFICATION_ORACLE_V1", case.pipeline())
            .env("FE2O3_WORKER_V2_CONFIG_V2", &manifest)
            .env("FE2O3_BUILD_SESSION_V1", "e1".repeat(16));
        if matches!(case, FailClosedCase::RowSoftmax) {
            command.env(QUALIFICATION_RELEASE_ACTION_ENV, ROW_SOFTMAX_V1_RUN_VALUE);
        } else {
            command.env_remove(QUALIFICATION_RELEASE_ACTION_ENV);
        }
        let status = crate::process_execution::status(&mut command).unwrap();
        assert!(status.success());
        assert_eq!(evidence_snapshot(&artifacts), before);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn protected_row_softmax_fails_before_any_artifact_state_mutation() {
        run_fail_closed_artifact_immutability_case(FailClosedCase::RowSoftmax);
    }

    #[test]
    fn protected_invalid_envelope_inputs_leave_durable_evidence_unchanged() {
        run_fail_closed_artifact_immutability_case(FailClosedCase::InvalidEnvelopeInputs);
    }

    #[test]
    fn ordinary_v1_publication_and_completed_recovery_remain_stable() {
        let directory = test_artifact_directory("ordinary-v1-orchestration");
        let fresh_producer = ProducerIdentity::from_codegen(
            "ordinary_v1_fresh",
            Some(Path::new("/workspace/ordinary_v1_fresh.rs")),
        )
        .unwrap();
        let fresh_attempt = begin_build_attempt(
            &directory,
            &fresh_producer,
            BuildInvocation::from_bytes([0xe1; 32]),
            BuildSession::from_bytes([0xe2; 16]),
        )
        .unwrap();
        let (fresh_output, fresh_plan, fresh_upstream) =
            protected_test_publication_inputs(fresh_attempt, 0xe3);
        let fresh_intent = prepare_v1_ready_publication(
            &directory,
            &fresh_producer,
            fresh_attempt,
            &fresh_output,
            fresh_plan,
            fresh_upstream,
        );
        let fresh_store = WorkerV2ResumeStoreV1::open(&directory, &fresh_producer).unwrap();
        let fresh_managed =
            managed_attempt_for_test(&directory, fresh_producer.clone(), fresh_attempt);
        assert_completion_succeeded(publish_finish_and_clear(
            &fresh_managed,
            &fresh_store,
            WorkerV2PublicationKindV1::Finalized,
            fresh_intent,
        ));
        assert!(fresh_store.load().unwrap().is_none());
        assert!(
            recover_worker_v2_publication_intent_v1(&directory, &fresh_producer, fresh_attempt,)
                .is_err()
        );
        let fresh_receipt = persisted_v1_receipt(&directory, &fresh_producer, fresh_attempt);
        assert_eq!(
            fs::read(finalized_artifact_path(&directory, fresh_plan)).unwrap(),
            fresh_output
        );
        drop(fresh_store);

        let recovery_producer = ProducerIdentity::from_codegen(
            "ordinary_v1_recovery",
            Some(Path::new("/workspace/ordinary_v1_recovery.rs")),
        )
        .unwrap();
        let recovery_attempt = begin_build_attempt(
            &directory,
            &recovery_producer,
            BuildInvocation::from_bytes([0xf1; 32]),
            BuildSession::from_bytes([0xf2; 16]),
        )
        .unwrap();
        let (recovery_output, recovery_plan, recovery_upstream) =
            protected_test_publication_inputs(recovery_attempt, 0xf3);
        let recovery_intent = prepare_v1_ready_publication(
            &directory,
            &recovery_producer,
            recovery_attempt,
            &recovery_output,
            recovery_plan,
            recovery_upstream,
        );
        let recovery_publication = publish_exact_hsaco_evidence_for_attempt_v1(
            &directory,
            &recovery_producer,
            recovery_attempt,
            recovery_plan,
            recovery_upstream,
            &recovery_output,
        )
        .unwrap();
        let recovery_store = WorkerV2ResumeStoreV1::open(&directory, &recovery_producer).unwrap();
        let completed = recovery_store
            .persist_completed(
                WorkerV2PublicationKindV1::Finalized,
                recovery_attempt,
                recovery_intent.record().identity(),
                recovery_publication.receipt(),
            )
            .unwrap();
        let recovery_managed =
            managed_attempt_for_test(&directory, recovery_producer.clone(), recovery_attempt);
        assert_completion_succeeded(complete_recovered_worker_v2(
            &recovery_managed,
            &recovery_store,
            completed,
        ));
        assert!(recovery_store.load().unwrap().is_none());
        assert!(
            recover_worker_v2_publication_intent_v1(
                &directory,
                &recovery_producer,
                recovery_attempt,
            )
            .is_err()
        );
        assert_eq!(
            persisted_v1_receipt(&directory, &recovery_producer, recovery_attempt),
            recovery_publication.receipt()
        );
        assert_eq!(
            fs::read(finalized_artifact_path(&directory, recovery_plan)).unwrap(),
            recovery_output
        );
        assert_ne!(fresh_receipt, recovery_publication.receipt());
        drop(recovery_store);

        let stable = recursive_file_snapshot(&directory);
        finish_build_attempt(&directory, &fresh_producer, fresh_attempt).unwrap();
        finish_build_attempt(&directory, &recovery_producer, recovery_attempt).unwrap();
        assert_eq!(recursive_file_snapshot(&directory), stable);
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn ordinary_binding_selection_retains_the_v1_resume_schema_and_bytes() {
        assert_eq!(
            WorkerV2BindingSchema::select(None, false).unwrap(),
            WorkerV2BindingSchema::OrdinaryV1
        );
        let directory = test_artifact_directory("ordinary-v1");
        let producer = ProducerIdentity::from_codegen(
            "ordinary_v1",
            Some(Path::new("/workspace/ordinary_v1.rs")),
        )
        .unwrap();
        let attempt = begin_build_attempt(
            &directory,
            &producer,
            BuildInvocation::from_bytes([0x41; 32]),
            BuildSession::from_bytes([0x42; 16]),
        )
        .unwrap();
        let (output, plan, upstream) = protected_test_publication_inputs(attempt, 0x43);
        let publication = WorkerV2PublicationKindV1::Finalized;
        let admission =
            restart_admission_commitment_with_inputs_v1(publication, plan, upstream, &output, None);
        let store = WorkerV2ResumeStoreV1::open(&directory, &producer).unwrap();
        store
            .persist_pending(publication, attempt, admission)
            .unwrap();
        let expected = store.load().unwrap().unwrap();
        drop(store);
        let before = regular_file_snapshot(&directory);
        let reopened = WorkerV2ResumeStoreV1::open(&directory, &producer).unwrap();
        assert_eq!(reopened.load().unwrap(), Some(expected));
        drop(reopened);
        assert_eq!(regular_file_snapshot(&directory), before);
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn protected_binding_recovers_only_the_exact_v2_compiler_closure() {
        let closure = protected_test_closure(0x51);
        assert_eq!(
            WorkerV2BindingSchema::select(Some(closure), false).unwrap(),
            WorkerV2BindingSchema::ProtectedV2
        );
        assert_eq!(
            WorkerV2BindingSchema::select(Some(closure), true).unwrap(),
            WorkerV2BindingSchema::ProductionV3
        );
        assert!(WorkerV2BindingSchema::select(None, true).is_err());
        let directory = test_artifact_directory("protected-v2");
        let producer = ProducerIdentity::from_codegen(
            "protected_v2",
            Some(Path::new("/workspace/protected_v2.rs")),
        )
        .unwrap();
        let attempt = begin_build_attempt(
            &directory,
            &producer,
            BuildInvocation::from_bytes([0x52; 32]),
            BuildSession::from_bytes([0x53; 16]),
        )
        .unwrap();
        let (output, plan, upstream) = protected_test_publication_inputs(attempt, 0x54);
        let intent = persist_worker_v2_publication_intent_v2(
            &directory, &producer, attempt, plan, upstream, closure, &output,
        )
        .unwrap();
        let publication = WorkerV2PublicationKindV1::Finalized;
        let store = WorkerV2ResumeStoreV2::open(&directory, &producer).unwrap();
        store
            .persist_pending(publication, attempt, [0xa5; 32])
            .unwrap();
        let pending = store.load().unwrap().unwrap();
        drop(store);
        let reopened = WorkerV2ResumeStoreV2::open(&directory, &producer).unwrap();
        assert_eq!(reopened.load().unwrap(), Some(pending));
        let changed_closure = protected_test_closure(0x55);
        assert!(
            recover_worker_v2_publication_intent_v2(
                &directory,
                &producer,
                attempt,
                changed_closure,
            )
            .is_err()
        );
        assert_eq!(reopened.load().unwrap(), Some(pending));
        let recovered =
            recover_worker_v2_publication_intent_v2(&directory, &producer, attempt, closure)
                .unwrap();
        assert_eq!(recovered.record().identity(), intent.record().identity());
        assert_eq!(recovered.compiler_closure(), closure);
        assert_eq!(reopened.load().unwrap(), Some(pending));
        drop(reopened);
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn production_v3_readiness_recovery_distinguishes_absence_from_corruption() {
        assert!(worker_v3_readiness_is_absent(
            &WorkerV3LoadEnvelopeErrorV1::LoadReadiness(
                fe2o3_artifact_transaction::WorkerV3LoadReadinessErrorV1::AttemptState,
            ),
        ));
        assert!(worker_v3_readiness_is_absent(
            &WorkerV3LoadEnvelopeErrorV1::LoadReadiness(
                fe2o3_artifact_transaction::WorkerV3LoadReadinessErrorV1::MissingEnvelope,
            ),
        ));
        assert!(worker_v3_readiness_is_absent(
            &WorkerV3LoadEnvelopeErrorV1::LoadReadiness(
                fe2o3_artifact_transaction::WorkerV3LoadReadinessErrorV1::MissingClaim,
            ),
        ));
        assert!(!worker_v3_readiness_is_absent(
            &WorkerV3LoadEnvelopeErrorV1::BadMagic,
        ));
    }

    #[test]
    fn protected_binding_rejects_v1_resume_state_without_mutation() {
        let directory = test_artifact_directory("reject-v1");
        let producer = ProducerIdentity::from_codegen(
            "protected_reject_v1",
            Some(Path::new("/workspace/protected_reject_v1.rs")),
        )
        .unwrap();
        let attempt = begin_build_attempt(
            &directory,
            &producer,
            BuildInvocation::from_bytes([0x61; 32]),
            BuildSession::from_bytes([0x62; 16]),
        )
        .unwrap();
        let (output, plan, upstream) = protected_test_publication_inputs(attempt, 0x63);
        let publication = WorkerV2PublicationKindV1::Finalized;
        let admission =
            restart_admission_commitment_with_inputs_v1(publication, plan, upstream, &output, None);
        let ordinary = WorkerV2ResumeStoreV1::open(&directory, &producer).unwrap();
        ordinary
            .persist_pending(publication, attempt, admission)
            .unwrap();
        drop(ordinary);
        let before = regular_file_snapshot(&directory);
        assert!(WorkerV2ResumeStoreV2::open(&directory, &producer).is_err());
        assert_eq!(regular_file_snapshot(&directory), before);
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn protected_unavailable_transitions_exclude_the_required_envelope_route() {
        let protected = WorkerV2BindingSchema::ProtectedV2;
        assert_eq!(
            protected_worker_v2_transition_blocker(protected, true, false),
            Some(ProtectedWorkerV2TransitionBlocker::RowSoftmax)
        );
        assert_eq!(
            protected_worker_v2_transition_blocker(protected, false, true),
            Some(ProtectedWorkerV2TransitionBlocker::InRustcExecution)
        );
        assert_eq!(
            protected_worker_v2_transition_blocker(protected, false, false),
            None
        );
        assert_eq!(
            protected_worker_v2_transition_blocker(WorkerV2BindingSchema::OrdinaryV1, true, true,),
            None
        );
    }
}
