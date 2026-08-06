use std::error::Error;
use std::ffi::{OsStr, OsString};
use std::fmt;
use std::path::PathBuf;
use std::process::{Command, ExitStatus};

use fe2o3_artifact_transaction::{
    BuildAttempt, BuildInvocation, BuildSession, EmitError, ProducerIdentity, begin_build_attempt,
    fail_build_attempt, finish_build_attempt,
};
use fe2o3_rustc_invocation::{
    RustcArgsErrorV2, RustcCompileInvocationV2, RustcInvocationV2, classify_rustc_invocation_v2,
};
use reserved_fe2o3_symbols::{CRATE_BINDING_ID_ENV_V1, derive_crate_binding_id_v1};
use sha2::{Digest, Sha256};

const HSACO_DIR_ENV: &str = "FE2O3_HSACO_DIR";
const TARGET_ENV: &str = "FE2O3_TARGET";
const BUILD_SESSION_ENV: &str = "FE2O3_BUILD_SESSION_V1";
const BUILD_ATTEMPT_ENV: &str = "FE2O3_BUILD_ATTEMPT_V1";
const BUILD_INVOCATION_DOMAIN: &[u8] = b"FE2O3/BUILD-INVOCATION/V1\0";

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
    CurrentDirectory(std::io::Error),
    Artifact(EmitError),
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
            Self::CurrentDirectory(error) => {
                write!(
                    formatter,
                    "failed to resolve rustc working directory: {error}"
                )
            }
            Self::Artifact(error) => write!(formatter, "artifact build attempt failed: {error}"),
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
            Self::Artifact(error) => Some(error),
            Self::AttemptTermination { cleanup, .. } => Some(cleanup),
            Self::MissingMetadata { .. }
            | Self::InvalidCodegenOption { .. }
            | Self::EmptyMetadata { .. }
            | Self::MissingManagedEnvironment(_)
            | Self::InvalidBuildSession
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
    let (crate_binding, managed_attempt) = match invocation {
        RustcInvocationV2::Compile(compile) => {
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
            let attempt = prepare_managed_attempt(compile)?;
            (Some(binding), Some(attempt))
        }
        RustcInvocationV2::Terminal(_) | RustcInvocationV2::Query(_) => (None, None),
        _ => return Err(BindingWrapperError::UnsupportedInvocation),
    };

    let mut command = Command::new(invocation.executable());
    command.args(invocation.forwarded_args());
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
            finish_build_attempt(&managed.output_dir, &managed.producer, managed.attempt)
                .map_err(BindingWrapperError::Artifact)?;
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

struct ManagedAttempt {
    output_dir: PathBuf,
    producer: ProducerIdentity,
    attempt: BuildAttempt,
}

fn prepare_managed_attempt(
    compile: RustcCompileInvocationV2<'_>,
) -> Result<ManagedAttempt, BindingWrapperError> {
    let output_dir = std::env::var_os(HSACO_DIR_ENV)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .ok_or(BindingWrapperError::MissingManagedEnvironment(
            HSACO_DIR_ENV,
        ))?;
    let session = std::env::var(BUILD_SESSION_ENV)
        .ok()
        .and_then(|value| BuildSession::from_hex(&value).ok())
        .filter(|session| *session != BuildSession::DIRECT)
        .ok_or(BindingWrapperError::InvalidBuildSession)?;
    let producer =
        ProducerIdentity::from_codegen(compile.crate_name(), Some(compile.source_path()))
            .map_err(BindingWrapperError::Artifact)?;
    let invocation = derive_build_invocation(compile.argv())?;
    let attempt = begin_build_attempt(&output_dir, &producer, invocation, session)
        .map_err(BindingWrapperError::Artifact)?;
    Ok(ManagedAttempt {
        output_dir,
        producer,
        attempt,
    })
}

fn derive_build_invocation(argv: &[OsString]) -> Result<BuildInvocation, BindingWrapperError> {
    let current_dir = std::env::current_dir().map_err(BindingWrapperError::CurrentDirectory)?;
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
    Ok(BuildInvocation::from_bytes(digest.finalize().into()))
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
        BindingWrapperError, derive_build_invocation, is_cargo_stdin_probe, ordered_metadata_values,
    };
    use reserved_fe2o3_symbols::derive_crate_binding_id_v1;
    use std::ffi::OsString;

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
        assert_eq!(
            derive_build_invocation(&first).unwrap(),
            derive_build_invocation(&first).unwrap()
        );
        assert_ne!(
            derive_build_invocation(&first).unwrap(),
            derive_build_invocation(&second).unwrap()
        );
    }
}
