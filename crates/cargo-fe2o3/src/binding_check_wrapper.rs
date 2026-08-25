//! Authority-free rustc wrapper for host-only workspace checking.
//!
//! This mode derives the same compilation-unit binding as the production
//! wrapper, but deliberately has no backend, artifact directory, capability
//! broker, build attempt, publication, or GPU authority.

use fe2o3_rustc_invocation::{
    RustcArgsErrorV2, RustcCodegenMetadataErrorV1, RustcInvocationV2, classify_rustc_invocation_v2,
    is_rustc_codegen_backend_selector_v2, ordered_rustc_codegen_metadata_v1,
};
use reserved_fe2o3_symbols::{CRATE_BINDING_ID_ENV_V1, derive_crate_binding_id_v1};
use std::error::Error;
use std::ffi::OsString;
use std::fmt;
use std::process::{Command, ExitStatus, Stdio};

pub(crate) const MODE_ENV_V1: &str = "FE2O3_BINDING_CHECK_WRAPPER_MODE_V1";

const PROHIBITED_ENVIRONMENT: &[&str] = &[
    "FE2O3_AUTHORITY_BACKEND_SHA256_V1",
    "FE2O3_AUTHORITY_CARGO_BINDING_TRAMPOLINE_PATH_V1",
    "FE2O3_AUTHORITY_CARGO_BINDING_TRAMPOLINE_SHA256_V1",
    "FE2O3_AUTHORITY_CARGO_SHA256_V1",
    "FE2O3_AUTHORITY_RUSTC_PATH_V1",
    "FE2O3_AUTHORITY_RUSTC_RUNTIME_SHA256_V1",
    "FE2O3_AUTHORITY_RUSTC_SHA256_V1",
    "FE2O3_BACKEND",
    "FE2O3_BINDING_WRAPPER_MODE_V1",
    "FE2O3_BUILD_ATTEMPT_V1",
    "FE2O3_BUILD_CARGO_FE2O3_EXECUTABLE_SHA256_V1",
    "FE2O3_BUILD_SESSION_V1",
    "FE2O3_CAPABILITY_BROKER_V1",
    "FE2O3_CARGO_FE2O3_EXECUTABLE_BUILD_OBSERVATION_V2",
    "FE2O3_CARGO_METADATA_BUILD_OBSERVATION_V2",
    "FE2O3_CODEGEN_BACKEND_BUILD_OBSERVATION_V2",
    "FE2O3_CODEGEN_PIPELINE",
    "FE2O3_DECLARED_CARGO_EXECUTABLE_BUILD_OBSERVATION_V2",
    "FE2O3_EXPECTED_COMPILER_CLOSURE_SHA256_V1",
    "FE2O3_EXPECTED_RUSTC_SHA256_V1",
    "FE2O3_HOST_PASSTHROUGH",
    "FE2O3_HSACO_DIR",
    "FE2O3_LLVM_BUILD_IDENTITY_OBSERVATION_V2",
    "FE2O3_MANAGED_RUSTC_ARGS_V1",
    "FE2O3_NON_PRODUCTION_UNPROTECTED_AUTHORITY_VALIDATION_V1",
    "FE2O3_OBSERVED_PARENT_PID_BUILD_OBSERVATION_V2",
    "FE2O3_OBSERVED_PARENT_START_TIME_BUILD_OBSERVATION_V2",
    "FE2O3_PINNED_CARGO_IMAGE_BUILD_OBSERVATION_V2",
    "FE2O3_PROTECTED_RELEASE_ACTION_V1",
    "FE2O3_QUALIFICATION_CODEGEN_BACKEND_SHA256_V1",
    "FE2O3_SIMULATION_ATTEMPT_V1",
    "FE2O3_SIMULATION_MODE_V1",
    "FE2O3_TARGET",
    "FE2O3_WORKER_BUILD_IDENTITY_OBSERVATION_V2",
    "FE2O3_WORKER_CONFIG_BUILD_OBSERVATION_V2",
    "FE2O3_WORKER_EXECUTABLE_BUILD_OBSERVATION_V2",
    "FE2O3_WORKER_V2_CONFIG_V2",
    "FE2O3_WORKER_V2_EXPECTED_ID_V1",
    "FE2O3_WORKER_V2_SOURCE_DEBUG_PROFILE_V1",
];

#[derive(Debug)]
pub(crate) enum BindingCheckWrapperError {
    Arguments(RustcArgsErrorV2),
    Metadata(RustcCodegenMetadataErrorV1),
    MissingMetadata { crate_name: String },
    PreexistingBinding,
    ProhibitedEnvironment(&'static str),
    PreexistingCodegenBackend { argument_index: usize },
    UnsupportedInvocation,
    Spawn(std::io::Error),
}

impl fmt::Display for BindingCheckWrapperError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Arguments(error) => write!(formatter, "invalid rustc invocation: {error}"),
            Self::Metadata(error) => error.fmt(formatter),
            Self::MissingMetadata { crate_name } => write!(
                formatter,
                "rustc compile for crate `{crate_name}` has no explicit -C metadata value"
            ),
            Self::PreexistingBinding => write!(
                formatter,
                "binding-only check wrapper rejects inherited {CRATE_BINDING_ID_ENV_V1}"
            ),
            Self::ProhibitedEnvironment(name) => write!(
                formatter,
                "binding-only check wrapper rejects authority-bearing environment {name}"
            ),
            Self::PreexistingCodegenBackend { argument_index } => write!(
                formatter,
                "binding-only check argv[{argument_index}] contains a codegen-backend selector"
            ),
            Self::UnsupportedInvocation => formatter.write_str(
                "binding-only check wrapper rejects this rustc invocation classification",
            ),
            Self::Spawn(error) => write!(formatter, "failed to execute rustc: {error}"),
        }
    }
}

impl Error for BindingCheckWrapperError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Arguments(error) => Some(error),
            Self::Metadata(error) => Some(error),
            Self::Spawn(error) => Some(error),
            Self::MissingMetadata { .. }
            | Self::PreexistingBinding
            | Self::ProhibitedEnvironment(_)
            | Self::PreexistingCodegenBackend { .. }
            | Self::UnsupportedInvocation => None,
        }
    }
}

impl From<RustcArgsErrorV2> for BindingCheckWrapperError {
    fn from(value: RustcArgsErrorV2) -> Self {
        Self::Arguments(value)
    }
}

impl From<RustcCodegenMetadataErrorV1> for BindingCheckWrapperError {
    fn from(value: RustcCodegenMetadataErrorV1) -> Self {
        Self::Metadata(value)
    }
}

pub(crate) fn run(argv: Vec<OsString>) -> Result<ExitStatus, BindingCheckWrapperError> {
    reject_prohibited_environment()?;
    reject_codegen_backend(&argv)?;
    let invocation = classify_rustc_invocation_v2(&argv)?;
    let mut command = Command::new(invocation.executable());
    command
        .args(invocation.forwarded_args())
        .stdin(Stdio::null())
        .env_remove(MODE_ENV_V1);
    crate::remove_dynamic_loader_environment(&mut command);
    command.env(
        "LD_LIBRARY_PATH",
        format!("/proc/self/fd/{}", crate::RUSTC_LIBRARY_CHILD_FD),
    );

    match invocation {
        RustcInvocationV2::Compile(compile) => {
            let metadata = ordered_rustc_codegen_metadata_v1(compile)?;
            if metadata.is_empty() {
                return Err(BindingCheckWrapperError::MissingMetadata {
                    crate_name: compile.crate_name().to_owned(),
                });
            }
            let binding = derive_crate_binding_id_v1(
                compile.crate_name(),
                metadata.iter().map(String::as_str),
            );
            command.env(CRATE_BINDING_ID_ENV_V1, binding.to_hex());
        }
        RustcInvocationV2::Terminal(_) | RustcInvocationV2::Query(_)
            if invocation.is_bootstrap_passthrough_approved() =>
        {
            command.env_remove(CRATE_BINDING_ID_ENV_V1);
        }
        _ => return Err(BindingCheckWrapperError::UnsupportedInvocation),
    }

    crate::process_execution::status(&mut command).map_err(BindingCheckWrapperError::Spawn)
}

pub(crate) fn reject_prohibited_environment() -> Result<(), BindingCheckWrapperError> {
    if std::env::var_os(CRATE_BINDING_ID_ENV_V1).is_some() {
        return Err(BindingCheckWrapperError::PreexistingBinding);
    }
    for &name in PROHIBITED_ENVIRONMENT {
        if std::env::var_os(name).is_some() {
            return Err(BindingCheckWrapperError::ProhibitedEnvironment(name));
        }
    }
    Ok(())
}

fn reject_codegen_backend(argv: &[OsString]) -> Result<(), BindingCheckWrapperError> {
    for (index, argument) in argv.iter().enumerate() {
        if is_rustc_codegen_backend_selector_v2(
            argument,
            argv.get(index + 1).map(OsString::as_os_str),
        ) {
            return Err(BindingCheckWrapperError::PreexistingCodegenBackend {
                argument_index: index,
            });
        }
    }
    Ok(())
}

pub(crate) fn clear_prohibited_environment(command: &mut Command) {
    command.env_remove(CRATE_BINDING_ID_ENV_V1);
    for name in PROHIBITED_ENVIRONMENT {
        command.env_remove(name);
    }
}

pub(crate) fn exit_code(status: ExitStatus) -> u8 {
    status
        .code()
        .and_then(|code| u8::try_from(code).ok())
        .unwrap_or(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(values: &[&str]) -> Vec<OsString> {
        values.iter().map(OsString::from).collect()
    }

    #[test]
    fn backend_selectors_are_never_binding_only_checks() {
        for argv in [
            args(&[
                "rustc",
                "--crate-name",
                "unit",
                "src/lib.rs",
                "-Zcodegen-backend=x",
            ]),
            args(&[
                "rustc",
                "--crate-name",
                "unit",
                "src/lib.rs",
                "-Z",
                "codegen-backend=x",
            ]),
        ] {
            assert!(matches!(
                reject_codegen_backend(&argv),
                Err(BindingCheckWrapperError::PreexistingCodegenBackend { .. })
            ));
        }
    }

    #[test]
    fn ordinary_metadata_derives_the_shared_binding_contract() {
        let argv = args(&[
            "rustc",
            "--crate-name",
            "unit",
            "src/lib.rs",
            "-C",
            "metadata=first",
            "-Cmetadata=second",
        ]);
        let RustcInvocationV2::Compile(compile) = classify_rustc_invocation_v2(&argv).unwrap()
        else {
            panic!("expected compile invocation");
        };
        let metadata = ordered_rustc_codegen_metadata_v1(compile).unwrap();
        assert_eq!(metadata, ["first", "second"]);
        assert_eq!(
            derive_crate_binding_id_v1("unit", metadata.iter().map(String::as_str)),
            derive_crate_binding_id_v1("unit", ["first", "second"]),
        );
    }
}
