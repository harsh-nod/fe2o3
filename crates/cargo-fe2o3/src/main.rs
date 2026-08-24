mod application_exec;
mod application_handoff;
mod application_sandbox;
mod application_supervisor;
mod authority_release;
mod authorized_kernel_closure;
mod binding_wrapper;
mod capability_broker;
mod cargo_binding_trampoline;
mod cargo_invocation_boundary;
mod clean;
#[cfg(feature = "compiler-handoff-observation-test-only")]
mod compiler_handoff_observation;
mod compiler_toolchain;
mod example_manifest;
mod generation;
mod inert_rustc_invocation_capture;
mod inspect;
mod non_production_reproduction;
#[allow(dead_code)]
#[path = "../../../examples/row_softmax_v1/src/numerical_contract.rs"]
mod numerical_contract;
#[allow(dead_code)]
#[path = "rustc_wrapper/pinned_codegen_backend.rs"]
mod pinned_codegen_backend;
#[path = "rustc_wrapper/pinned_executable.rs"]
mod pinned_executable;
#[cfg(test)]
mod pinned_executable_test_directory;
#[cfg(feature = "legacy-hsa-runtime")]
#[path = "../../../examples/row_softmax_v1/src/production_release.rs"]
mod production_release;
#[cfg(not(feature = "legacy-hsa-runtime"))]
mod production_release_no_hardware;
#[cfg(not(feature = "legacy-hsa-runtime"))]
use production_release_no_hardware as production_release;
mod project;
mod protected_compiler_handoff_v3;
#[path = "rustc_runtime.rs"]
mod rustc_lib_tree;
mod simulation_capture;
mod tool_commands;
#[allow(dead_code)]
#[path = "../../../examples/row_softmax_v1/src/verification_certificate.rs"]
mod verification_certificate;
mod worker_v2;
mod worker_v2_artifact_container;
mod worker_v2_restart;

use std::env;
use std::ffi::{OsStr, OsString};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::os::fd::{AsRawFd, BorrowedFd, IntoRawFd};
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

const TARGET_ENV: &str = "FE2O3_TARGET";
const BACKEND_ENV: &str = "FE2O3_BACKEND";
const HSACO_DIR_ENV: &str = "FE2O3_HSACO_DIR";
const DEFAULT_TARGET: &str = "gfx1100";
const BINDING_WRAPPER_MODE_ENV: &str = "FE2O3_BINDING_WRAPPER_MODE_V1";
const MANAGED_RUSTC_ARGS_ENV: &str = "FE2O3_MANAGED_RUSTC_ARGS_V1";
const BUILD_SESSION_ENV: &str = "FE2O3_BUILD_SESSION_V1";
pub(crate) const SIMULATION_MODE_ENV: &str = "FE2O3_SIMULATION_MODE_V1";
const SIMULATION_PIPELINE: &str = "simulation-v1";
const SIMULATION_FAILURE_ALREADY_REPORTED: &str =
    "cargo fe2o3 simulate emitted a structured simulation error";
const EXPECTED_RUSTC_SHA256_ENV: &str = "FE2O3_EXPECTED_RUSTC_SHA256_V1";
const EXPECTED_COMPILER_CLOSURE_SHA256_ENV: &str = "FE2O3_EXPECTED_COMPILER_CLOSURE_SHA256_V1";
const AUTHORITY_CARGO_SHA256_ENV: &str = "FE2O3_AUTHORITY_CARGO_SHA256_V1";
const AUTHORITY_RUSTC_SHA256_ENV: &str = "FE2O3_AUTHORITY_RUSTC_SHA256_V1";
const AUTHORITY_RUSTC_PATH_ENV: &str = "FE2O3_AUTHORITY_RUSTC_PATH_V1";
const AUTHORITY_RUSTC_RUNTIME_SHA256_ENV: &str = "FE2O3_AUTHORITY_RUSTC_RUNTIME_SHA256_V1";
const AUTHORITY_BACKEND_SHA256_ENV: &str = "FE2O3_AUTHORITY_BACKEND_SHA256_V1";
const AUTHORITY_CARGO_BINDING_TRAMPOLINE_PATH_ENV: &str =
    "FE2O3_AUTHORITY_CARGO_BINDING_TRAMPOLINE_PATH_V1";
const AUTHORITY_CARGO_BINDING_TRAMPOLINE_SHA256_ENV: &str =
    "FE2O3_AUTHORITY_CARGO_BINDING_TRAMPOLINE_SHA256_V1";
const NON_PRODUCTION_AUTHORITY_VALIDATION_ENV: &str =
    "FE2O3_NON_PRODUCTION_UNPROTECTED_AUTHORITY_VALIDATION_V1";
const AUTHORITY_BEARING_ROW_PIPELINE: &str = "collected-row-softmax-v1";
pub(crate) const PROTECTED_RELEASE_ACTION_ENV: &str = "FE2O3_PROTECTED_RELEASE_ACTION_V1";
const INTERNAL_RUNNER_ARG: &str = "__fe2o3-runner-v1";
const CARGO_BINDING_WRAPPER_CHILD_FD: std::os::fd::RawFd = 191;
const CARGO_BINDING_TRAMPOLINE_CHILD_FD: std::os::fd::RawFd = 192;
const BACKEND_BUILD_CHILD_FD: std::os::fd::RawFd = 196;
const RUSTC_LIBRARY_CHILD_FD: std::os::fd::RawFd = 193;
const RUSTC_CHILD_FD: std::os::fd::RawFd = 194;
const RUSTC_INVOCATION_CHILD_FD: std::os::fd::RawFd =
    fe2o3_compiler_closure_capability::RUSTC_INVOCATION_CHILD_FD_V1;
const ARTIFACT_CHILD_FD: std::os::fd::RawFd =
    fe2o3_artifact_transaction::BROKERED_ARTIFACT_DIRECTORY_CHILD_FD_V1;
const BACKEND_CHILD_FD: std::os::fd::RawFd =
    fe2o3_artifact_transaction::BROKERED_CODEGEN_BACKEND_CHILD_FD_V1;
const _: () = assert!(RUSTC_INVOCATION_CHILD_FD != RUSTC_LIBRARY_CHILD_FD);
const _: () = assert!(RUSTC_INVOCATION_CHILD_FD != RUSTC_CHILD_FD);
const _: () = assert!(
    RUSTC_INVOCATION_CHILD_FD
        != fe2o3_artifact_transaction::BROKERED_INVOCATION_AUTHORITY_CHILD_FD_V1
);
const _: () = assert!(RUSTC_INVOCATION_CHILD_FD != ARTIFACT_CHILD_FD);
const _: () = assert!(RUSTC_INVOCATION_CHILD_FD != BACKEND_CHILD_FD);

const COMPILER_SELECTION_ENVIRONMENT: &[&str] = &[
    "RUSTC",
    "CARGO_BUILD_RUSTC",
    "RUSTC_WRAPPER",
    "CARGO_BUILD_RUSTC_WRAPPER",
    "RUSTC_WORKSPACE_WRAPPER",
];

#[derive(Clone, Copy)]
enum ProtectedReleaseAction {
    RowSoftmaxProvision,
    RowSoftmaxRun,
}

impl ProtectedReleaseAction {
    const fn environment_value(self) -> &'static str {
        match self {
            Self::RowSoftmaxProvision => "row-softmax-v1-provision",
            Self::RowSoftmaxRun => "row-softmax-v1-run",
        }
    }
}

fn main() -> ExitCode {
    let raw_args = env::args_os().skip(1).collect::<Vec<_>>();
    if raw_args
        .first()
        .is_some_and(|argument| argument == authority_release::INTERNAL_CHILD_ARG)
    {
        return authority_release::run_child(&raw_args[1..]);
    }
    if raw_args
        .first()
        .is_some_and(|argument| argument == application_supervisor::INTERNAL_SUPERVISOR_ARG)
    {
        return run_application_supervisor(&raw_args[1..]);
    }
    if raw_args
        .first()
        .is_some_and(|argument| argument == INTERNAL_RUNNER_ARG)
    {
        return run_application_boundary_frontend(&raw_args[1..]);
    }
    if env::var_os(BINDING_WRAPPER_MODE_ENV).is_some() {
        return match binding_wrapper::run(raw_args) {
            Ok(status) => ExitCode::from(binding_wrapper::exit_code(status)),
            Err(error) => {
                eprintln!("cargo-fe2o3 binding wrapper: {error}");
                ExitCode::FAILURE
            }
        };
    }

    let mut invocation = normalize_invocation(raw_args);
    let mut args = invocation.drain(..);
    let command = args.next().unwrap_or_else(|| OsString::from("help"));
    let rest: Vec<OsString> = args.collect();

    match command.to_str() {
        Some("authority") => authority_release::command(&rest),
        Some("doctor") => doctor(),
        Some("build") => cargo_with_backend("build", &rest),
        Some("run") => cargo_with_backend("run", &rest),
        Some("simulate") => simulate_command(&rest),
        Some("smoke") => with_utf8_args(&rest, smoke),
        Some("examples") => with_utf8_args(&rest, example_manifest::command),
        Some("clean") => clean_command(&rest),
        Some("inspect") => with_utf8_args(&rest, |args| report(inspect::command(args))),
        Some("sanitize") => with_utf8_args(&rest, |args| {
            tool_report(tool_commands::command(tool_commands::Mode::Sanitize, args))
        }),
        Some("debug") => with_utf8_args(&rest, |args| {
            tool_report(tool_commands::command(tool_commands::Mode::Debug, args))
        }),
        Some("help" | "--help" | "-h") => {
            print_help();
            ExitCode::SUCCESS
        }
        _ => {
            eprintln!("unknown cargo-fe2o3 command {command:?}");
            print_help();
            ExitCode::FAILURE
        }
    }
}

fn with_utf8_args(args: &[OsString], command: impl FnOnce(&[String]) -> ExitCode) -> ExitCode {
    let args = match args
        .iter()
        .map(|arg| {
            arg.to_str()
                .map(str::to_owned)
                .ok_or_else(|| format!("command argument is not valid UTF-8: {arg:?}"))
        })
        .collect::<Result<Vec<_>, _>>()
    {
        Ok(args) => args,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::FAILURE;
        }
    };
    command(&args)
}

fn report(result: Result<String, String>) -> ExitCode {
    match result {
        Ok(output) => {
            println!("{output}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

fn tool_report(result: Result<tool_commands::CommandReport, String>) -> ExitCode {
    match result {
        Ok(report) => {
            println!("{}", report.output());
            if report.succeeded() {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            }
        }
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

fn normalize_invocation(mut args: Vec<OsString>) -> Vec<OsString> {
    if args.first().is_some_and(|arg| arg == OsStr::new("fe2o3")) {
        args.remove(0);
    }
    args
}

fn clean_command(args: &[OsString]) -> ExitCode {
    let options = match clean::parse_project_options(args) {
        Ok(options) => options,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::FAILURE;
        }
    };
    let project = match project::CargoProject::discover(args, None, None, false) {
        Ok(project) => project,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::FAILURE;
        }
    };
    let plan = match clean::plan_project(project) {
        Ok(plan) => plan,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::FAILURE;
        }
    };

    match clean::execute_project(plan, options) {
        Ok(actions) => {
            for action in actions {
                eprintln!("{}", action.diagnostic());
            }
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

fn doctor() -> ExitCode {
    let target = amd_gpu_target(false);
    println!("fe2o3 diagnostics");
    println!("target: {target}");

    match detect_rocm_toolchain() {
        Ok(toolchain) => {
            println!("ROCm: {}", toolchain.rocm_path.display());
            println!("clang: {}", toolchain.clang.display());
            println!("ld.lld: {}", toolchain.ld_lld.display());
            if let Some(llc) = toolchain.llc {
                println!("llc: {}", llc.display());
            }
            if let Some(llvm_readobj) = toolchain.llvm_readobj {
                println!("llvm-readobj: {}", llvm_readobj.display());
            }
            println!("HIP: {}", toolchain.hip_library.display());
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("ROCm toolchain: {error}");
            ExitCode::FAILURE
        }
    }
}

fn cargo_with_backend(command: &str, args: &[OsString]) -> ExitCode {
    match cargo_with_backend_result(command, args, None, None, None) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

struct SimulationCommand {
    request: PathBuf,
    request_identity: fe2o3_kir_sim_cli::SimulationRequestIdentityV1,
    output: Option<PathBuf>,
}

fn simulate_command(args: &[OsString]) -> ExitCode {
    if matches!(args, [argument] if argument == "--help" || argument == "-h") {
        println!("{}", simulation_usage());
        return ExitCode::SUCCESS;
    }
    let (simulation, cargo_args) = match parse_simulation_command(args) {
        Ok(parsed) => parsed,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::FAILURE;
        }
    };
    match cargo_with_backend_result("build", &cargo_args, None, None, Some(&simulation)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            if error != SIMULATION_FAILURE_ALREADY_REPORTED {
                eprintln!("{error}");
            }
            ExitCode::FAILURE
        }
    }
}

fn parse_simulation_command(
    args: &[OsString],
) -> Result<(SimulationCommand, Vec<OsString>), String> {
    let mut request: Option<PathBuf> = None;
    let mut output: Option<PathBuf> = None;
    let mut index = 0;
    while index < args.len() {
        let argument = &args[index];
        if argument == "--" {
            let cargo_args = args[index + 1..].to_vec();
            let request = request.ok_or_else(simulation_usage)?;
            let request_identity = fe2o3_kir_sim_cli::bind_request_v1(&request)?;
            return Ok((
                SimulationCommand {
                    request,
                    request_identity,
                    output,
                },
                cargo_args,
            ));
        }
        let slot = if argument == "--request" {
            &mut request
        } else if argument == "--output" {
            &mut output
        } else {
            return Err(format!(
                "unknown cargo fe2o3 simulate option {argument:?}; {}",
                simulation_usage()
            ));
        };
        index += 1;
        let value = args
            .get(index)
            .ok_or_else(|| format!("{argument:?} requires a path; {}", simulation_usage()))?;
        if value.is_empty() || slot.is_some() {
            return Err(format!(
                "{argument:?} requires one non-empty path; {}",
                simulation_usage()
            ));
        }
        let path = PathBuf::from(value);
        let path = if path.is_absolute() {
            path
        } else {
            env::current_dir()
                .map_err(|error| format!("cannot resolve simulation path: {error}"))?
                .join(path)
        };
        *slot = Some(path);
        index += 1;
    }
    let request = request.ok_or_else(simulation_usage)?;
    let request_identity = fe2o3_kir_sim_cli::bind_request_v1(&request)?;
    Ok((
        SimulationCommand {
            request,
            request_identity,
            output,
        },
        Vec::new(),
    ))
}

fn simulation_usage() -> String {
    "usage: cargo fe2o3 simulate --request PATH [--output PATH] [-- CARGO_BUILD_ARGS...]".to_owned()
}

fn cargo_with_protected_release(
    args: &[OsString],
    admission: authority_release::ProtectedReleaseAdmission,
) -> ExitCode {
    let Some(command) = args.first().and_then(|value| value.to_str()) else {
        eprintln!("protected authority release has no UTF-8 Cargo command");
        return ExitCode::FAILURE;
    };
    if !matches!(command, "build" | "run") {
        eprintln!("protected authority release child requires build or run");
        return ExitCode::FAILURE;
    }
    let row_softmax = env::var_os(worker_v2::CODEGEN_PIPELINE_ENV).as_deref()
        == Some(OsStr::new(AUTHORITY_BEARING_ROW_PIPELINE));
    let action = match (command, row_softmax) {
        ("build", true) => Some(ProtectedReleaseAction::RowSoftmaxProvision),
        ("run", true) => Some(ProtectedReleaseAction::RowSoftmaxRun),
        _ => None,
    };
    if action.is_some() {
        eprintln!(
            "stage=binding-wrapper: gfx942 row-softmax production release requires an integrated static binding wrapper; Cargo mutates the dynamic-loader environment before invoking a Rust workspace wrapper, so the dynamic wrapper cannot hold compiler authority"
        );
        return ExitCode::FAILURE;
    }
    let cargo_command = if matches!(action, Some(ProtectedReleaseAction::RowSoftmaxRun)) {
        "build"
    } else {
        command
    };
    match cargo_with_backend_result(cargo_command, &args[1..], Some(&admission), action, None) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

fn smoke(args: &[String]) -> ExitCode {
    if !args.is_empty() {
        eprintln!("cargo fe2o3 smoke does not accept additional arguments");
        return ExitCode::FAILURE;
    }

    let workspace_root = match find_workspace_root() {
        Ok(path) => path,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::FAILURE;
        }
    };
    let packages = match example_manifest::gpu_smoke_packages(&workspace_root) {
        Ok(packages) => packages,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::FAILURE;
        }
    };
    for package in packages {
        eprintln!("cargo fe2o3 smoke: running {package}");
        let args = [OsString::from("-p"), OsString::from(package)];
        if let Err(error) = cargo_with_backend_result("run", &args, None, None, None) {
            eprintln!("{error}");
            return ExitCode::FAILURE;
        }
    }

    ExitCode::SUCCESS
}

fn cargo_with_backend_result(
    command: &str,
    args: &[OsString],
    protected_release: Option<&authority_release::ProtectedReleaseAdmission>,
    protected_release_action: Option<ProtectedReleaseAction>,
    simulation: Option<&SimulationCommand>,
) -> Result<(), String> {
    if authority_sensitive_request_selected(protected_release.is_some()) {
        reject_dynamic_loader_environment()?;
    }
    scrub_process_dynamic_loader_environment();
    reject_preexisting_compiler_environment()?;
    let worker_v2 =
        worker_v2::PreparedWorkerV2Config::from_environment_for_cargo_setup().map_err(|error| {
            if matches!(
                protected_release_action,
                Some(ProtectedReleaseAction::RowSoftmaxRun)
            ) {
                format!("stage=worker-artifact: Worker V2 setup failed: {error}")
            } else {
                format!("Worker V2 setup failed: {error}")
            }
        })?;
    if matches!(
        protected_release_action,
        Some(ProtectedReleaseAction::RowSoftmaxRun)
    ) && worker_v2
        .as_ref()
        .and_then(worker_v2::PreparedWorkerV2Config::row_softmax_v1)
        .is_none()
    {
        return Err(
            "cargo fe2o3 authority release run requires an exact row_softmax_v1 Worker V2 pin contract"
                .to_owned(),
        );
    }
    let requires_authorized_closure = protected_release.is_some()
        || env::var("FE2O3_CODEGEN_PIPELINE").as_deref() == Ok(AUTHORITY_BEARING_ROW_PIPELINE)
        || worker_v2
            .as_ref()
            .and_then(worker_v2::PreparedWorkerV2Config::source_debug_profile)
            .is_some();
    if requires_authorized_closure {
        require_protected_authority_launch(protected_release)?;
        reject_authority_environment_overrides(args)?;
    }
    let authority_rustc_sha256 = requires_authorized_closure
        .then(authority_rustc_sha256_from_environment)
        .transpose()?;
    let authority_rustc_lib_tree_sha256 = requires_authorized_closure
        .then(authority_rustc_runtime_sha256_from_environment)
        .transpose()?;
    let authority_cargo_sha256 = requires_authorized_closure
        .then(authority_cargo_sha256_from_environment)
        .transpose()?;
    let authority_backend_sha256 = requires_authorized_closure
        .then(authority_backend_sha256_from_environment)
        .transpose()?;
    let authority_backend = authority_backend_sha256
        .map(preflight_declared_authority_backend)
        .transpose()?;
    let invocation_directory = env::current_dir()
        .map_err(|error| format!("failed to resolve Cargo invocation directory: {error}"))?;
    let cargo_declaration = env::var_os("CARGO").unwrap_or_else(|| OsString::from("cargo"));
    let cargo_path = if requires_authorized_closure {
        require_absolute_authority_tool_path(&cargo_declaration, "CARGO")?
    } else {
        binding_wrapper::resolve_command_executable(&cargo_declaration, &invocation_directory)
            .map_err(|error| format!("failed to resolve Cargo executable: {error}"))?
    };
    let source_cargo = pinned_executable::PinnedExecutable::open(&cargo_path)
        .map_err(|error| format!("failed to pin Cargo executable: {error}"))?;
    if let Some(expected) = authority_cargo_sha256
        && source_cargo.sha256() != &expected
    {
        return Err(format!(
            "cargo fe2o3 authority Cargo does not match {AUTHORITY_CARGO_SHA256_ENV}"
        ));
    }
    let pinned_cargo = if requires_authorized_closure {
        source_cargo
            .seal_executable_image()
            .map_err(|error| format!("failed to seal authority Cargo executable: {error}"))?
    } else {
        source_cargo
    };
    let authority_rustc = if requires_authorized_closure {
        Some(pin_authority_rustc(
            &invocation_directory,
            authority_rustc_sha256.expect("authority rustc digest parsed"),
            authority_rustc_lib_tree_sha256.expect("authority rustc lib-tree digest parsed"),
        )?)
    } else {
        None
    };
    let project = project::CargoProject::discover(
        args,
        Some(&pinned_cargo),
        authority_rustc.as_ref(),
        requires_authorized_closure,
    )?;
    reject_configured_compiler_selection(
        &project,
        args,
        &pinned_cargo,
        authority_rustc.as_ref(),
        requires_authorized_closure,
    )?;
    if requires_authorized_closure {
        reject_authority_config_overrides(
            &project,
            args,
            &pinned_cargo,
            authority_rustc.as_ref().expect("authority rustc pinned"),
        )?;
    }
    let pinned_rustc = match authority_rustc {
        Some(rustc) => rustc,
        None => pin_default_rustc(&project)?,
    };
    pinned_rustc.assert_lib_tree_unmutated()?;
    let authorized_closure = requires_authorized_closure
        .then(|| {
            authorized_kernel_closure::AuthorizedKernelClosureV1::observe(
                &project,
                args,
                &pinned_cargo,
                &pinned_rustc,
            )
        })
        .transpose()?;
    let protected_binding_wrapper = protected_release
        .map(authority_release::ProtectedReleaseAdmission::pin_binding_wrapper)
        .transpose()?;
    let cargo_binding_trampoline = protected_release
        .map(|_| pin_authority_cargo_binding_trampoline())
        .transpose()?;
    let protected_compiler_closure =
        protected_release.map(authority_release::ProtectedReleaseAdmission::compiler_closure);
    let mut context = BackendRunContext::prepare(
        BackendRunPreparation {
            project,
            worker_v2,
            pinned_cargo,
            pinned_rustc,
            authority_backend,
            protected_binding_wrapper,
            cargo_binding_trampoline,
            protected_compiler_closure,
            authorized_closure,
            protected_release_action,
        },
        args,
        simulation.is_some(),
    )?;
    run_cargo_with_backend(&mut context, command, args, protected_release, simulation)
}

fn authority_sensitive_request_selected(protected_release: bool) -> bool {
    protected_release
        || env::var_os(worker_v2::CODEGEN_PIPELINE_ENV).as_deref()
        == Some(OsStr::new(AUTHORITY_BEARING_ROW_PIPELINE))
        // A Worker V2 manifest can select the source-debug authority profile. Treat the
        // unparsed selection as authority-sensitive so mutable manifest contents cannot
        // downgrade the loader check that precedes manifest authentication.
        || env::var_os(worker_v2::WORKER_V2_CONFIG_ENV).is_some()
}

fn preflight_declared_authority_backend(
    expected: [u8; 32],
) -> Result<(PathBuf, pinned_codegen_backend::PinnedCodegenBackend), String> {
    let path = env::var_os(BACKEND_ENV)
        .map(PathBuf::from)
        .ok_or_else(|| {
            format!(
                "cargo fe2o3 authority build requires {BACKEND_ENV} to name an explicit prebuilt codegen backend"
            )
        })?;
    if !path.is_absolute() {
        return Err(format!(
            "cargo fe2o3 authority build requires {BACKEND_ENV} to name an absolute prebuilt codegen backend path"
        ));
    }
    let backend = pinned_codegen_backend::PinnedCodegenBackend::open(&path)
        .map_err(|error| format!("failed to pin declared authority codegen backend: {error}"))?;
    if backend.sha256() != &expected {
        return Err(format!(
            "cargo fe2o3 authority backend does not match {AUTHORITY_BACKEND_SHA256_ENV}"
        ));
    }
    Ok((path, backend))
}

fn pin_authority_cargo_binding_trampoline() -> Result<pinned_executable::PinnedExecutable, String> {
    let declaration = env::var_os(AUTHORITY_CARGO_BINDING_TRAMPOLINE_PATH_ENV).ok_or_else(|| {
        format!(
            "cargo fe2o3 authority release requires {AUTHORITY_CARGO_BINDING_TRAMPOLINE_PATH_ENV}"
        )
    })?;
    let path = require_absolute_authority_tool_path(
        &declaration,
        AUTHORITY_CARGO_BINDING_TRAMPOLINE_PATH_ENV,
    )?;
    let expected =
        authority_sha256_from_environment(AUTHORITY_CARGO_BINDING_TRAMPOLINE_SHA256_ENV)?;
    let source = pinned_executable::PinnedExecutable::open(&path)
        .map_err(|error| format!("failed to pin Cargo binding trampoline: {error}"))?;
    if source.sha256() != &expected {
        return Err(format!(
            "Cargo binding trampoline does not match {AUTHORITY_CARGO_BINDING_TRAMPOLINE_SHA256_ENV}"
        ));
    }
    let bytes = source
        .authenticated_bytes()
        .map_err(|error| format!("failed to authenticate Cargo binding trampoline: {error}"))?;
    cargo_binding_trampoline::validate_v1(&bytes)?;
    source
        .seal_executable_image()
        .map_err(|error| format!("failed to seal Cargo binding trampoline: {error}"))
}

struct BackendRunContext {
    target: String,
    project: project::CargoProject,
    backend: PathBuf,
    pinned_backend: pinned_codegen_backend::PinnedCodegenBackend,
    pinned_cargo: pinned_executable::PinnedExecutable,
    pinned_rustc: PinnedRustc,
    _worker_v2: Option<worker_v2::PreparedWorkerV2Config>,
    worker_v2_identity: Option<worker_v2::WorkerV2ConfigIdentity>,
    compiler_closure_sha256: [u8; 32],
    protected_compiler_closure: Option<fe2o3_build_authority::CompilerClosureV2>,
    target_dir: project::PinnedDirectory,
    generation: generation::PreparedGeneration,
    managed_rustc_args: OsString,
    binding_wrapper_path: PathBuf,
    pinned_binding_wrapper: pinned_executable::PinnedExecutable,
    cargo_binding_trampoline: Option<pinned_executable::PinnedExecutable>,
    build_session: fe2o3_artifact_transaction::BuildSession,
    requires_locked_closure: bool,
    authorized_closure: Option<authorized_kernel_closure::AuthorizedKernelClosureV1>,
    protected_release_action: Option<ProtectedReleaseAction>,
}

struct BackendRunPreparation {
    project: project::CargoProject,
    worker_v2: Option<worker_v2::PreparedWorkerV2Config>,
    pinned_cargo: pinned_executable::PinnedExecutable,
    pinned_rustc: PinnedRustc,
    authority_backend: Option<(PathBuf, pinned_codegen_backend::PinnedCodegenBackend)>,
    protected_binding_wrapper: Option<pinned_executable::PinnedExecutable>,
    cargo_binding_trampoline: Option<pinned_executable::PinnedExecutable>,
    protected_compiler_closure: Option<fe2o3_build_authority::CompilerClosureV2>,
    authorized_closure: Option<authorized_kernel_closure::AuthorizedKernelClosureV1>,
    protected_release_action: Option<ProtectedReleaseAction>,
}

impl BackendRunContext {
    fn prepare(
        preparation: BackendRunPreparation,
        args: &[OsString],
        simulation: bool,
    ) -> Result<Self, String> {
        let BackendRunPreparation {
            project,
            worker_v2,
            pinned_cargo,
            pinned_rustc,
            authority_backend,
            protected_binding_wrapper,
            cargo_binding_trampoline,
            protected_compiler_closure,
            authorized_closure,
            protected_release_action,
        } = preparation;
        let target = amd_gpu_target(simulation);
        let target_dir = project.open_or_create_target()?;
        pinned_rustc.assert_lib_tree_unmutated()?;
        let (backend, pinned_backend) = match authority_backend {
            Some(prebuilt) => prebuilt,
            None => {
                let backend = find_or_build_backend(&target_dir, &pinned_cargo, &pinned_rustc)?;
                let pinned_backend =
                    pinned_codegen_backend::PinnedCodegenBackend::open(&backend)
                        .map_err(|error| format!("failed to pin codegen backend: {error}"))?;
                (backend, pinned_backend)
            }
        };
        pinned_rustc.assert_lib_tree_unmutated()?;
        let binding_wrapper_path = env::current_exe()
            .map_err(|error| format!("failed to locate cargo-fe2o3 executable: {error}"))?;
        let pinned_binding_wrapper = match protected_binding_wrapper {
            Some(wrapper) => wrapper,
            None => pinned_executable::PinnedExecutable::open(&binding_wrapper_path)
                .map_err(|error| format!("failed to pin cargo-fe2o3 wrapper: {error}"))?,
        };
        let protected_closure = match (
            cargo_binding_trampoline.as_ref(),
            protected_compiler_closure,
        ) {
            (Some(trampoline), Some(expected)) => {
                if pinned_cargo.sha256() == trampoline.sha256()
                    || pinned_cargo.sha256() == pinned_binding_wrapper.sha256()
                    || trampoline.sha256() == pinned_binding_wrapper.sha256()
                {
                    return Err(
                        "protected Cargo, binding trampoline, and full wrapper images must be distinct"
                            .to_owned(),
                    );
                }
                let actual = compiler_toolchain::compiler_closure_v2_from_pins(
                    pinned_cargo.sha256(),
                    trampoline.sha256(),
                    pinned_binding_wrapper.sha256(),
                    pinned_rustc.executable.sha256(),
                    pinned_rustc.lib_tree_sha256(),
                    pinned_backend.sha256(),
                )
                .map_err(|error| format!("invalid protected compiler closure: {error}"))?;
                if actual != expected {
                    return Err(
                        "retained protected compiler closure differs from release admission"
                            .to_owned(),
                    );
                }
                Some(actual)
            }
            (None, None) => None,
            _ => {
                return Err(
                    "protected compiler closure and static binding trampoline must be admitted together"
                        .to_owned(),
                );
            }
        };
        let compiler_closure_sha256 = protected_closure.map_or_else(
            || {
                compiler_toolchain::compiler_closure_sha256_v1(
                    pinned_cargo.sha256(),
                    pinned_rustc.executable.sha256(),
                    pinned_rustc.lib_tree_sha256(),
                    pinned_backend.sha256(),
                )
            },
            fe2o3_build_authority::CompilerClosureV2::identity_sha256,
        );
        let worker_v2_identity = worker_v2.as_ref().map(|config| config.identity());
        let mut cargo_configuration = project.semantic_configuration(
            args,
            &pinned_cargo,
            authorized_closure.is_some().then_some(&pinned_rustc),
        )?;
        if let Some(authorized_closure) = authorized_closure.as_ref() {
            cargo_configuration.extend_from_slice(b"fe2o3-authorized-kernel-closure-v1\0");
            cargo_configuration
                .extend_from_slice(&(authorized_closure.snapshot().len() as u64).to_le_bytes());
            cargo_configuration.extend_from_slice(authorized_closure.snapshot());
        }
        if simulation {
            cargo_configuration.extend_from_slice(b"fe2o3-cargo-simulation-v1\0");
        }
        let semantic = generation::semantic_identity(
            &target,
            &compiler_closure_sha256,
            worker_v2_identity,
            &cargo_configuration,
        )?;
        let generation = generation::PreparedGeneration::prepare(&target_dir, semantic)?;
        let backend_reference = pinned_backend
            .fixed_child_descriptor_path(BACKEND_CHILD_FD)
            .map_err(|error| format!("failed to retain pinned codegen backend: {error}"))?;
        let managed_rustc_args =
            generation::managed_rustc_args(&backend_reference, generation.token())?;
        let build_session = random_build_session()?;

        Ok(Self {
            target,
            project,
            backend,
            pinned_backend,
            pinned_cargo,
            pinned_rustc,
            _worker_v2: worker_v2,
            worker_v2_identity,
            compiler_closure_sha256,
            protected_compiler_closure: protected_closure,
            target_dir,
            generation,
            managed_rustc_args,
            binding_wrapper_path,
            pinned_binding_wrapper,
            cargo_binding_trampoline,
            build_session,
            requires_locked_closure: authorized_closure.is_some(),
            authorized_closure,
            protected_release_action,
        })
    }
}

fn run_cargo_with_backend(
    context: &mut BackendRunContext,
    command: &str,
    args: &[OsString],
    protected_release: Option<&authority_release::ProtectedReleaseAdmission>,
    simulation: Option<&SimulationCommand>,
) -> Result<(), String> {
    context.project.validate_paths()?;
    context.target_dir.validate_path("Cargo target directory")?;
    context.generation.reject_if_substituted()?;
    eprintln!(
        "cargo fe2o3 {command}: using backend {} for target {}",
        context.backend.display(),
        context.target
    );

    let mut cargo = context
        .pinned_cargo
        .command()
        .map_err(|error| format!("failed to prepare pinned Cargo executable: {error}"))?;
    let workspace_wrapper = match context.cargo_binding_trampoline.as_ref() {
        Some(trampoline) => {
            context
                .pinned_binding_wrapper
                .inherit_for_child_at(cargo.as_command_mut(), CARGO_BINDING_WRAPPER_CHILD_FD)
                .map_err(|error| {
                    format!("failed to inherit sealed cargo-fe2o3 wrapper: {error}")
                })?;
            let path = trampoline
                .fixed_child_path(CARGO_BINDING_TRAMPOLINE_CHILD_FD)
                .map_err(|error| format!("failed to retain Cargo binding trampoline: {error}"))?;
            trampoline
                .inherit_for_child_at(cargo.as_command_mut(), CARGO_BINDING_TRAMPOLINE_CHILD_FD)
                .map_err(|error| format!("failed to inherit Cargo binding trampoline: {error}"))?;
            path
        }
        None => context.binding_wrapper_path.clone(),
    };
    let mut forwarded_args = args.to_vec();
    if context.requires_locked_closure {
        let position = forwarded_args
            .iter()
            .position(|argument| argument == "--")
            .unwrap_or(forwarded_args.len());
        for required in ["--offline", "--frozen"] {
            if !forwarded_args.iter().any(|argument| argument == required) {
                forwarded_args.insert(position, OsString::from(required));
            }
        }
    }
    if command == "run" {
        let expects_envelope = context
            ._worker_v2
            .as_ref()
            .is_some_and(|config| config.envelope_mode().is_required());
        inject_application_runner(
            &context.project,
            &context.pinned_cargo,
            &context.pinned_rustc,
            context.generation.artifact_dir(),
            &mut forwarded_args,
            expects_envelope,
            context.requires_locked_closure,
        )?;
    }
    let artifact_dir = context.generation.artifact_dir();
    let capability_profile = if context
        ._worker_v2
        .as_ref()
        .and_then(worker_v2::PreparedWorkerV2Config::source_debug_profile)
        .is_some()
    {
        capability_broker::CapabilityProfileV1::S09
    } else {
        capability_broker::CapabilityProfileV1::Ordinary
    };
    let rustc_lib_tree_stat = rustix::fs::fstat(context.pinned_rustc.lib_tree_directory().file())
        .map_err(|error| {
        format!("failed to inspect retained rustc lib-tree directory: {error}")
    })?;
    let retained_object_binding_sha256 = compiler_toolchain::retained_object_binding_sha256_v1(
        &context.compiler_closure_sha256,
        rustc_lib_tree_stat.st_dev,
        rustc_lib_tree_stat.st_ino,
        rustc_lib_tree_stat.st_mode,
    );
    let config_identity = context
        .worker_v2_identity
        .map(|identity| *identity.as_bytes());
    let capability_broker = if let Some(compiler_closure) = context.protected_compiler_closure {
        let binding = capability_broker::CapabilityBindingV3::new_protected(
            capability_profile,
            config_identity,
            compiler_closure,
            retained_object_binding_sha256,
        )?;
        capability_broker::CapabilityBroker::start_protected(
            context.build_session,
            binding,
            compiler_closure,
            &context.pinned_backend,
            artifact_dir,
            &context.pinned_cargo,
        )?
    } else {
        let binding = capability_broker::CapabilityBindingV3::new(
            capability_profile,
            config_identity,
            context.compiler_closure_sha256,
            *context.pinned_rustc.executable.sha256(),
            retained_object_binding_sha256,
        )?;
        capability_broker::CapabilityBroker::start(
            context.build_session,
            binding,
            &context.pinned_backend,
            artifact_dir,
            &context.pinned_cargo,
        )?
    };
    let invocation_authorization = capability_broker.invocation_authorization();
    let pending_invocation_boundary =
        cargo_invocation_boundary::PendingCargoInvocationBoundary::start(
            &context.pinned_cargo,
            &context.pinned_binding_wrapper,
            context.cargo_binding_trampoline.as_ref(),
            invocation_authorization.clone(),
        )?;
    cargo
        .as_command_mut()
        .arg(command)
        .args(&forwarded_args)
        .current_dir(context.project.invocation_dir().child_path())
        .env_remove(HSACO_DIR_ENV)
        .env(
            capability_broker::CAPABILITY_BROKER_ENV,
            capability_broker.route(),
        )
        .env(TARGET_ENV, &context.target)
        .env("FE2O3_HOST_PASSTHROUGH", "0")
        .env("RUSTC_WRAPPER", "")
        .env("CARGO_BUILD_RUSTC_WRAPPER", "")
        .env("RUSTC_WORKSPACE_WRAPPER", workspace_wrapper)
        .env(BINDING_WRAPPER_MODE_ENV, "1")
        .env(MANAGED_RUSTC_ARGS_ENV, &context.managed_rustc_args)
        .env(
            EXPECTED_RUSTC_SHA256_ENV,
            hex_encode(context.pinned_rustc.executable.sha256()),
        )
        .env(
            EXPECTED_COMPILER_CLOSURE_SHA256_ENV,
            hex_encode(&context.compiler_closure_sha256),
        )
        .env(BUILD_SESSION_ENV, context.build_session.to_hex());
    configure_simulation_build_environment(cargo.as_command_mut(), simulation.is_some());
    if let Some(action) = context.protected_release_action {
        cargo
            .as_command_mut()
            .env(PROTECTED_RELEASE_ACTION_ENV, action.environment_value());
    } else {
        cargo
            .as_command_mut()
            .env_remove(PROTECTED_RELEASE_ACTION_ENV);
    }
    if context.requires_locked_closure {
        // Authority builds do not admit unpinned C tools, ROCm headers, or native libraries.
        cargo.as_command_mut().env("FE2O3_HIP_SYS_DISABLE", "1");
    }
    cargo
        .as_command_mut()
        .env_remove(AUTHORITY_CARGO_SHA256_ENV)
        .env_remove(AUTHORITY_RUSTC_SHA256_ENV)
        .env_remove(AUTHORITY_RUSTC_PATH_ENV)
        .env_remove(AUTHORITY_RUSTC_RUNTIME_SHA256_ENV)
        .env_remove(AUTHORITY_BACKEND_SHA256_ENV)
        .env_remove(AUTHORITY_CARGO_BINDING_TRAMPOLINE_PATH_ENV)
        .env_remove(AUTHORITY_CARGO_BINDING_TRAMPOLINE_SHA256_ENV);
    remove_dynamic_loader_environment(cargo.as_command_mut());
    context.pinned_rustc.assert_lib_tree_unmutated()?;
    configure_pinned_rustc_child(cargo.as_command_mut(), &context.pinned_rustc)?;
    cargo.as_command_mut().env(
        "LD_LIBRARY_PATH",
        format!("/proc/self/fd/{RUSTC_LIBRARY_CHILD_FD}"),
    );
    match context.worker_v2_identity {
        Some(identity) => {
            cargo
                .as_command_mut()
                .env(worker_v2::WORKER_V2_EXPECTED_ID_ENV, identity.to_hex());
        }
        None => {
            cargo
                .as_command_mut()
                .env_remove(worker_v2::WORKER_V2_EXPECTED_ID_ENV);
        }
    }
    if let Some(admission) = protected_release {
        admission.configure_descendant(cargo.as_command_mut());
    }
    pending_invocation_boundary.configure_child(cargo.as_command_mut());
    let mut cargo_child = cargo
        .spawn()
        .map_err(|error| format!("failed to run pinned Cargo: {error}"))?;
    let invocation_boundary =
        match pending_invocation_boundary.complete(cargo_child.id(), invocation_authorization) {
            Ok(boundary) => boundary,
            Err(error) => {
                let _ = cargo_child.kill();
                let cleanup_result = cargo_child
                    .wait()
                    .map(|_| ())
                    .map_err(|cleanup| format!("failed to reap rejected Cargo child: {cleanup}"));
                drop(capability_broker);
                let lib_tree_result = context.pinned_rustc.revalidate_lib_tree();
                let closure_result = context
                    .authorized_closure
                    .as_ref()
                    .map_or(Ok(()), |closure| closure.revalidate());
                return aggregate_post_spawn_results(
                    Err(error),
                    [
                        ("Cargo child cleanup", cleanup_result),
                        ("rustc runtime-tree revalidation", lib_tree_result),
                        ("authorized kernel-closure revalidation", closure_result),
                    ],
                );
            }
        };
    let status = cargo_child.wait();
    let boundary_result = invocation_boundary.finish();
    drop(capability_broker);
    let lib_tree_result = context.pinned_rustc.revalidate_lib_tree();
    let closure_result = context
        .authorized_closure
        .as_ref()
        .map_or(Ok(()), |closure| closure.revalidate());
    let cargo_result = match status {
        Ok(status) if status.success() => Ok(()),
        Ok(status) => Err(format!("cargo {command} failed with status {status}")),
        Err(error) => Err(format!("failed to run cargo: {error}")),
    };
    aggregate_post_spawn_results(
        cargo_result,
        [
            ("Cargo invocation-boundary finish", boundary_result),
            ("rustc runtime-tree revalidation", lib_tree_result),
            ("authorized kernel-closure revalidation", closure_result),
        ],
    )?;

    context.project.validate_paths()?;
    context.target_dir.validate_path("Cargo target directory")?;
    context.generation.reject_if_substituted()?;
    let canonical_kir = simulation
        .map(|_| simulation_capture::consume_exactly_one(context.generation.artifact_dir()))
        .transpose()?;
    context.generation.commit()?;
    if let (Some(simulation), Some(canonical_kir)) = (simulation, canonical_kir) {
        let status = fe2o3_kir_sim_cli::run_captured_kir_v6_with_bound_request(
            &canonical_kir,
            &simulation.request,
            simulation.request_identity,
            simulation.output.as_deref(),
        );
        if status != ExitCode::SUCCESS {
            return Err(SIMULATION_FAILURE_ALREADY_REPORTED.to_owned());
        }
    }
    Ok(())
}

fn configure_simulation_build_environment(command: &mut Command, selected: bool) {
    if selected {
        command
            .env(worker_v2::CODEGEN_PIPELINE_ENV, SIMULATION_PIPELINE)
            .env(SIMULATION_MODE_ENV, "1")
            .env("FE2O3_HIP_SYS_DISABLE", "1");
    } else {
        command
            .env_remove(SIMULATION_MODE_ENV)
            .env_remove("FE2O3_HIP_SYS_DISABLE");
    }
}

fn aggregate_post_spawn_results<const N: usize>(
    primary: Result<(), String>,
    checks: [(&str, Result<(), String>); N],
) -> Result<(), String> {
    let mut failure = primary.err();
    for (label, result) in checks {
        if let Err(error) = result {
            match failure.as_mut() {
                Some(primary) => primary.push_str(&format!("; {label} also failed: {error}")),
                None => failure = Some(error),
            }
        }
    }
    failure.map_or(Ok(()), Err)
}

fn inject_application_runner(
    project: &project::CargoProject,
    pinned_cargo: &pinned_executable::PinnedExecutable,
    pinned_rustc: &PinnedRustc,
    artifact_dir: &project::PinnedDirectory,
    args: &mut Vec<OsString>,
    expects_envelope: bool,
    authority: bool,
) -> Result<(), String> {
    let target = match selected_run_target(args)? {
        Some(target) => target,
        None => match configured_run_target(
            project,
            pinned_cargo,
            authority.then_some(pinned_rustc),
            args,
        )? {
            Some(target) => target,
            None if authority => {
                return Err(
                    "cargo fe2o3 authority run requires an explicit --target or reviewed build.target"
                        .to_owned(),
                );
            }
            None => host_rustc_target()?,
        },
    };
    if !target
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err(format!(
            "unsupported Cargo run target for runner isolation: {target:?}"
        ));
    }
    let original_runner = resolve_original_runner(
        project,
        pinned_cargo,
        authority.then_some(pinned_rustc),
        args,
        &target,
    )?;
    reject_recursive_runner(&original_runner)?;
    inject_application_runner_config(
        args,
        &target,
        artifact_dir,
        &original_runner,
        expects_envelope,
    )
}

fn inject_application_runner_config(
    args: &mut Vec<OsString>,
    target: &str,
    artifact_dir: &project::PinnedDirectory,
    original_runner: &[OsString],
    expects_envelope: bool,
) -> Result<(), String> {
    let executable = env::current_exe()
        .map_err(|error| format!("failed to locate cargo-fe2o3 runner executable: {error}"))?;
    let executable = executable.to_str().ok_or_else(|| {
        "cargo fe2o3 run requires a UTF-8 cargo-fe2o3 executable path for Cargo runner configuration"
            .to_string()
    })?;
    let (artifact_device, artifact_inode) = artifact_dir.identity_parts();
    let mut runner = vec![
        executable.to_string(),
        INTERNAL_RUNNER_ARG.to_string(),
        application_handoff::RUNNER_CONTEXT_VERSION.to_string(),
        hex_encode(os_bytes(artifact_dir.display_path().as_os_str())),
        artifact_device.to_string(),
        artifact_inode.to_string(),
        if expects_envelope {
            application_handoff::RUNNER_EXPECTS_ENVELOPE.to_string()
        } else {
            application_handoff::RUNNER_EXPECTS_NO_ENVELOPE.to_string()
        },
        original_runner.len().to_string(),
    ];
    runner.extend(
        original_runner
            .iter()
            .map(|argument| hex_encode(os_bytes(argument))),
    );
    let runner = serde_json::to_string(&runner)
        .map_err(|error| format!("failed to encode Cargo runner configuration: {error}"))?;
    let config = OsString::from(format!("target.{target}.runner={runner}"));
    let insert_at = args
        .iter()
        .position(|argument| argument == "--")
        .unwrap_or(args.len());
    args.insert(insert_at, OsString::from("--config"));
    args.insert(insert_at + 1, config);
    Ok(())
}

fn configured_run_target(
    project: &project::CargoProject,
    pinned_cargo: &pinned_executable::PinnedExecutable,
    authority_rustc: Option<&PinnedRustc>,
    args: &[OsString],
) -> Result<Option<String>, String> {
    let Some(value) =
        project.cargo_config_value(args, "build.target", pinned_cargo, authority_rustc)?
    else {
        return Ok(None);
    };
    let targets = match value {
        serde_json::Value::String(target) => vec![target],
        serde_json::Value::Array(targets) => targets
            .into_iter()
            .map(|target| {
                target.as_str().map(str::to_string).ok_or_else(|| {
                    "Cargo build.target array contains a non-string value".to_string()
                })
            })
            .collect::<Result<Vec<_>, _>>()?,
        _ => {
            return Err("Cargo build.target must be a string or string array".to_string());
        }
    };
    match targets.as_slice() {
        [] => Err("Cargo build.target may not be empty".to_string()),
        [target] if !target.is_empty() => Ok(Some(target.clone())),
        [_] => Err("Cargo build.target may not contain an empty target".to_string()),
        _ => Err("cargo fe2o3 run requires exactly one configured build target".to_string()),
    }
}

fn resolve_original_runner(
    project: &project::CargoProject,
    pinned_cargo: &pinned_executable::PinnedExecutable,
    authority_rustc: Option<&PinnedRustc>,
    args: &[OsString],
    target: &str,
) -> Result<Vec<OsString>, String> {
    let environment_name = target_runner_environment_name(target);
    if let Some(environment_runner) = env::var_os(&environment_name)
        && environment_runner.to_str().is_none()
    {
        if has_invocation_config(args) {
            return Err(format!(
                "cannot determine precedence for non-UTF-8 {environment_name} with --config; refusing to bypass a Cargo runner"
            ));
        }
        return parse_runner_bytes(os_bytes(&environment_runner), &environment_name);
    }

    let key = format!("target.{target}.runner");
    if let Some(value) = project.cargo_config_value(args, &key, pinned_cargo, authority_rustc)? {
        return parse_runner_value(value, &key);
    }

    if let Some(serde_json::Value::Object(targets)) =
        project.cargo_config_value(args, "target", pinned_cargo, authority_rustc)?
        && targets.iter().any(|(selector, value)| {
            selector.starts_with("cfg(")
                && value
                    .as_object()
                    .is_some_and(|table| table.contains_key("runner"))
        })
    {
        return Err(format!(
            "cannot safely resolve cfg-selected Cargo runner for target {target}; configure target.{target}.runner explicitly"
        ));
    }
    Ok(Vec::new())
}

fn target_runner_environment_name(target: &str) -> String {
    let normalized = target
        .bytes()
        .map(|byte| match byte {
            b'a'..=b'z' => (byte - b'a' + b'A') as char,
            b'A'..=b'Z' | b'0'..=b'9' => byte as char,
            _ => '_',
        })
        .collect::<String>();
    format!("CARGO_TARGET_{normalized}_RUNNER")
}

fn has_invocation_config(args: &[OsString]) -> bool {
    args.iter()
        .take_while(|argument| *argument != "--")
        .any(|argument| argument == "--config" || os_bytes(argument).starts_with(b"--config="))
}

fn parse_runner_value(value: serde_json::Value, source: &str) -> Result<Vec<OsString>, String> {
    let runner = match value {
        serde_json::Value::String(value) => value
            .split_whitespace()
            .map(OsString::from)
            .collect::<Vec<_>>(),
        serde_json::Value::Array(values) => values
            .into_iter()
            .map(|value| {
                value.as_str().map(OsString::from).ok_or_else(|| {
                    format!("Cargo runner `{source}` contains a non-string argument")
                })
            })
            .collect::<Result<Vec<_>, _>>()?,
        _ => {
            return Err(format!(
                "Cargo runner `{source}` must be a string or string array"
            ));
        }
    };
    if runner.is_empty() || runner[0].is_empty() {
        return Err(format!("Cargo runner `{source}` may not be empty"));
    }
    Ok(runner)
}

fn parse_runner_bytes(value: &[u8], source: &str) -> Result<Vec<OsString>, String> {
    let runner = value
        .split(|byte| byte.is_ascii_whitespace())
        .filter(|argument| !argument.is_empty())
        .map(|argument| os_string(argument.to_vec()))
        .collect::<Result<Vec<_>, _>>()?;
    if runner.is_empty() {
        return Err(format!("Cargo runner `{source}` may not be empty"));
    }
    Ok(runner)
}

fn reject_recursive_runner(runner: &[OsString]) -> Result<(), String> {
    let Some(program) = runner.first() else {
        return Ok(());
    };
    let current = env::current_exe()
        .map_err(|error| format!("failed to identify cargo-fe2o3 executable: {error}"))?;
    if program == current.as_os_str()
        || program == current.file_name().unwrap_or_default()
        || resolve_runner_program(program).is_some_and(|path| {
            same_executable_file(&path, &current)
                || std::fs::canonicalize(path).ok() == std::fs::canonicalize(&current).ok()
        })
    {
        return Err("refusing recursive cargo-fe2o3 Cargo runner configuration".to_string());
    }
    Ok(())
}

#[cfg(unix)]
fn same_executable_file(left: &Path, right: &Path) -> bool {
    use std::os::unix::fs::MetadataExt;

    let Ok(left) = std::fs::metadata(left) else {
        return false;
    };
    let Ok(right) = std::fs::metadata(right) else {
        return false;
    };
    left.is_file() && right.is_file() && left.dev() == right.dev() && left.ino() == right.ino()
}

#[cfg(not(unix))]
fn same_executable_file(_left: &Path, _right: &Path) -> bool {
    false
}

fn resolve_runner_program(program: &OsStr) -> Option<PathBuf> {
    let path = Path::new(program);
    if path.is_absolute() || os_bytes(program).contains(&b'/') {
        return Some(if path.is_absolute() {
            path.to_path_buf()
        } else {
            env::current_dir().ok()?.join(path)
        });
    }
    env::var_os("PATH")
        .into_iter()
        .flat_map(|paths| env::split_paths(&paths).collect::<Vec<_>>())
        .map(|directory| directory.join(path))
        .find(|candidate| candidate.is_file())
}

fn selected_run_target(args: &[OsString]) -> Result<Option<String>, String> {
    let mut selected = None;
    let mut index = 0;
    while index < args.len() {
        let argument = &args[index];
        if argument == "--" {
            break;
        }
        let value = if argument == "--target" {
            index += 1;
            Some(
                args.get(index)
                    .ok_or_else(|| "--target requires an argument".to_string())?
                    .clone(),
            )
        } else {
            split_joined_os_option(argument, "--target")
        };
        if let Some(value) = value {
            let value = value
                .to_str()
                .filter(|value| !value.is_empty())
                .ok_or_else(|| "Cargo run target must be non-empty UTF-8".to_string())?;
            if selected.replace(value.to_string()).is_some() {
                return Err("--target may be specified only once".to_string());
            }
        }
        index += 1;
    }
    Ok(selected)
}

fn split_joined_os_option(argument: &OsStr, option: &str) -> Option<OsString> {
    let bytes = os_bytes(argument);
    let prefix = option.as_bytes();
    if !bytes.starts_with(prefix) || bytes.get(prefix.len()) != Some(&b'=') {
        return None;
    }
    os_string(bytes[prefix.len() + 1..].to_vec()).ok()
}

fn host_rustc_target() -> Result<String, String> {
    let rustc = env::var_os("RUSTC").unwrap_or_else(|| OsString::from("rustc"));
    let output = Command::new(rustc)
        .arg("-vV")
        .output()
        .map_err(|error| format!("failed to query rustc host target: {error}"))?;
    if !output.status.success() {
        return Err(format!("rustc -vV failed with status {}", output.status));
    }
    let output = String::from_utf8(output.stdout)
        .map_err(|_| "rustc -vV output was not UTF-8".to_string())?;
    output
        .lines()
        .find_map(|line| line.strip_prefix("host: "))
        .map(str::to_string)
        .ok_or_else(|| "rustc -vV output did not contain a host target".to_string())
}

fn run_application_boundary_frontend(args: &[OsString]) -> ExitCode {
    match application_supervisor::run_frontend(args) {
        Ok(status) => ExitCode::from(binding_wrapper::exit_code(status)),
        Err(error) => {
            eprintln!("cargo-fe2o3 application runner: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run_application_supervisor(args: &[OsString]) -> ExitCode {
    match application_supervisor::run_supervisor(
        args,
        run_application_boundary_result,
        application_handoff::application_cleanup_is_pending,
        application_handoff::finish_application_cleanup_supervisor,
    ) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("cargo-fe2o3 application supervisor: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run_application_boundary_result(args: &[OsString]) -> Result<std::process::ExitStatus, String> {
    if args.len() < 7 {
        return Err(
            "runner requires a generation context, original-runner count, and application"
                .to_string(),
        );
    }
    let application_timeouts = match args[0].to_str() {
        Some(application_handoff::RUNNER_CONTEXT_VERSION) => {
            application_handoff::ApplicationTimeouts::PRODUCTION
        }
        #[cfg(feature = "worker-v2-fault-injection-test-only")]
        Some(application_handoff::RUNNER_SHORT_TIMEOUT_TEST_CONTEXT_VERSION) => {
            application_handoff::ApplicationTimeouts::TEST_SHORT
        }
        #[cfg(feature = "worker-v2-fault-injection-test-only")]
        Some(application_handoff::RUNNER_SCHEDULER_TOLERANT_TEST_CONTEXT_VERSION) => {
            application_handoff::ApplicationTimeouts::TEST_SCHEDULER_TOLERANT
        }
        _ => {
            return Err(format!(
                "unsupported application runner context {:?}",
                args[0]
            ));
        }
    };
    let artifact_path = PathBuf::from(hex_decode_os(&args[1])?);
    let artifact_device = parse_runner_u64(&args[2], "artifact directory device")?;
    let artifact_inode = parse_runner_u64(&args[3], "artifact directory inode")?;
    let artifact_dir = application_handoff::open_expected_generation(
        artifact_path,
        artifact_device,
        artifact_inode,
    )?;
    let expects_envelope = match args[4].to_str() {
        Some(application_handoff::RUNNER_EXPECTS_ENVELOPE) => true,
        Some(application_handoff::RUNNER_EXPECTS_NO_ENVELOPE) => false,
        _ => {
            return Err(format!(
                "invalid application envelope expectation {:?}",
                args[4]
            ));
        }
    };
    let runner_count = args[5]
        .to_str()
        .and_then(|value| value.parse::<usize>().ok())
        .ok_or_else(|| format!("invalid original runner argument count {:?}", args[5]))?;
    let application_index = 6_usize
        .checked_add(runner_count)
        .ok_or_else(|| "original runner argument count overflowed".to_string())?;
    let application = args
        .get(application_index)
        .ok_or_else(|| "runner argument count does not leave an application".to_string())?;
    let original_runner = args[6..application_index]
        .iter()
        .map(|argument| hex_decode_os(argument))
        .collect::<Result<Vec<_>, _>>()?;
    if original_runner
        .first()
        .is_some_and(|program| program.is_empty())
    {
        return Err("original Cargo runner executable may not be empty".to_string());
    }
    reject_recursive_runner(&original_runner)?;

    let handoff = application_handoff::PinnedApplicationEnvelope::discover(&artifact_dir)?;
    if expects_envelope && handoff.is_none() {
        return Err("Cargo runner expected a canonical Worker V2 envelope, but none exists".into());
    }
    if !expects_envelope && handoff.is_some() {
        return Err(
            "Cargo runner did not expect a Worker V2 envelope for this application build".into(),
        );
    }
    if let Some(mut handoff) = handoff {
        if !original_runner.is_empty() {
            return Err(
                "Worker V2 application descriptor handoff does not permit an intermediate Cargo runner"
                    .to_string(),
            );
        }
        let current_dir = env::current_dir()
            .map_err(|error| format!("failed to resolve application runner directory: {error}"))?;
        let application_path =
            binding_wrapper::resolve_command_executable(application, &current_dir)
                .map_err(|error| format!("failed to resolve application executable: {error}"))?;
        let pinned_application = pinned_executable::PinnedExecutable::open(&application_path)
            .map_err(|error| format!("failed to pin application executable: {error}"))?;
        let sealed_application = pinned_application
            .seal_static_application()
            .map_err(|error| format!("failed to seal application runtime image: {error}"))?;
        let application_identity = sealed_application.identity();
        let mut child = sealed_application
            .command()
            .map_err(|error| format!("failed to prepare sealed application: {error}"))?;
        child.args(&args[application_index + 1..]);
        scrub_application_environment(child.as_command_mut());
        let pending_ack = handoff.configure_child_with_timeouts(
            child.as_command_mut(),
            application_identity,
            sealed_application.identity_v3(),
            application_timeouts,
        )?;
        let mut process = child
            .as_command_mut()
            .spawn()
            .map_err(|error| format!("failed to launch pinned Cargo application: {error}"))?;
        let active_handoff = match pending_ack.await_after_spawn(&mut process) {
            Ok(active_handoff) => active_handoff,
            Err(failure) => {
                let (error, cleanup) = failure.into_parts();
                drop(handoff);
                drop(artifact_dir);
                return match application_handoff::terminate_application_group(process, cleanup) {
                    Ok(_) => Err(error),
                    Err(containment) => Err(format!(
                        "{error}; application containment failed: {containment}"
                    )),
                };
            }
        };
        if let Err(error) = application_handoff::wait_for_application_exit_without_reaping(&process)
        {
            drop(handoff);
            drop(artifact_dir);
            return match application_handoff::terminate_application_group(
                process,
                active_handoff.into_cleanup(),
            ) {
                Ok(_) => Err(error),
                Err(containment) => Err(format!(
                    "{error}; application containment failed: {containment}"
                )),
            };
        }
        // The application retains its currentness token through all descriptor-dependent work.
        // Observe its exit without reaping before reacquiring the runner's token, avoiding a
        // scheduler race while preserving the leader identity for process-group containment.
        if let Err(error) = handoff.validate_retained_currentness() {
            drop(handoff);
            drop(artifact_dir);
            return match application_handoff::terminate_application_group(
                process,
                active_handoff.into_cleanup(),
            ) {
                Ok(_) => Err(error),
                Err(containment) => Err(format!(
                    "{error}; application containment failed: {containment}"
                )),
            };
        }
        let cleanup = active_handoff.into_cleanup();
        drop(handoff);
        drop(artifact_dir);
        let status = application_handoff::wait_and_contain_application_group(process, cleanup)?;
        return Ok(status);
    }

    let mut child = if let Some(program) = original_runner.first() {
        let mut command = Command::new(program);
        command.args(&original_runner[1..]);
        command.arg(application);
        command.args(&args[application_index + 1..]);
        command
    } else {
        let mut command = Command::new(application);
        command.args(&args[application_index + 1..]);
        command
    };
    scrub_application_environment(&mut child);
    // Non-Worker typed kernels still load their compiler-produced image by name. Give the
    // application only the already pinned generation directory rather than reopening its path or
    // restoring any ambient build environment.
    application_exec::configure_closed_descriptor_baseline(&mut child);
    artifact_dir.replace_for_child_at(&mut child, ARTIFACT_CHILD_FD)?;
    child.env(HSACO_DIR_ENV, format!("/proc/self/fd/{ARTIFACT_CHILD_FD}"));
    child
        .status()
        .map_err(|error| format!("failed to launch Cargo runner/application: {error}"))
}

fn scrub_application_environment(child: &mut Command) {
    // The application boundary has no ambient-environment allowlist. A typed Worker V2 handoff
    // adds only its five explicit values after this reset.
    child.env_clear();
}

fn parse_runner_u64(value: &OsStr, kind: &str) -> Result<u64, String> {
    value
        .to_str()
        .and_then(|value| value.parse::<u64>().ok())
        .ok_or_else(|| format!("invalid {kind} {value:?}"))
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(HEX[(byte >> 4) as usize] as char);
        encoded.push(HEX[(byte & 0x0f) as usize] as char);
    }
    encoded
}

fn hex_decode_os(value: &OsStr) -> Result<OsString, String> {
    let bytes = os_bytes(value);
    if !bytes.len().is_multiple_of(2) {
        return Err(format!("invalid encoded Cargo runner argument {value:?}"));
    }
    let decoded = bytes
        .chunks_exact(2)
        .map(|pair| {
            let high = hex_nibble(pair[0])?;
            let low = hex_nibble(pair[1])?;
            Ok((high << 4) | low)
        })
        .collect::<Result<Vec<_>, String>>()?;
    os_string(decoded)
}

fn hex_nibble(byte: u8) -> Result<u8, String> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        _ => Err("Cargo runner argument contains invalid hexadecimal".to_string()),
    }
}

#[cfg(unix)]
fn os_bytes(value: &OsStr) -> &[u8] {
    use std::os::unix::ffi::OsStrExt;
    value.as_bytes()
}

#[cfg(unix)]
fn os_string(value: Vec<u8>) -> Result<OsString, String> {
    use std::os::unix::ffi::OsStringExt;
    Ok(OsString::from_vec(value))
}

#[cfg(not(unix))]
fn os_bytes(value: &OsStr) -> &[u8] {
    value
        .to_str()
        .expect("environment names are UTF-8 off Unix")
        .as_bytes()
}

#[cfg(not(unix))]
fn os_string(value: Vec<u8>) -> Result<OsString, String> {
    String::from_utf8(value)
        .map(OsString::from)
        .map_err(|_| "Cargo option value is not UTF-8".to_string())
}

fn random_build_session() -> Result<fe2o3_artifact_transaction::BuildSession, String> {
    if non_production_reproduction::enabled() {
        return Ok(fe2o3_artifact_transaction::BuildSession::from_bytes(
            non_production_reproduction::deterministic_16(b"build-session"),
        ));
    }
    for _ in 0..8 {
        let mut bytes = [0_u8; 16];
        std::fs::File::open("/dev/urandom")
            .and_then(|mut source| source.read_exact(&mut bytes))
            .map_err(|error| format!("failed to obtain a build-session nonce: {error}"))?;
        let session = fe2o3_artifact_transaction::BuildSession::from_bytes(bytes);
        if session != fe2o3_artifact_transaction::BuildSession::DIRECT {
            return Ok(session);
        }
    }
    Err("failed to obtain a nonzero build-session nonce".to_string())
}

fn reject_preexisting_compiler_environment() -> Result<(), String> {
    for variable in COMPILER_SELECTION_ENVIRONMENT {
        if let Some(value) = env::var_os(variable) {
            if variable.ends_with("WRAPPER") {
                return Err(format!(
                    "cargo fe2o3 cannot compose its binding-identity wrapper with preexisting {variable}={value:?}"
                ));
            }
            return Err(format!(
                "cargo fe2o3 rejects preexisting compiler selection {variable}={value:?}"
            ));
        }
    }
    Ok(())
}

fn reject_configured_compiler_selection(
    project: &project::CargoProject,
    args: &[OsString],
    pinned_cargo: &pinned_executable::PinnedExecutable,
    authority_rustc: Option<&PinnedRustc>,
    authority: bool,
) -> Result<(), String> {
    let authority_rustc =
        if authority {
            Some(authority_rustc.ok_or_else(|| {
                "authority config query has no independently pinned rustc".to_owned()
            })?)
        } else {
            None
        };
    for key in [
        "build.rustc",
        "build.rustc-wrapper",
        "build.rustc-workspace-wrapper",
    ] {
        if let Some(value) = project.cargo_config_value(args, key, pinned_cargo, authority_rustc)? {
            if key.ends_with("wrapper") {
                return Err(format!(
                    "cargo fe2o3 cannot compose its binding-identity wrapper with configured {key}={value}"
                ));
            }
            return Err(format!(
                "cargo fe2o3 rejects configured compiler selection {key}={value}"
            ));
        }
    }
    if let Some(serde_json::Value::Object(configured)) =
        project.cargo_config_value(args, "env", pinned_cargo, authority_rustc)?
    {
        for name in configured.keys() {
            if is_dynamic_loader_environment_name(OsStr::new(name)) {
                return Err(format!(
                    "cargo fe2o3 rejects configured dynamic-loader environment env.{name}"
                ));
            }
        }
    }
    Ok(())
}

fn require_protected_authority_launch(
    protected_release: Option<&authority_release::ProtectedReleaseAdmission>,
) -> Result<(), String> {
    if protected_release.is_some() {
        return Ok(());
    }
    if cfg!(debug_assertions)
        && env::var_os(NON_PRODUCTION_AUTHORITY_VALIDATION_ENV).as_deref() == Some(OsStr::new("1"))
    {
        eprintln!(
            "cargo fe2o3: non-production unprotected authority validation only; no protected release-launch claim"
        );
        return Ok(());
    }
    Err(
        "cargo fe2o3 authority release requires a protected pre-exec launcher/image contract; this build has no admitted release launcher"
            .to_owned(),
    )
}

fn reject_authority_environment_overrides(args: &[OsString]) -> Result<(), String> {
    if has_invocation_config(args) {
        return Err(
            "cargo fe2o3 authority build rejects command-line --config before admission".to_owned(),
        );
    }
    for (name, value) in env::vars_os() {
        let bytes = os_bytes(&name);
        if bytes.starts_with(b"RUSTUP_") {
            return Err(format!(
                "cargo fe2o3 authority build rejects rustup selection channel {name:?}={value:?}"
            ));
        }
        if bytes.starts_with(b"CARGO_REGISTRIES_")
            || bytes.starts_with(b"CARGO_REGISTRY_")
            || bytes.starts_with(b"CARGO_CREDENTIAL")
            || bytes.starts_with(b"CARGO_HTTP_")
            || bytes.starts_with(b"CARGO_NET_")
        {
            return Err(format!(
                "cargo fe2o3 authority build rejects pre-admission helper/configuration channel {name:?}={value:?}"
            ));
        }
        if is_authority_tool_override_environment_name(&name) {
            return Err(format!(
                "cargo fe2o3 authority build rejects tool override {name:?}={value:?}"
            ));
        }
    }
    Ok(())
}

fn reject_authority_config_overrides(
    project: &project::CargoProject,
    args: &[OsString],
    pinned_cargo: &pinned_executable::PinnedExecutable,
    pinned_rustc: &PinnedRustc,
) -> Result<(), String> {
    for key in [
        "source",
        "registries",
        "registry",
        "credential-alias",
        "net",
        "http",
    ] {
        if let Some(value) =
            project.cargo_config_value(args, key, pinned_cargo, Some(pinned_rustc))?
        {
            return Err(format!(
                "cargo fe2o3 authority build rejects configured pre-admission Cargo {key}={value}"
            ));
        }
    }
    match project.cargo_config_value(args, "target", pinned_cargo, Some(pinned_rustc))? {
        Some(serde_json::Value::Object(targets)) => {
            for (target, configuration) in targets {
                let serde_json::Value::Object(configuration) = configuration else {
                    return Err(format!(
                        "cargo fe2o3 authority build cannot inspect configured target.{target}"
                    ));
                };
                for key in ["linker", "runner"] {
                    if let Some(value) = configuration.get(key) {
                        return Err(format!(
                            "cargo fe2o3 authority build rejects configured target.{target}.{key}={value}"
                        ));
                    }
                }
            }
        }
        Some(_) => {
            return Err(
                "cargo fe2o3 authority build cannot inspect configured target table".to_owned(),
            );
        }
        None => {}
    }
    Ok(())
}

fn authority_rustc_sha256_from_environment() -> Result<[u8; 32], String> {
    authority_sha256_from_environment(AUTHORITY_RUSTC_SHA256_ENV)
}

fn authority_rustc_runtime_sha256_from_environment() -> Result<[u8; 32], String> {
    authority_sha256_from_environment(AUTHORITY_RUSTC_RUNTIME_SHA256_ENV)
}

fn authority_cargo_sha256_from_environment() -> Result<[u8; 32], String> {
    authority_sha256_from_environment(AUTHORITY_CARGO_SHA256_ENV)
}

fn authority_backend_sha256_from_environment() -> Result<[u8; 32], String> {
    authority_sha256_from_environment(AUTHORITY_BACKEND_SHA256_ENV)
}

fn authority_sha256_from_environment(name: &'static str) -> Result<[u8; 32], String> {
    let value =
        env::var_os(name).ok_or_else(|| format!("cargo fe2o3 authority build requires {name}"))?;
    let encoded = os_bytes(&value);
    if encoded.len() != 64 {
        return Err(format!(
            "{name} must be exactly 64 lowercase hexadecimal bytes"
        ));
    }
    let mut digest = [0_u8; 32];
    for (output, pair) in digest.iter_mut().zip(encoded.chunks_exact(2)) {
        let decode = |byte| match byte {
            b'0'..=b'9' => Some(byte - b'0'),
            b'a'..=b'f' => Some(byte - b'a' + 10),
            _ => None,
        };
        let Some(high) = decode(pair[0]) else {
            return Err(format!(
                "{name} must be exactly 64 lowercase hexadecimal bytes"
            ));
        };
        let Some(low) = decode(pair[1]) else {
            return Err(format!(
                "{name} must be exactly 64 lowercase hexadecimal bytes"
            ));
        };
        *output = (high << 4) | low;
    }
    if digest == [0; 32] {
        return Err(format!("{name} may not be zero"));
    }
    Ok(digest)
}

fn is_authority_tool_override_environment_name(name: &OsStr) -> bool {
    let name = os_bytes(name);
    name.starts_with(b"CARGO_TARGET_")
        && [b"_LINKER".as_slice(), b"_RUNNER"]
            .iter()
            .any(|suffix| name.ends_with(suffix))
}

fn reject_dynamic_loader_environment() -> Result<(), String> {
    for (name, value) in env::vars_os() {
        if is_dynamic_loader_environment_name(&name) {
            return Err(format!(
                "cargo fe2o3 rejects dynamic-loader injection variable {name:?}={value:?}"
            ));
        }
    }
    Ok(())
}

fn scrub_process_dynamic_loader_environment() {
    let names = env::vars_os()
        .map(|(name, _)| name)
        .filter(|name| is_dynamic_loader_environment_name(name))
        .collect::<Vec<_>>();
    for name in names {
        // SAFETY: cargo-fe2o3 performs this one-time environment normalization before starting
        // any worker or supervisor threads. The variables stay absent for the process lifetime.
        unsafe { env::remove_var(name) };
    }
}

pub(crate) fn is_dynamic_loader_injection_environment_name(name: &OsStr) -> bool {
    is_dynamic_loader_environment_name(name)
}

pub(crate) fn is_dynamic_loader_environment_name(name: &OsStr) -> bool {
    let name = os_bytes(name);
    name.starts_with(b"LD_") || name.starts_with(b"DYLD_") || name == b"GLIBC_TUNABLES"
}

pub(crate) fn remove_dynamic_loader_environment(command: &mut Command) {
    for (name, _) in env::vars_os() {
        if is_dynamic_loader_environment_name(&name) {
            command.env_remove(name);
        }
    }
    for name in [
        "LD_PRELOAD",
        "LD_AUDIT",
        "LD_LIBRARY_PATH",
        "GLIBC_TUNABLES",
    ] {
        command.env_remove(name);
    }
}

fn find_or_build_backend(
    target_dir: &project::PinnedDirectory,
    pinned_cargo: &pinned_executable::PinnedExecutable,
    pinned_rustc: &PinnedRustc,
) -> Result<PathBuf, String> {
    if let Some(path) = env::var_os(BACKEND_ENV) {
        let path = PathBuf::from(path);
        if path.is_file() {
            return Ok(path);
        }
        return Err(format!(
            "{BACKEND_ENV} points to {}, but that file does not exist",
            path.display()
        ));
    }

    let source_root = fe2o3_source_root()?;
    let cargo_fe2o3_executable = env::current_exe()
        .map_err(|error| format!("failed to locate running cargo-fe2o3 executable: {error}"))?;
    let cargo_fe2o3_sha256 = fe2o3_process_identity::measure_executable_sha256_v3(
        &cargo_fe2o3_executable,
    )
    .map_err(|error| format!("failed to measure running cargo-fe2o3 executable: {error}"))?;
    let backend_target = target_dir.open_or_create_child(
        ".fe2o3-backend-build-v1",
        "isolated codegen-backend build directory",
    )?;
    let backend = dylib_path(backend_target.display_path());
    eprintln!("building rustc-codegen-fe2o3 backend...");
    let mut command = pinned_cargo
        .command()
        .map_err(|error| format!("failed to prepare pinned Cargo executable: {error}"))?;
    command
        .as_command_mut()
        .args(["build", "--manifest-path"])
        .arg(source_root.join("Cargo.toml"))
        .args(["--target-dir"])
        .arg(backend_target.fixed_child_path(BACKEND_BUILD_CHILD_FD)?)
        .args(["-p", "rustc-codegen-fe2o3"])
        .current_dir(&source_root)
        // The backend is an internal compiler plugin, not a debuggable user artifact. Limited
        // line-table information keeps the pinned image and its bounded hashing work stable as
        // target-neutral analysis dependencies grow.
        .env("CARGO_PROFILE_DEV_DEBUG", "1")
        .env_remove("RUSTFLAGS")
        .env_remove("CARGO_ENCODED_RUSTFLAGS")
        .env_remove("CARGO_TARGET_DIR")
        .env_remove("RUSTC_WRAPPER")
        .env_remove("RUSTC_WORKSPACE_WRAPPER")
        .env(
            "FE2O3_BUILD_CARGO_FE2O3_EXECUTABLE_SHA256_V1",
            cargo_fe2o3_sha256
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>(),
        );
    remove_dynamic_loader_environment(command.as_command_mut());
    for name in [
        TARGET_ENV,
        BACKEND_ENV,
        HSACO_DIR_ENV,
        capability_broker::CAPABILITY_BROKER_ENV,
        BINDING_WRAPPER_MODE_ENV,
        BUILD_SESSION_ENV,
        worker_v2::CODEGEN_PIPELINE_ENV,
        worker_v2::WORKER_V2_CONFIG_ENV,
        worker_v2::WORKER_V2_EXPECTED_ID_ENV,
        AUTHORITY_CARGO_SHA256_ENV,
        AUTHORITY_RUSTC_SHA256_ENV,
        AUTHORITY_RUSTC_RUNTIME_SHA256_ENV,
        AUTHORITY_BACKEND_SHA256_ENV,
        "FE2O3_HOST_PASSTHROUGH",
    ] {
        command.as_command_mut().env_remove(name);
    }
    configure_pinned_rustc_child(command.as_command_mut(), pinned_rustc)?;
    command.as_command_mut().env(
        "LD_LIBRARY_PATH",
        format!("/proc/self/fd/{RUSTC_LIBRARY_CHILD_FD}"),
    );
    backend_target.inherit_for_child_at(command.as_command_mut(), BACKEND_BUILD_CHILD_FD)?;
    let status = command
        .status()
        .map_err(|error| format!("failed to build rustc-codegen-fe2o3: {error}"))?;

    if !status.success() {
        return Err("failed to build rustc-codegen-fe2o3".to_string());
    }

    if backend.is_file() {
        Ok(backend)
    } else {
        Err(format!(
            "backend build succeeded, but {} was not produced",
            backend.display()
        ))
    }
}

struct PinnedRustc {
    executable: pinned_executable::PinnedExecutable,
    lib_tree: RustcLibTree,
}

enum RustcLibTree {
    Ordinary(project::PinnedDirectory),
    Authority(rustc_lib_tree::PinnedRustcLibTree),
}

impl PinnedRustc {
    fn lib_tree_directory(&self) -> &project::PinnedDirectory {
        match &self.lib_tree {
            RustcLibTree::Ordinary(directory) => directory,
            RustcLibTree::Authority(lib_tree) => lib_tree.directory(),
        }
    }

    fn lib_tree_sha256(&self) -> &[u8; 32] {
        match &self.lib_tree {
            RustcLibTree::Ordinary(_) => &[0_u8; 32],
            RustcLibTree::Authority(lib_tree) => lib_tree.sha256(),
        }
    }

    fn assert_lib_tree_unmutated(&self) -> Result<(), String> {
        match &self.lib_tree {
            RustcLibTree::Ordinary(_) => Ok(()),
            RustcLibTree::Authority(lib_tree) => lib_tree.assert_unmutated(),
        }
    }

    fn revalidate_lib_tree(&self) -> Result<(), String> {
        match &self.lib_tree {
            RustcLibTree::Ordinary(_) => Ok(()),
            RustcLibTree::Authority(lib_tree) => lib_tree.revalidate(),
        }
    }
}

fn require_absolute_authority_tool_path(value: &OsStr, name: &str) -> Result<PathBuf, String> {
    let path = PathBuf::from(value);
    if !path.is_absolute() {
        return Err(format!(
            "cargo fe2o3 authority build requires {name} to name an absolute executable path"
        ));
    }
    Ok(path)
}

fn pin_authority_rustc(
    invocation_directory: &Path,
    expected_executable_sha256: [u8; 32],
    expected_lib_tree_sha256: [u8; 32],
) -> Result<PinnedRustc, String> {
    let declaration = env::var_os(AUTHORITY_RUSTC_PATH_ENV).ok_or_else(|| {
        format!("cargo fe2o3 authority build requires {AUTHORITY_RUSTC_PATH_ENV}")
    })?;
    let declared = require_absolute_authority_tool_path(&declaration, AUTHORITY_RUSTC_PATH_ENV)?;
    let canonical = std::fs::canonicalize(&declared).map_err(|error| {
        format!(
            "failed to inspect authority rustc executable {}: {error}",
            declared.display()
        )
    })?;
    if canonical.file_name() == Some(OsStr::new("rustup")) {
        return Err(
            "cargo fe2o3 authority rustc path resolves to a rustup proxy; rustup is never executed during authority selection"
                .to_owned(),
        );
    }
    if canonical.parent().is_none() || !invocation_directory.is_absolute() {
        return Err("authority rustc path is not canonicalizable".to_owned());
    }
    let source_executable = pinned_executable::PinnedExecutable::open(&canonical)
        .map_err(|error| format!("failed to pin authority rustc executable: {error}"))?;
    if source_executable.sha256() != &expected_executable_sha256 {
        return Err(format!(
            "cargo fe2o3 authority rustc does not match {AUTHORITY_RUSTC_SHA256_ENV}"
        ));
    }
    let lib_tree_directory = rustc_lib_tree_directory(&canonical)?;
    let lib_tree = rustc_lib_tree::PinnedRustcLibTree::pin(lib_tree_directory)?;
    if lib_tree.sha256() != &expected_lib_tree_sha256 {
        return Err(format!(
            "cargo fe2o3 authority rustc toolchain lib tree does not match {AUTHORITY_RUSTC_RUNTIME_SHA256_ENV}"
        ));
    }
    let executable = source_executable
        .seal_executable_image()
        .map_err(|error| format!("failed to seal authority rustc executable: {error}"))?;
    Ok(PinnedRustc {
        executable,
        lib_tree: RustcLibTree::Authority(lib_tree),
    })
}

fn pin_default_rustc(project: &project::CargoProject) -> Result<PinnedRustc, String> {
    let declared = OsStr::new("rustc");
    let resolved = binding_wrapper::resolve_command_executable(
        declared,
        &project.invocation_dir().child_path(),
    )
    .map_err(|error| format!("failed to resolve default rustc executable: {error}"))?;
    let canonical = std::fs::canonicalize(&resolved)
        .map_err(|error| format!("failed to inspect default rustc executable: {error}"))?;
    let rustc_path = if canonical.file_name() == Some(OsStr::new("rustup")) {
        resolve_rustup_toolchain_rustc(&canonical, project)?
    } else {
        canonical
    };
    let executable = pinned_executable::PinnedExecutable::open(&rustc_path)
        .map_err(|error| format!("failed to pin default rustc executable: {error}"))?;
    let lib_tree = rustc_lib_tree_directory(&rustc_path)?;
    Ok(PinnedRustc {
        executable,
        lib_tree: RustcLibTree::Ordinary(lib_tree),
    })
}

fn rustc_lib_tree_directory(rustc_path: &Path) -> Result<project::PinnedDirectory, String> {
    let executable_directory = rustc_path
        .parent()
        .ok_or_else(|| "default rustc executable has no parent directory".to_owned())?;
    let toolchain_library = executable_directory
        .parent()
        .map(|parent| parent.join("lib"))
        .filter(|path| path.is_dir())
        .unwrap_or_else(|| executable_directory.to_path_buf());
    project::PinnedDirectory::open_existing(
        toolchain_library,
        "pinned rustc toolchain lib-tree directory",
    )
}

fn resolve_rustup_toolchain_rustc(
    rustup_proxy: &Path,
    project: &project::CargoProject,
) -> Result<PathBuf, String> {
    let pinned = pinned_executable::PinnedExecutable::open(rustup_proxy)
        .map_err(|error| format!("failed to pin rustup proxy: {error}"))?;
    let mut command = pinned
        .command()
        .map_err(|error| format!("failed to prepare pinned rustup proxy: {error}"))?;
    command
        .as_command_mut()
        .arg0("rustup")
        .args(["which", "rustc"])
        .current_dir(project.invocation_dir().child_path());
    remove_dynamic_loader_environment(command.as_command_mut());
    let output = command
        .output()
        .map_err(|error| format!("failed to resolve rustup toolchain rustc: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "pinned rustup could not resolve the active rustc: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let mut path = output.stdout;
    while matches!(path.last(), Some(b'\n' | b'\r')) {
        path.pop();
    }
    if path.is_empty() || path.contains(&b'\n') || path.contains(&0) {
        return Err("pinned rustup returned a noncanonical rustc path".to_owned());
    }
    let path = PathBuf::from(os_string(path)?);
    if !path.is_absolute() {
        return Err("pinned rustup returned a relative rustc path".to_owned());
    }
    Ok(path)
}

fn configure_pinned_rustc_child(command: &mut Command, rustc: &PinnedRustc) -> Result<(), String> {
    let descriptor_path = PathBuf::from(format!("/proc/self/fd/{RUSTC_CHILD_FD}"));
    match std::fs::symlink_metadata(&descriptor_path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Ok(_) => {
            return Err(format!(
                "reserved rustc child descriptor {RUSTC_CHILD_FD} is already in use"
            ));
        }
        Err(error) => {
            return Err(format!(
                "failed to inspect reserved rustc child descriptor {RUSTC_CHILD_FD}: {error}"
            ));
        }
    }
    let mut source = rustc
        .executable
        .try_clone_for_transfer()
        .map_err(|error| format!("failed to retain pinned rustc executable: {error}"))?;
    source
        .seek(SeekFrom::Start(0))
        .map_err(|error| format!("failed to rewind pinned rustc executable: {error}"))?;
    let image = rustix::fs::memfd_create(
        "fe2o3-pinned-rustc",
        rustix::fs::MemfdFlags::CLOEXEC | rustix::fs::MemfdFlags::ALLOW_SEALING,
    )
    .map(File::from)
    .map_err(|error| format!("failed to allocate sealed rustc image: {error}"))?;
    let mut image = image;
    let copied = std::io::copy(&mut source, &mut image)
        .map_err(|error| format!("failed to snapshot pinned rustc executable: {error}"))?;
    if copied != rustc.executable.size() {
        return Err(format!(
            "pinned rustc snapshot copied {copied} bytes instead of {}",
            rustc.executable.size()
        ));
    }
    image
        .set_permissions(std::fs::Permissions::from_mode(0o500))
        .map_err(|error| format!("failed to make sealed rustc image executable: {error}"))?;
    rustix::fs::fcntl_add_seals(
        &image,
        rustix::fs::SealFlags::WRITE | rustix::fs::SealFlags::GROW | rustix::fs::SealFlags::SHRINK,
    )
    .and_then(|()| rustix::fs::fcntl_add_seals(&image, rustix::fs::SealFlags::SEAL))
    .map_err(|error| format!("failed to seal pinned rustc image: {error}"))?;
    image
        .seek(SeekFrom::Start(0))
        .map_err(|error| format!("failed to rewind sealed rustc image: {error}"))?;
    let verified = pinned_executable::PinnedExecutable::from_transferred_file(
        image
            .try_clone()
            .map_err(|error| format!("failed to verify sealed rustc image: {error}"))?,
        PathBuf::from("<sealed parent-pinned rustc image>"),
    )
    .map_err(|error| format!("failed to verify sealed rustc image: {error}"))?;
    if verified.sha256() != rustc.executable.sha256() {
        return Err("sealed rustc image differs from the parent-pinned compiler".to_owned());
    }
    let metadata = image
        .metadata()
        .map_err(|error| format!("failed to inspect sealed rustc image: {error}"))?;
    let expected = (metadata.dev(), metadata.ino(), metadata.mode());
    // SAFETY: the callback performs descriptor-only operations and owns `image` through exec.
    unsafe {
        command.pre_exec(move || {
            let installed = rustix::io::fcntl_dupfd_cloexec(&image, RUSTC_CHILD_FD)
                .map_err(std::io::Error::from)?;
            if installed.as_raw_fd() != RUSTC_CHILD_FD {
                return Err(std::io::Error::from_raw_os_error(
                    rustix::io::Errno::BUSY.raw_os_error(),
                ));
            }
            let descriptor = BorrowedFd::borrow_raw(RUSTC_CHILD_FD);
            let stat = rustix::fs::fstat(descriptor).map_err(std::io::Error::from)?;
            if (stat.st_dev, stat.st_ino, stat.st_mode) != expected {
                return Err(std::io::Error::from_raw_os_error(
                    rustix::io::Errno::STALE.raw_os_error(),
                ));
            }
            rustix::io::fcntl_setfd(descriptor, rustix::io::FdFlags::empty())
                .map_err(std::io::Error::from)?;
            let _ = installed.into_raw_fd();
            Ok(())
        });
    }
    rustc
        .lib_tree_directory()
        .inherit_for_child_at(command, RUSTC_LIBRARY_CHILD_FD)?;
    command.env("RUSTC", descriptor_path);
    Ok(())
}

fn configure_authority_cargo_child(
    command: &mut Command,
    rustc: &PinnedRustc,
) -> Result<(), String> {
    #[cfg(debug_assertions)]
    let fixture_environment = if env::var_os(NON_PRODUCTION_AUTHORITY_VALIDATION_ENV).as_deref()
        == Some(OsStr::new("1"))
    {
        env::vars_os()
            .filter(|(name, _)| os_bytes(name).starts_with(b"FE2O3_TEST_"))
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };

    command.env_clear();
    command.env("LANG", "C");
    #[cfg(debug_assertions)]
    command.envs(fixture_environment);
    configure_pinned_rustc_child(command, rustc)?;
    command.env(
        "LD_LIBRARY_PATH",
        format!("/proc/self/fd/{RUSTC_LIBRARY_CHILD_FD}"),
    );
    Ok(())
}

fn fe2o3_source_root() -> Result<PathBuf, String> {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let source_root = crate_root.parent().and_then(Path::parent).ok_or_else(|| {
        format!(
            "cargo-fe2o3 was built from an unsupported source layout: {}",
            crate_root.display()
        )
    })?;
    let manifest = source_root.join("Cargo.toml");
    if !manifest.is_file() {
        return Err(format!(
            "fe2o3 source manifest is unavailable at {}; set {BACKEND_ENV} to a built backend",
            manifest.display()
        ));
    }
    std::fs::canonicalize(source_root)
        .map_err(|error| format!("failed to resolve fe2o3 source root: {error}"))
}

fn dylib_path(target_dir: &Path) -> PathBuf {
    target_dir.join("debug/librustc_codegen_fe2o3.so")
}

fn find_workspace_root() -> Result<PathBuf, String> {
    let cargo = env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let output = Command::new(cargo)
        .args(["locate-project", "--workspace", "--message-format", "json"])
        .output()
        .map_err(|error| format!("failed to run cargo locate-project: {error}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "could not find Cargo project/workspace root: {}",
            stderr.trim()
        ));
    }

    let record: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|error| format!("failed to parse cargo locate-project output: {error}"))?;
    let manifest = record
        .get("root")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "cargo locate-project output did not contain a string `root`".to_string())?;
    let root = Path::new(manifest)
        .parent()
        .ok_or_else(|| format!("Cargo manifest has no parent directory: {manifest}"))?;

    std::fs::canonicalize(root)
        .map_err(|error| format!("failed to resolve Cargo project/workspace root: {error}"))
}

#[derive(Debug)]
struct RocmToolchain {
    rocm_path: PathBuf,
    clang: PathBuf,
    ld_lld: PathBuf,
    llc: Option<PathBuf>,
    llvm_readobj: Option<PathBuf>,
    hip_library: PathBuf,
}

fn detect_rocm_toolchain() -> Result<RocmToolchain, String> {
    let rocm_path =
        find_rocm_path().ok_or_else(|| "could not find ROCm; set ROCM_PATH".to_string())?;
    let llvm_bin = rocm_path.join("lib/llvm/bin");
    let clang = require_tool(&llvm_bin, "clang")?;
    let ld_lld = require_tool(&llvm_bin, "ld.lld")?;
    let hip_library = rocm_path.join("lib/libamdhip64.so");
    if !hip_library.is_file() {
        return Err(format!(
            "required ROCm path does not exist: {}",
            hip_library.display()
        ));
    }

    Ok(RocmToolchain {
        rocm_path,
        clang,
        ld_lld,
        llc: optional_tool(&llvm_bin, "llc"),
        llvm_readobj: optional_tool(&llvm_bin, "llvm-readobj"),
        hip_library,
    })
}

fn find_rocm_path() -> Option<PathBuf> {
    for var in ["ROCM_PATH", "HIP_PATH"] {
        if let Ok(value) = env::var(var) {
            let path = PathBuf::from(value);
            if path.join("lib/libamdhip64.so").is_file() {
                return Some(path);
            }
        }
    }

    ["/opt/rocm", "/opt/rocm-7.2.0", "/opt/rocm-7.1.0"]
        .into_iter()
        .map(PathBuf::from)
        .find(|path| path.join("lib/libamdhip64.so").is_file())
}

fn require_tool(llvm_bin: &Path, name: &str) -> Result<PathBuf, String> {
    let path = llvm_bin.join(name);
    if path.is_file() {
        Ok(path)
    } else {
        Err(format!(
            "required ROCm path does not exist: {}",
            path.display()
        ))
    }
}

fn optional_tool(llvm_bin: &Path, name: &str) -> Option<PathBuf> {
    let path = llvm_bin.join(name);
    path.is_file().then_some(path)
}

fn amd_gpu_target(simulation: bool) -> String {
    resolve_amd_gpu_target(
        simulation,
        env::var(TARGET_ENV)
            .ok()
            .filter(|value| !value.trim().is_empty()),
        detect_amd_gpu_target,
    )
}

fn resolve_amd_gpu_target(
    simulation: bool,
    declared: Option<String>,
    detect: impl FnOnce() -> Option<String>,
) -> String {
    declared
        .or_else(|| (!simulation).then(detect).flatten())
        .unwrap_or_else(|| DEFAULT_TARGET.to_string())
}

fn detect_amd_gpu_target() -> Option<String> {
    let output = Command::new("rocminfo").output().ok()?;
    if !output.status.success() {
        return None;
    }

    let mut text = String::from_utf8_lossy(&output.stdout).into_owned();
    if !output.stderr.is_empty() {
        text.push('\n');
        text.push_str(&String::from_utf8_lossy(&output.stderr));
    }

    parse_rocminfo_target(&text)
}

fn parse_rocminfo_target(text: &str) -> Option<String> {
    let mut generic = None;

    for raw in text.split_whitespace() {
        let token = raw.trim_matches(|c: char| {
            !(c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == ':')
        });
        let candidate = token.rsplit("--").next().unwrap_or(token);
        let candidate = candidate.trim_end_matches(':');

        if !is_gfx_target(candidate) {
            continue;
        }

        if candidate.contains("generic") {
            generic.get_or_insert_with(|| candidate.to_string());
        } else {
            return Some(candidate.to_string());
        }
    }

    generic
}

fn is_gfx_target(candidate: &str) -> bool {
    candidate.starts_with("gfx")
        && candidate.len() > 3
        && candidate.chars().any(|c| c.is_ascii_digit())
        && candidate
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

fn print_help() {
    eprintln!(
        "usage: cargo fe2o3 <command>\n\ncommands:\n  authority release   run an authority build through the protected self-launch boundary\n  doctor              check ROCm/HIP toolchain discovery\n  build               build with the fe2o3 rustc backend\n  run                 run with the fe2o3 rustc backend\n  simulate            compile source to exact KIR V6 and execute it deterministically on CPU\n  smoke               run manifest-selected GPU examples\n  examples            validate or query the example regression manifest\n  clean [--dry-run]   remove guarded fe2o3-owned target artifacts\n  inspect             inspect bounded artifact or HSACO metadata without execution\n  sanitize            plan or execute bounded ROCgdb precise-memory diagnostics\n  debug               plan or execute bounded batch/interactive ROCgdb sessions"
    );
}

#[cfg(test)]
mod tests {
    use super::{
        SIMULATION_MODE_ENV, aggregate_post_spawn_results, configure_simulation_build_environment,
        inject_application_runner_config, normalize_invocation, parse_rocminfo_target,
        resolve_amd_gpu_target, selected_run_target,
    };
    use crate::project::PinnedDirectory;
    use std::ffi::{OsStr, OsString};
    use std::process::Command;

    fn command_environment<'command>(
        command: &'command Command,
        name: &str,
    ) -> Option<&'command OsStr> {
        command
            .get_envs()
            .find(|(candidate, _)| *candidate == OsStr::new(name))
            .and_then(|(_, value)| value)
    }

    #[test]
    fn normalizes_direct_and_cargo_subcommand_invocations() {
        for command in [
            "authority",
            "doctor",
            "build",
            "run",
            "simulate",
            "smoke",
            "examples",
            "clean",
            "inspect",
            "sanitize",
            "debug",
        ] {
            let direct = vec![OsString::from(command), OsString::from("argument")];
            let cargo = vec![
                OsString::from("fe2o3"),
                OsString::from(command),
                OsString::from("argument"),
            ];

            assert_eq!(normalize_invocation(direct.clone()), direct);
            assert_eq!(normalize_invocation(cargo), direct);
        }
    }

    #[test]
    fn simulation_environment_is_explicit_and_normal_commands_clear_activation() {
        let mut command = Command::new("cargo");
        command
            .env(SIMULATION_MODE_ENV, "attacker")
            .env("FE2O3_HIP_SYS_DISABLE", "attacker");
        configure_simulation_build_environment(&mut command, false);
        assert_eq!(command_environment(&command, SIMULATION_MODE_ENV), None);
        assert_eq!(command_environment(&command, "FE2O3_HIP_SYS_DISABLE"), None);

        configure_simulation_build_environment(&mut command, true);
        assert_eq!(
            command_environment(&command, SIMULATION_MODE_ENV),
            Some(OsStr::new("1"))
        );
        assert_eq!(
            command_environment(&command, "FE2O3_CODEGEN_PIPELINE"),
            Some(OsStr::new("simulation-v1"))
        );
        assert_eq!(
            command_environment(&command, "FE2O3_HIP_SYS_DISABLE"),
            Some(OsStr::new("1"))
        );
    }

    #[test]
    fn simulation_target_selection_never_calls_hardware_detection() {
        assert_eq!(
            resolve_amd_gpu_target(true, None, || panic!("GPU detector was called")),
            super::DEFAULT_TARGET
        );
        assert_eq!(
            resolve_amd_gpu_target(true, Some("gfx942".to_owned()), || panic!(
                "GPU detector was called"
            ),),
            "gfx942"
        );
    }

    #[test]
    fn parses_agent_target_before_isa_generic() {
        let text = r#"
Agent 2
  Name:                    gfx1201
  ISA Info:
    Name:                    amdgcn-amd-amdhsa--gfx12-generic
"#;

        assert_eq!(parse_rocminfo_target(text).as_deref(), Some("gfx1201"));
    }

    #[test]
    fn parses_isa_target_when_agent_name_is_missing() {
        let text = "Name: amdgcn-amd-amdhsa--gfx942";

        assert_eq!(parse_rocminfo_target(text).as_deref(), Some("gfx942"));
    }

    #[test]
    fn falls_back_to_generic_target() {
        let text = "Name: amdgcn-amd-amdhsa--gfx12-generic";

        assert_eq!(
            parse_rocminfo_target(text).as_deref(),
            Some("gfx12-generic")
        );
    }

    #[test]
    fn runner_configuration_precedes_and_preserves_application_arguments() {
        let artifact_dir = PinnedDirectory::open_existing(
            std::env::current_dir().expect("current directory"),
            "runner configuration test directory",
        )
        .expect("pin test directory");
        let mut args = ["--target", "gfx942", "--", "application"]
            .map(OsString::from)
            .to_vec();
        inject_application_runner_config(
            &mut args,
            "gfx942",
            &artifact_dir,
            &[
                OsString::from("qemu"),
                OsString::from("-cpu"),
                OsString::from("max"),
            ],
            false,
        )
        .expect("inject runner");
        let separator = args
            .iter()
            .position(|argument| argument == "--")
            .expect("application separator");
        assert_eq!(&args[separator..], ["--", "application"]);
        assert_eq!(&args[separator - 2], "--config");
        assert!(
            args[separator - 1]
                .to_string_lossy()
                .starts_with("target.gfx942.runner=")
        );
        assert!(args[separator - 1].to_string_lossy().contains("71656d75"));
    }

    #[test]
    fn run_target_routing_is_strict_and_stops_at_separator() {
        let args = ["--target=gfx942", "--", "--target", "application"].map(OsString::from);
        assert_eq!(
            selected_run_target(&args).expect("parse target").as_deref(),
            Some("gfx942")
        );
        let duplicate = ["--target", "gfx942", "--target=gfx1100"].map(OsString::from);
        assert!(selected_run_target(&duplicate).is_err());
    }

    #[test]
    fn post_spawn_failures_preserve_primary_and_append_checks_in_order() {
        let error = aggregate_post_spawn_results(
            Err("cargo failed".to_owned()),
            [
                ("boundary", Err("boundary failed".to_owned())),
                ("runtime", Err("runtime changed".to_owned())),
                ("closure", Err("source changed".to_owned())),
            ],
        )
        .expect_err("post-spawn failures must fail");
        assert_eq!(
            error,
            "cargo failed; boundary also failed: boundary failed; runtime also failed: runtime changed; closure also failed: source changed"
        );
    }

    #[test]
    fn first_post_spawn_check_becomes_primary_after_cargo_success() {
        let error = aggregate_post_spawn_results(
            Ok(()),
            [
                ("boundary", Err("boundary failed".to_owned())),
                ("runtime", Err("runtime changed".to_owned())),
            ],
        )
        .expect_err("post-spawn validation failure must fail");
        assert_eq!(
            error,
            "boundary failed; runtime also failed: runtime changed"
        );
    }
}
