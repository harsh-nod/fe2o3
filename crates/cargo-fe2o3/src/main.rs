mod application_handoff;
mod application_sandbox;
mod binding_wrapper;
mod capability_broker;
mod clean;
mod example_manifest;
mod generation;
mod inspect;
mod non_production_reproduction;
#[allow(dead_code)]
#[path = "rustc_wrapper/pinned_codegen_backend.rs"]
mod pinned_codegen_backend;
#[path = "rustc_wrapper/pinned_executable.rs"]
mod pinned_executable;
#[cfg(test)]
mod pinned_executable_test_directory;
mod project;
mod tool_commands;
mod worker_v2;
mod worker_v2_artifact_container;
mod worker_v2_restart;

use std::env;
use std::ffi::{OsStr, OsString};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

const TARGET_ENV: &str = "FE2O3_TARGET";
const BACKEND_ENV: &str = "FE2O3_BACKEND";
const HSACO_DIR_ENV: &str = "FE2O3_HSACO_DIR";
const DEFAULT_TARGET: &str = "gfx1100";
const BINDING_WRAPPER_MODE_ENV: &str = "FE2O3_BINDING_WRAPPER_MODE_V1";
const MANAGED_RUSTC_ARGS_ENV: &str = "FE2O3_MANAGED_RUSTC_ARGS_V1";
const BUILD_SESSION_ENV: &str = "FE2O3_BUILD_SESSION_V1";
const INTERNAL_RUNNER_ARG: &str = "__fe2o3-runner-v1";
const BACKEND_BUILD_CHILD_FD: std::os::fd::RawFd = 196;
const ARTIFACT_CHILD_FD: std::os::fd::RawFd = 197;
const BACKEND_CHILD_FD: std::os::fd::RawFd = 198;

fn main() -> ExitCode {
    let raw_args = env::args_os().skip(1).collect::<Vec<_>>();
    if raw_args
        .first()
        .is_some_and(|argument| argument == INTERNAL_RUNNER_ARG)
    {
        return run_application_boundary(&raw_args[1..]);
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
        Some("doctor") => doctor(),
        Some("build") => cargo_with_backend("build", &rest),
        Some("run") => cargo_with_backend("run", &rest),
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
    let project = match project::CargoProject::discover(args) {
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
    let target = amd_gpu_target();
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
    match cargo_with_backend_result(command, args) {
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
        if let Err(error) = cargo_with_backend_result("run", &args) {
            eprintln!("{error}");
            return ExitCode::FAILURE;
        }
    }

    ExitCode::SUCCESS
}

fn cargo_with_backend_result(command: &str, args: &[OsString]) -> Result<(), String> {
    let project = project::CargoProject::discover(args)?;
    reject_preexisting_rustc_wrappers(&project, args)?;
    let mut context = BackendRunContext::prepare(project, args)?;
    run_cargo_with_backend(&mut context, command, args)
}

struct BackendRunContext {
    target: String,
    project: project::CargoProject,
    backend: PathBuf,
    pinned_backend: pinned_codegen_backend::PinnedCodegenBackend,
    _worker_v2: Option<worker_v2::PreparedWorkerV2Config>,
    worker_v2_identity: Option<worker_v2::WorkerV2ConfigIdentity>,
    target_dir: project::PinnedDirectory,
    generation: generation::PreparedGeneration,
    managed_rustc_args: OsString,
    binding_wrapper: PathBuf,
    build_session: fe2o3_artifact_transaction::BuildSession,
}

impl BackendRunContext {
    fn prepare(project: project::CargoProject, args: &[OsString]) -> Result<Self, String> {
        let target = amd_gpu_target();
        let target_dir = project.open_or_create_target()?;
        let backend = find_or_build_backend(&target_dir)?;
        let pinned_backend = pinned_codegen_backend::PinnedCodegenBackend::open(&backend)
            .map_err(|error| format!("failed to pin codegen backend: {error}"))?;
        let worker_v2 = worker_v2::PreparedWorkerV2Config::from_environment_for_cargo_setup()
            .map_err(|error| format!("Worker V2 setup failed: {error}"))?;
        let worker_v2_identity = worker_v2.as_ref().map(|config| config.identity());
        let cargo_configuration = project.semantic_configuration(args)?;
        let semantic = generation::semantic_identity(
            &target,
            pinned_backend.sha256(),
            worker_v2_identity,
            &cargo_configuration,
        )?;
        let generation = generation::PreparedGeneration::prepare(&target_dir, semantic)?;
        let backend_reference = pinned_backend
            .fixed_child_descriptor_path(BACKEND_CHILD_FD)
            .map_err(|error| format!("failed to retain pinned codegen backend: {error}"))?;
        let managed_rustc_args =
            generation::managed_rustc_args(&backend_reference, generation.token())?;
        let binding_wrapper = env::current_exe()
            .map_err(|error| format!("failed to locate cargo-fe2o3 executable: {error}"))?;
        let build_session = random_build_session()?;

        Ok(Self {
            target,
            project,
            backend,
            pinned_backend,
            _worker_v2: worker_v2,
            worker_v2_identity,
            target_dir,
            generation,
            managed_rustc_args,
            binding_wrapper,
            build_session,
        })
    }
}

fn run_cargo_with_backend(
    context: &mut BackendRunContext,
    command: &str,
    args: &[OsString],
) -> Result<(), String> {
    context.project.validate_paths()?;
    context.target_dir.validate_path("Cargo target directory")?;
    context.generation.reject_if_substituted()?;
    eprintln!(
        "cargo fe2o3 {command}: using backend {} for target {}",
        context.backend.display(),
        context.target
    );

    let cargo_declaration = env::var_os("CARGO").unwrap_or_else(|| OsString::from("cargo"));
    let cargo_path = binding_wrapper::resolve_command_executable(
        &cargo_declaration,
        &context.project.invocation_dir().child_path(),
    )
    .map_err(|error| format!("failed to resolve Cargo executable: {error}"))?;
    let pinned_cargo = pinned_executable::PinnedExecutable::open(&cargo_path)
        .map_err(|error| format!("failed to pin Cargo executable: {error}"))?;
    let mut cargo = pinned_cargo
        .command()
        .map_err(|error| format!("failed to prepare pinned Cargo executable: {error}"))?;
    let mut forwarded_args = args.to_vec();
    if command == "run" {
        let expects_envelope = context
            ._worker_v2
            .as_ref()
            .is_some_and(|config| config.envelope_mode().is_required());
        inject_application_runner(
            &context.project,
            context.generation.artifact_dir(),
            &mut forwarded_args,
            expects_envelope,
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
    let capability_binding = capability_broker::CapabilityBindingV2::new(
        capability_profile,
        context
            .worker_v2_identity
            .map(|identity| *identity.as_bytes()),
    )?;
    let capability_broker = capability_broker::CapabilityBroker::start(
        context.build_session,
        capability_binding,
        &context.pinned_backend,
        artifact_dir,
        &pinned_cargo,
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
        .env("RUSTC_WORKSPACE_WRAPPER", &context.binding_wrapper)
        .env(BINDING_WRAPPER_MODE_ENV, "1")
        .env(MANAGED_RUSTC_ARGS_ENV, &context.managed_rustc_args)
        .env(BUILD_SESSION_ENV, context.build_session.to_hex());
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
    let status = cargo.status();
    drop(capability_broker);

    match status {
        Ok(status) if status.success() => {
            context.project.validate_paths()?;
            context.target_dir.validate_path("Cargo target directory")?;
            context.generation.reject_if_substituted()?;
            context.generation.commit()
        }
        Ok(status) => Err(format!("cargo {command} failed with status {status}")),
        Err(error) => Err(format!("failed to run cargo: {error}")),
    }
}

fn inject_application_runner(
    project: &project::CargoProject,
    artifact_dir: &project::PinnedDirectory,
    args: &mut Vec<OsString>,
    expects_envelope: bool,
) -> Result<(), String> {
    let target = match selected_run_target(args)? {
        Some(target) => target,
        None => configured_run_target(project, args)?.unwrap_or(host_rustc_target()?),
    };
    if !target
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err(format!(
            "unsupported Cargo run target for runner isolation: {target:?}"
        ));
    }
    let original_runner = resolve_original_runner(project, args, &target)?;
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
    args: &[OsString],
) -> Result<Option<String>, String> {
    let Some(value) = project.cargo_config_value(args, "build.target")? else {
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
    if let Some(value) = project.cargo_config_value(args, &key)? {
        return parse_runner_value(value, &key);
    }

    if let Some(serde_json::Value::Object(targets)) = project.cargo_config_value(args, "target")?
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

fn run_application_boundary(args: &[OsString]) -> ExitCode {
    match run_application_boundary_result(args) {
        Ok(status) => ExitCode::from(binding_wrapper::exit_code(status)),
        Err(error) => {
            eprintln!("cargo-fe2o3 application runner: {error}");
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
    if args[0] != application_handoff::RUNNER_CONTEXT_VERSION {
        return Err(format!(
            "unsupported application runner context {:?}",
            args[0]
        ));
    }
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
        let application_identity = pinned_application
            .sealed_static_application_identity()
            .map_err(|error| format!("failed to bind application runtime identity: {error}"))?;
        let mut child = pinned_application
            .command()
            .map_err(|error| format!("failed to prepare pinned application: {error}"))?;
        child.args(&args[application_index + 1..]);
        scrub_application_environment(child.as_command_mut());
        let pending_ack = handoff.configure_child(child.as_command_mut(), application_identity)?;
        let mut process = child
            .as_command_mut()
            .spawn()
            .map_err(|error| format!("failed to launch pinned Cargo application: {error}"))?;
        if let Err(error) = pending_ack.await_after_spawn(&mut process) {
            return match application_handoff::terminate_application_group(&mut process) {
                Ok(()) => Err(error),
                Err(containment) => Err(format!(
                    "{error}; application containment failed: {containment}"
                )),
            };
        }
        if let Err(error) = handoff.validate_retained_currentness() {
            return match application_handoff::terminate_application_group(&mut process) {
                Ok(()) => Err(error),
                Err(containment) => Err(format!(
                    "{error}; application containment failed: {containment}"
                )),
            };
        }
        return application_handoff::wait_and_contain_application_group(&mut process);
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

fn reject_preexisting_rustc_wrappers(
    project: &project::CargoProject,
    args: &[OsString],
) -> Result<(), String> {
    for variable in ["RUSTC_WRAPPER", "RUSTC_WORKSPACE_WRAPPER"] {
        if let Some(value) = env::var_os(variable) {
            return Err(format!(
                "cargo fe2o3 cannot compose its binding-identity wrapper with preexisting {variable}={value:?}"
            ));
        }
    }
    for key in ["build.rustc-wrapper", "build.rustc-workspace-wrapper"] {
        if let Some(value) = project.cargo_config_value(args, key)? {
            return Err(format!(
                "cargo fe2o3 cannot compose its binding-identity wrapper with configured {key}={value}"
            ));
        }
    }
    Ok(())
}

fn find_or_build_backend(target_dir: &project::PinnedDirectory) -> Result<PathBuf, String> {
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
    let backend_target = target_dir.open_or_create_child(
        ".fe2o3-backend-build-v1",
        "isolated codegen-backend build directory",
    )?;
    let backend = dylib_path(backend_target.display_path());
    eprintln!("building rustc-codegen-fe2o3 backend...");
    let cargo = env::var_os("CARGO").unwrap_or_else(|| OsString::from("cargo"));
    let mut command = Command::new(cargo);
    command
        .args(["build", "--manifest-path"])
        .arg(source_root.join("Cargo.toml"))
        .args(["--target-dir"])
        .arg(backend_target.fixed_child_path(BACKEND_BUILD_CHILD_FD)?)
        .args(["-p", "rustc-codegen-fe2o3"])
        .current_dir(&source_root)
        .env_remove("RUSTFLAGS")
        .env_remove("CARGO_ENCODED_RUSTFLAGS")
        .env_remove("CARGO_TARGET_DIR")
        .env_remove("RUSTC_WRAPPER")
        .env_remove("RUSTC_WORKSPACE_WRAPPER");
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
        "FE2O3_HOST_PASSTHROUGH",
    ] {
        command.env_remove(name);
    }
    backend_target.inherit_for_child_at(&mut command, BACKEND_BUILD_CHILD_FD)?;
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

fn amd_gpu_target() -> String {
    env::var(TARGET_ENV)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .or_else(detect_amd_gpu_target)
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
        "usage: cargo fe2o3 <command>\n\ncommands:\n  doctor              check ROCm/HIP toolchain discovery\n  build               build with the fe2o3 rustc backend\n  run                 run with the fe2o3 rustc backend\n  smoke               run manifest-selected GPU examples\n  examples            validate or query the example regression manifest\n  clean [--dry-run]   remove guarded fe2o3-owned target artifacts\n  inspect             inspect bounded artifact or HSACO metadata without execution\n  sanitize            plan or execute bounded ROCgdb precise-memory diagnostics\n  debug               plan or execute bounded batch/interactive ROCgdb sessions"
    );
}

#[cfg(test)]
mod tests {
    use super::{
        inject_application_runner_config, normalize_invocation, parse_rocminfo_target,
        selected_run_target,
    };
    use crate::project::PinnedDirectory;
    use std::ffi::OsString;

    #[test]
    fn normalizes_direct_and_cargo_subcommand_invocations() {
        for command in [
            "doctor", "build", "run", "smoke", "examples", "clean", "inspect", "sanitize", "debug",
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
}
