use std::error::Error;
use std::ffi::{OsStr, OsString};
use std::fmt;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus};

use fe2o3_artifact_transaction::{
    AttemptScopedHsacoPublicationErrorV1, BackendPublicationReceiptV1, BuildAttempt,
    BuildInvocation, BuildSession, EmitError, PersistedBackendReceiptV1, ProducerIdentity,
    RecoveredWorkerV2PublicationIntentV1, WorkerV2PublicationIntentErrorV1, begin_build_attempt,
    clear_worker_v2_publication_intent_v1, consume_compiler_module_handoff_v1, fail_build_attempt,
    finish_build_attempt, publish_exact_hsaco_evidence_for_attempt_v1,
    read_backend_publication_receipt_v1,
};
use fe2o3_hsaco_finalize::inspect_worker_v2_raw_hsaco_v1;
use fe2o3_rustc_invocation::{
    RustcArgsErrorV2, RustcCompileInvocationV2, RustcInvocationV2, classify_rustc_invocation_v2,
};
use reserved_fe2o3_symbols::{CRATE_BINDING_ID_ENV_V1, derive_crate_binding_id_v1};
use sha2::{Digest, Sha256};

use crate::capability_broker;
use crate::pinned_codegen_backend::{PinCodegenBackendError, PinnedCodegenBackend};
use crate::project::PinnedDirectory;
use crate::worker_v2::{
    PreparedWorkerV2Config, WORKER_V2_EXPECTED_ID_ENV, WorkerV2ConfigError, WorkerV2ConfigIdentity,
};
#[cfg(feature = "worker-v2-fault-injection-test-only")]
use crate::worker_v2_restart::injected_fault_point_v1;
use crate::worker_v2_restart::{
    RestartIntentErrorV1, ResumeMarkerErrorV1, ResumeMarkerStateV1, WorkerV2PublicationKindV1,
    WorkerV2ResumeStoreV1, persist_admitted_worker_v2_intent_v1, recover_worker_v2_intent_v1,
    restart_admission_commitment_v1,
};
use crate::{ARTIFACT_CHILD_FD, BACKEND_CHILD_FD, MANAGED_RUSTC_ARGS_ENV};

const HSACO_DIR_ENV: &str = "FE2O3_HSACO_DIR";
const TARGET_ENV: &str = "FE2O3_TARGET";
const BUILD_SESSION_ENV: &str = "FE2O3_BUILD_SESSION_V1";
const BUILD_ATTEMPT_ENV: &str = "FE2O3_BUILD_ATTEMPT_V1";
const BUILD_INVOCATION_DOMAIN: &[u8] = b"FE2O3/BUILD-INVOCATION/V1\0";
const WORKER_V2_CONFIG_ID_DOMAIN: &[u8] = b"FE2O3/WORKER-V2-CONFIG-ID/V1\0";

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
        }
    }
}

impl From<RustcArgsErrorV2> for BindingWrapperError {
    fn from(value: RustcArgsErrorV2) -> Self {
        Self::Arguments(value)
    }
}

pub(crate) fn run(argv: Vec<OsString>) -> Result<ExitStatus, BindingWrapperError> {
    reject_uninspectable_rustc_args(&argv)?;
    let invocation = match classify_rustc_invocation_v2(&argv) {
        Ok(invocation) => invocation,
        Err(_) if is_cargo_stdin_probe(&argv) => {
            let mut command = Command::new(&argv[0]);
            command.args(&argv[1..]);
            command.env_remove(CRATE_BINDING_ID_ENV_V1);
            return command.status().map_err(BindingWrapperError::Spawn);
        }
        Err(error) => return Err(error.into()),
    };
    let (crate_binding, managed_attempt, managed_rustc_args, compiler_capabilities) =
        match invocation {
            RustcInvocationV2::Compile(compile) => {
                let managed_rustc_args = managed_rustc_args_from_environment()?;
                let compiler_capabilities = CompilerCapabilities::from_environment()?;
                let metadata = ordered_metadata_values(compile.argv())?;
                if metadata.is_empty() {
                    return Err(BindingWrapperError::MissingMetadata {
                        crate_name: compile.crate_name().to_owned(),
                    });
                }
                let binding = derive_crate_binding_id_v1(
                    compile.crate_name(),
                    metadata.iter().map(String::as_str),
                );
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
                    Some(binding),
                    managed,
                    managed_rustc_args,
                    Some(compiler_capabilities),
                )
            }
            RustcInvocationV2::Terminal(_) | RustcInvocationV2::Query(_) => {
                (None, None, Vec::new(), None)
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

    let mut command = Command::new(invocation.executable());
    command.args(invocation.forwarded_args());
    command.args(managed_rustc_args);
    if let Some(capabilities) = &compiler_capabilities {
        capabilities.prepare_command(&mut command)?;
    }
    if let Some(crate_binding) = crate_binding {
        command.env(CRATE_BINDING_ID_ENV_V1, crate_binding.to_hex());
    } else {
        command.env_remove(CRATE_BINDING_ID_ENV_V1);
    }
    if let Some(managed) = &managed_attempt {
        command.env(BUILD_ATTEMPT_ENV, managed.attempt.to_env_value());
    } else {
        command.env_remove(BUILD_ATTEMPT_ENV);
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
        let output_dir = artifact.child_path();
        Ok(Self {
            backend,
            artifact,
            output_dir,
        })
    }

    fn output_dir(&self) -> &Path {
        &self.output_dir
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

#[derive(Clone, Copy)]
enum CompletionEnvelopeV1<'a> {
    Fresh(Option<&'a fe2o3_worker_v2_bundle::WorkerV2LoadEnvelopeV1>),
    RecoverDurable,
}

impl ManagedAttempt {
    fn is_worker_v2_recovery(&self) -> bool {
        matches!(self.worker_v2, Some(ManagedWorkerV2::Recovery { .. }))
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
    let invocation = derive_build_invocation(compile.argv(), worker_v2.as_ref(), current_dir);
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
            let attempt = begin_build_attempt(output_dir, &producer, invocation, session)
                .map_err(BindingWrapperError::Artifact)?;
            (attempt, Some(ManagedWorkerV2::Fresh { config, resume }))
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
                ManagedWorkerV2::Fresh { config, resume } => {
                    complete_fresh_worker_v2(&managed, config, resume)
                }
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
    )
    .map_err(|error| preserve_restart_error("persistence", error))?;
    publish_finish_and_clear(
        managed,
        resume,
        persisted.publication,
        persisted.intent,
        CompletionEnvelopeV1::Fresh(None),
    )
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
                .clear_exact(state)
                .map_err(|error| preserve_marker_error("abandoned-pending cleanup", error))?;
            return Err(CompletionFailure::Uncommitted(
                "Worker V2 process stopped before its publication intent became durable".into(),
            ));
        }
        Err(error) => return Err(preserve_restart_error("recovery", error)),
    };
    publish_finish_and_clear(
        managed,
        resume,
        state.publication(),
        intent,
        CompletionEnvelopeV1::RecoverDurable,
    )
}

fn publish_finish_and_clear(
    managed: &ManagedAttempt,
    resume: &WorkerV2ResumeStoreV1,
    publication: WorkerV2PublicationKindV1,
    intent: RecoveredWorkerV2PublicationIntentV1,
    envelope: CompletionEnvelopeV1<'_>,
) -> Result<(), CompletionFailure> {
    if publication.requires_envelope() && matches!(envelope, CompletionEnvelopeV1::Fresh(None)) {
        #[cfg(feature = "worker-v2-fault-injection-test-only")]
        injected_fault_point_v1("envelope-inputs-required");
        return Err(preserve_restart_error(
            "envelope assembly",
            RestartIntentErrorV1::MissingEnvelopeInputs,
        ));
    }
    let record = intent.record();
    let intent_identity = record.identity();
    let receipt = publish_recovered_worker_v2(managed, &intent)?;
    #[cfg(feature = "worker-v2-fault-injection-test-only")]
    injected_fault_point_v1("published");
    let completed = match envelope {
        CompletionEnvelopeV1::Fresh(Some(envelope)) => resume.persist_envelope_and_completed(
            publication,
            managed.attempt,
            intent_identity,
            receipt,
            envelope,
        ),
        CompletionEnvelopeV1::RecoverDurable if publication.requires_envelope() => resume
            .recover_envelope_and_completed(publication, managed.attempt, intent_identity, receipt),
        CompletionEnvelopeV1::Fresh(None) | CompletionEnvelopeV1::RecoverDurable => {
            resume.persist_completed(publication, managed.attempt, intent_identity, receipt)
        }
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
        .clear_completed(completed)
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
        validate_completed_worker_v2_envelope(resume, completed, receipt, managed.attempt)?;
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
        .clear_completed(completed)
        .map_err(|error| preserve_marker_error("completed recovery cleanup", error))
}

fn validate_completed_worker_v2_envelope(
    resume: &WorkerV2ResumeStoreV1,
    completed: ResumeMarkerStateV1,
    receipt: BackendPublicationReceiptV1,
    attempt: BuildAttempt,
) -> Result<(), CompletionFailure> {
    let envelope = resume.recover_load_envelope(receipt).map_err(|error| {
        CompletionFailure::PreserveAttempt(format!(
            "Worker V2 completed-recovery envelope inspection failed: {error}"
        ))
    })?;
    let claim = envelope.published_claim();
    let admission = restart_admission_commitment_v1(
        completed.publication(),
        claim.plan(),
        claim.upstream_evidence(),
        envelope.finalized_payload(),
    );
    if admission != completed.admission()
        || claim.receipt() != receipt
        || claim.plan().attempt() != attempt
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

fn derive_build_invocation(
    argv: &[OsString],
    worker_v2: Option<&PreparedWorkerV2Config>,
    current_dir: &std::path::Path,
) -> BuildInvocation {
    derive_build_invocation_with_config_identity(
        argv,
        worker_v2.map(PreparedWorkerV2Config::identity),
        current_dir,
    )
}

fn derive_build_invocation_with_config_identity(
    argv: &[OsString],
    worker_v2_identity: Option<WorkerV2ConfigIdentity>,
    current_dir: &std::path::Path,
) -> BuildInvocation {
    let mut digest = Sha256::new();
    digest.update(BUILD_INVOCATION_DOMAIN);
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
        BindingWrapperError, decode_managed_rustc_args,
        derive_build_invocation_with_config_identity, is_cargo_stdin_probe,
        ordered_metadata_values, reject_uninspectable_rustc_args,
    };
    use crate::worker_v2::WorkerV2ConfigIdentity;
    use crate::worker_v2_restart::{
        WorkerV2PublicationKindV1, WorkerV2ResumeStoreV1, restart_admission_commitment_v1,
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
    use reserved_fe2o3_symbols::derive_crate_binding_id_v1;
    use sha2::Digest;
    use std::ffi::OsString;
    use std::fs;
    use std::path::Path;

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
    fn invocation_identity_is_deterministic_and_argument_order_sensitive() {
        let first = args(&["rustc", "--crate-name", "unit", "unit.rs"]);
        let second = args(&["rustc", "unit.rs", "--crate-name", "unit"]);
        let current_dir = std::env::current_dir().unwrap();
        assert_eq!(
            derive_build_invocation_with_config_identity(&first, None, &current_dir),
            derive_build_invocation_with_config_identity(&first, None, &current_dir)
        );
        assert_ne!(
            derive_build_invocation_with_config_identity(&first, None, &current_dir),
            derive_build_invocation_with_config_identity(&second, None, &current_dir)
        );
    }

    #[test]
    fn worker_v2_config_identity_changes_build_invocation() {
        let argv = args(&["rustc", "--crate-name", "unit", "unit.rs"]);
        let current_dir = std::env::current_dir().unwrap();
        let first = WorkerV2ConfigIdentity::for_test([0x11; 32]);
        let second = WorkerV2ConfigIdentity::for_test([0x12; 32]);
        assert_ne!(
            derive_build_invocation_with_config_identity(&argv, Some(first), &current_dir),
            derive_build_invocation_with_config_identity(&argv, Some(second), &current_dir)
        );
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
        let admission = restart_admission_commitment_v1(required, plan, upstream, output);
        let store = WorkerV2ResumeStoreV1::open(&directory, &producer).unwrap();
        store.persist_pending(required, attempt, admission).unwrap();
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
