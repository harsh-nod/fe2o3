//! Authority-free rustc wrapper for host-only workspace checking and trusted tests.
//!
//! This mode derives the same compilation-unit binding as the production
//! wrapper, but deliberately has no backend, artifact directory, capability
//! broker, build attempt, publication, or GPU authority and performs no performance
//! prediction. Test execution remains trusted project code; this wrapper is not a sandbox.

use fe2o3_rustc_invocation::{
    RustcArgsErrorV2, RustcCodegenMetadataErrorV1, RustcInvocationV2, classify_rustc_invocation_v2,
    is_rustc_codegen_backend_selector_v2, ordered_rustc_codegen_metadata_v1,
};
use reserved_fe2o3_symbols::{CRATE_BINDING_ID_ENV_V1, derive_crate_binding_id_v1};
use std::error::Error;
use std::ffi::OsString;
use std::fmt;
use std::path::{Component, Path, PathBuf};
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
    "FE2O3_QUALIFICATION_ORACLE_V1",
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
    Projection(String),
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
            Self::Projection(error) => write!(formatter, "invalid binding projection: {error}"),
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
            | Self::Projection(_)
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
    let projection = crate::binding_check_projection::consume_inherited()
        .map_err(BindingCheckWrapperError::Projection)?;
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
            if projection_binding_for_source(&projection, compile.source_path())? {
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
            } else {
                command.env_remove(CRATE_BINDING_ID_ENV_V1);
            }
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

fn projection_binding_for_source(
    projection: &crate::binding_check_projection::Projection,
    source: &Path,
) -> Result<bool, BindingCheckWrapperError> {
    let current = std::env::current_dir()
        .map_err(|error| BindingCheckWrapperError::Projection(error.to_string()))?;
    let Some(target) = projected_target_for_source(projection, source, &current)
        .map_err(BindingCheckWrapperError::Projection)?
    else {
        return Ok(false);
    };

    let workspace = crate::project::PinnedDirectory::open_existing(
        projection.workspace_root.clone(),
        "binding projection workspace root",
    )
    .map_err(BindingCheckWrapperError::Projection)?;
    if !workspace.matches_identity(projection.workspace_device, projection.workspace_inode) {
        return Err(BindingCheckWrapperError::Projection(
            "binding projection workspace root identity changed".to_owned(),
        ));
    }
    let package = crate::project::PinnedDirectory::open_existing(
        target.package_root.clone(),
        "binding projection package root",
    )
    .map_err(BindingCheckWrapperError::Projection)?;
    if !package.matches_identity(target.package_device, target.package_inode) {
        return Err(BindingCheckWrapperError::Projection(format!(
            "binding projection package root identity changed for `{}`",
            target.package_name
        )));
    }
    let source_file = crate::example_manifest::open_contained_regular_file(
        &workspace,
        &target.source_path,
        "binding projection target source",
    )
    .map_err(BindingCheckWrapperError::Projection)?;
    let source_stat = rustix::fs::fstat(&source_file)
        .map_err(|error| BindingCheckWrapperError::Projection(error.to_string()))?;
    let source_identity = crate::binding_check_projection::ObjectIdentity::from_stat(&source_stat)
        .map_err(BindingCheckWrapperError::Projection)?;
    validate_source_identity(target, source_identity)
        .map_err(BindingCheckWrapperError::Projection)?;
    workspace
        .validate_path("binding projection workspace root")
        .map_err(BindingCheckWrapperError::Projection)?;
    package
        .validate_path("binding projection package root")
        .map_err(BindingCheckWrapperError::Projection)?;

    validate_cargo_owner(
        target,
        std::env::var_os("CARGO_PKG_NAME").as_deref(),
        std::env::var_os("CARGO_MANIFEST_DIR").as_deref(),
    )
    .map_err(BindingCheckWrapperError::Projection)?;
    Ok(target.managed)
}

fn projected_target_for_source<'projection>(
    projection: &'projection crate::binding_check_projection::Projection,
    source: &Path,
    current: &Path,
) -> Result<Option<&'projection crate::binding_check_projection::TargetSource>, String> {
    let absolute = if source.is_absolute() {
        source.to_path_buf()
    } else {
        current.join(source)
    };
    let absolute = lexical_normalize_absolute(&absolute)?;
    if !absolute.starts_with(&projection.workspace_root) {
        return Ok(None);
    }
    find_target_source(projection, &absolute)
        .map(Some)
        .ok_or_else(|| {
            format!(
                "rustc selected unknown in-workspace target source {}",
                absolute.display()
            )
        })
}

fn validate_source_identity(
    target: &crate::binding_check_projection::TargetSource,
    observed: crate::binding_check_projection::ObjectIdentity,
) -> Result<(), String> {
    if observed != target.source_identity {
        return Err(format!(
            "binding projection target source changed for `{}`",
            target.package_name
        ));
    }
    Ok(())
}

fn validate_cargo_owner(
    target: &crate::binding_check_projection::TargetSource,
    cargo_name: Option<&std::ffi::OsStr>,
    cargo_manifest: Option<&std::ffi::OsStr>,
) -> Result<(), String> {
    let cargo_name = cargo_name
        .ok_or_else(|| "Cargo omitted CARGO_PKG_NAME for an in-workspace target".to_owned())?;
    let cargo_manifest = cargo_manifest
        .ok_or_else(|| "Cargo omitted CARGO_MANIFEST_DIR for an in-workspace target".to_owned())?;
    if cargo_name != std::ffi::OsStr::new(&target.package_name)
        || Path::new(cargo_manifest) != target.package_root
    {
        return Err(format!(
            "Cargo package identity disagrees with target-source owner `{}`",
            target.package_name
        ));
    }
    Ok(())
}

fn find_target_source<'projection>(
    projection: &'projection crate::binding_check_projection::Projection,
    source: &Path,
) -> Option<&'projection crate::binding_check_projection::TargetSource> {
    let source = source.as_os_str().as_encoded_bytes();
    projection
        .targets
        .binary_search_by(|target| {
            target
                .source_path
                .as_os_str()
                .as_encoded_bytes()
                .cmp(source)
        })
        .ok()
        .map(|index| &projection.targets[index])
}

fn lexical_normalize_absolute(path: &Path) -> Result<PathBuf, String> {
    if !path.is_absolute() {
        return Err("rustc target source did not resolve to an absolute path".to_owned());
    }
    let mut normalized = PathBuf::from("/");
    for component in path.components() {
        match component {
            Component::RootDir | Component::CurDir => {}
            Component::Normal(component) => normalized.push(component),
            Component::ParentDir => {
                if !normalized.pop() {
                    return Err("rustc target source escapes the filesystem root".to_owned());
                }
            }
            Component::Prefix(_) => {
                return Err("rustc target source has an unsupported path prefix".to_owned());
            }
        }
    }
    Ok(normalized)
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
    fn qualification_environment_is_parent_prohibited_and_child_cleared() {
        const QUALIFICATION: &str = "FE2O3_QUALIFICATION_ORACLE_V1";
        assert!(PROHIBITED_ENVIRONMENT.contains(&QUALIFICATION));

        let mut child = Command::new("cargo");
        child.env(QUALIFICATION, "kernel-ir-v1");
        clear_prohibited_environment(&mut child);
        assert_eq!(
            child
                .get_envs()
                .find(|(name, _)| *name == std::ffi::OsStr::new(QUALIFICATION)),
            Some((std::ffi::OsStr::new(QUALIFICATION), None))
        );
    }

    #[test]
    fn rustc_target_paths_are_normalized_without_following_names() {
        assert_eq!(
            lexical_normalize_absolute(Path::new("/workspace/./crate/src/../src/lib.rs")).unwrap(),
            Path::new("/workspace/crate/src/lib.rs")
        );
        assert!(lexical_normalize_absolute(Path::new("relative.rs")).is_err());
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

    #[test]
    fn target_lookup_is_logarithmic_over_the_canonical_source_order() {
        let identity = crate::binding_check_projection::ObjectIdentity {
            device: 1,
            inode: 1,
            mode: 0o100644,
            size: 1,
            modified_seconds: 1,
            modified_nanoseconds: 1,
            changed_seconds: 1,
            changed_nanoseconds: 1,
        };
        let targets = ["a", "m", "z"]
            .into_iter()
            .map(|name| crate::binding_check_projection::TargetSource {
                package_name: name.to_owned(),
                package_root: PathBuf::from(format!("/workspace/{name}")),
                package_device: 1,
                package_inode: 1,
                source_path: PathBuf::from(format!("/workspace/{name}/src/lib.rs")),
                source_identity: identity,
                managed: true,
            })
            .collect();
        let projection = crate::binding_check_projection::Projection {
            workspace_root: PathBuf::from("/workspace"),
            workspace_device: 1,
            workspace_inode: 1,
            targets,
        };
        for name in ["a", "m", "z"] {
            assert_eq!(
                find_target_source(
                    &projection,
                    Path::new(&format!("/workspace/{name}/src/lib.rs"))
                )
                .unwrap()
                .package_name,
                name
            );
        }
        assert!(find_target_source(&projection, Path::new("/workspace/x/src/lib.rs")).is_none());

        let first = projected_target_for_source(
            &projection,
            Path::new("a/src/lib.rs"),
            Path::new("/workspace"),
        )
        .unwrap()
        .unwrap();
        assert_eq!(first.package_name, "a");
        assert!(
            projected_target_for_source(
                &projection,
                Path::new("/outside/src/lib.rs"),
                Path::new("/workspace"),
            )
            .unwrap()
            .is_none()
        );
        assert!(
            projected_target_for_source(
                &projection,
                Path::new("unknown/src/lib.rs"),
                Path::new("/workspace"),
            )
            .unwrap_err()
            .contains("unknown in-workspace")
        );

        assert!(validate_source_identity(first, identity).is_ok());
        let mut substituted = identity;
        substituted.inode = 2;
        assert!(validate_source_identity(first, substituted).is_err());
        assert!(
            validate_cargo_owner(
                first,
                Some(std::ffi::OsStr::new("a")),
                Some(std::ffi::OsStr::new("/workspace/a")),
            )
            .is_ok()
        );
        for (name, manifest) in [("attacker", "/workspace/a"), ("a", "/workspace/attacker")] {
            assert!(
                validate_cargo_owner(
                    first,
                    Some(std::ffi::OsStr::new(name)),
                    Some(std::ffi::OsStr::new(manifest)),
                )
                .is_err()
            );
        }
    }
}
