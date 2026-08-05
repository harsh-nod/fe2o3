mod clean;

use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use syn::visit::{self, Visit};
use syn::{Expr, ExprMethodCall, Lit};

const TARGET_ENV: &str = "FE2O3_TARGET";
const BACKEND_ENV: &str = "FE2O3_BACKEND";
const HSACO_DIR_ENV: &str = "FE2O3_HSACO_DIR";
const DEFAULT_TARGET: &str = "gfx1100";
const EXAMPLE_MANIFEST_PATH: &str = "examples/regression-manifest-v1.txt";
const EXAMPLE_MANIFEST_VERSION: &str = "fe2o3-example-regressions-v1";
const EXAMPLE_MANIFEST_COLUMNS: &str = "package|rustc_check|rocm_compile|gpu_smoke|artifacts";

fn main() -> ExitCode {
    let mut invocation = normalize_invocation(env::args().skip(1).collect());
    let mut args = invocation.drain(..);
    let command = args.next().unwrap_or_else(|| "help".to_string());
    let rest: Vec<String> = args.collect();

    match command.as_str() {
        "doctor" => doctor(),
        "build" => cargo_with_backend("build", &rest),
        "run" => cargo_with_backend("run", &rest),
        "smoke" => smoke(&rest),
        "examples" => examples_command(&rest),
        "clean" => clean_command(&rest),
        "help" | "--help" | "-h" => {
            print_help();
            ExitCode::SUCCESS
        }
        other => {
            eprintln!("unknown cargo-fe2o3 command `{other}`");
            print_help();
            ExitCode::FAILURE
        }
    }
}

fn normalize_invocation(mut args: Vec<String>) -> Vec<String> {
    if args.first().is_some_and(|arg| arg == "fe2o3") {
        args.remove(0);
    }
    args
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ExampleLane {
    All,
    RustcCheck,
    RocmCompile,
    GpuSmoke,
}

impl ExampleLane {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "all" => Ok(Self::All),
            "rustc-check" => Ok(Self::RustcCheck),
            "rocm-compile" => Ok(Self::RocmCompile),
            "gpu-smoke" => Ok(Self::GpuSmoke),
            _ => Err(format!(
                "unknown example lane `{value}`; expected all, rustc-check, rocm-compile, or gpu-smoke"
            )),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ExampleEntry {
    package: String,
    rustc_check: bool,
    rocm_compile: bool,
    gpu_smoke: bool,
    artifacts: Vec<String>,
}

impl ExampleEntry {
    fn participates(&self, lane: ExampleLane) -> bool {
        match lane {
            ExampleLane::All => true,
            ExampleLane::RustcCheck => self.rustc_check,
            ExampleLane::RocmCompile => self.rocm_compile,
            ExampleLane::GpuSmoke => self.gpu_smoke,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ExampleManifest {
    entries: Vec<ExampleEntry>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct WorkspaceExample {
    package: String,
    artifacts: Vec<String>,
}

fn examples_command(args: &[String]) -> ExitCode {
    match examples_command_result(args) {
        Ok(lines) => {
            for line in lines {
                println!("{line}");
            }
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

fn examples_command_result(args: &[String]) -> Result<Vec<String>, String> {
    let workspace_root = find_workspace_root()?;
    let manifest = load_example_manifest(&workspace_root)?;

    match args {
        [command] if command == "check" => {
            let artifact_count = manifest
                .entries
                .iter()
                .map(|entry| entry.artifacts.len())
                .sum::<usize>();
            Ok(vec![format!(
                "example manifest: {} packages, {artifact_count} artifacts",
                manifest.entries.len()
            )])
        }
        [command, lane] if command == "list" => {
            let lane = ExampleLane::parse(lane)?;
            Ok(manifest
                .entries
                .iter()
                .filter(|entry| entry.participates(lane))
                .map(|entry| entry.package.clone())
                .collect())
        }
        [command, package] if command == "check-artifacts" => {
            let entry = manifest
                .entries
                .iter()
                .find(|entry| entry.package == *package)
                .ok_or_else(|| format!("package `{package}` is not in {EXAMPLE_MANIFEST_PATH}"))?;
            if !entry.rocm_compile {
                return Err(format!(
                    "package `{package}` does not participate in ROCm compilation"
                ));
            }

            let artifact_dir = workspace_root.join("target/fe2o3");
            for artifact in &entry.artifacts {
                let path = artifact_dir.join(artifact);
                if !path.is_file() {
                    return Err(format!(
                        "expected artifact for package `{package}` was not produced: {}",
                        path.display()
                    ));
                }
            }
            Ok(vec![format!(
                "example artifacts: {package}: {}",
                entry.artifacts.join(",")
            )])
        }
        _ => Err(
            "usage: cargo fe2o3 examples <check|list <all|rustc-check|rocm-compile|gpu-smoke>|check-artifacts <package>>"
                .to_string(),
        ),
    }
}

fn load_example_manifest(workspace_root: &Path) -> Result<ExampleManifest, String> {
    let path = workspace_root.join(EXAMPLE_MANIFEST_PATH);
    let contents = fs::read_to_string(&path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    let manifest = parse_example_manifest(&contents)
        .map_err(|error| format!("invalid {}: {error}", path.display()))?;
    let workspace_examples = workspace_example_projection(workspace_root)?;
    validate_example_projection(&manifest, &workspace_examples)
        .map_err(|error| format!("invalid {}: {error}", path.display()))?;
    Ok(manifest)
}

fn parse_example_manifest(contents: &str) -> Result<ExampleManifest, String> {
    if !contents.ends_with('\n') {
        return Err("manifest must end with a newline".to_string());
    }
    if contents.contains('\r') {
        return Err("carriage returns are not permitted".to_string());
    }

    let mut lines = contents.lines();
    if lines.next() != Some(EXAMPLE_MANIFEST_VERSION) {
        return Err(format!("first line must be `{EXAMPLE_MANIFEST_VERSION}`"));
    }
    if lines.next() != Some(EXAMPLE_MANIFEST_COLUMNS) {
        return Err(format!("second line must be `{EXAMPLE_MANIFEST_COLUMNS}`"));
    }

    let mut entries = Vec::new();
    let mut packages = BTreeSet::new();
    let mut artifacts = BTreeSet::new();
    let mut previous_package: Option<String> = None;

    for (index, line) in lines.enumerate() {
        let line_number = index + 3;
        if line.is_empty() {
            return Err(format!("line {line_number}: blank lines are not permitted"));
        }
        let fields = line.split('|').collect::<Vec<_>>();
        let [
            package,
            rustc_check,
            rocm_compile,
            gpu_smoke,
            artifact_field,
        ] = fields.as_slice()
        else {
            return Err(format!(
                "line {line_number}: expected exactly five pipe-delimited fields"
            ));
        };

        validate_package_name(package).map_err(|error| format!("line {line_number}: {error}"))?;
        if !packages.insert((*package).to_string()) {
            return Err(format!("line {line_number}: duplicate package `{package}`"));
        }
        if previous_package
            .as_deref()
            .is_some_and(|previous| previous >= *package)
        {
            return Err(format!(
                "line {line_number}: packages must be sorted lexicographically"
            ));
        }
        previous_package = Some((*package).to_string());

        let rustc_check = parse_manifest_bool(rustc_check, line_number, "rustc_check")?;
        let rocm_compile = parse_manifest_bool(rocm_compile, line_number, "rocm_compile")?;
        let gpu_smoke = parse_manifest_bool(gpu_smoke, line_number, "gpu_smoke")?;
        let entry_artifacts = if *artifact_field == "-" {
            Vec::new()
        } else {
            let mut parsed = Vec::new();
            let mut previous_artifact: Option<&str> = None;
            for artifact in artifact_field.split(',') {
                validate_artifact_name(artifact)
                    .map_err(|error| format!("line {line_number}: {error}"))?;
                if !artifacts.insert(artifact.to_string()) {
                    return Err(format!(
                        "line {line_number}: duplicate artifact `{artifact}`"
                    ));
                }
                if previous_artifact.is_some_and(|previous| previous >= artifact) {
                    return Err(format!(
                        "line {line_number}: artifacts must be sorted lexicographically"
                    ));
                }
                previous_artifact = Some(artifact);
                parsed.push(artifact.to_string());
            }
            parsed
        };

        if gpu_smoke && !rocm_compile {
            return Err(format!(
                "line {line_number}: gpu_smoke requires rocm_compile"
            ));
        }
        if rocm_compile != !entry_artifacts.is_empty() {
            return Err(format!(
                "line {line_number}: rocm_compile must have one or more artifacts, and CPU-only entries must use `-`"
            ));
        }

        entries.push(ExampleEntry {
            package: package.to_string(),
            rustc_check,
            rocm_compile,
            gpu_smoke,
            artifacts: entry_artifacts,
        });
    }

    if entries.is_empty() {
        return Err("manifest must contain at least one package".to_string());
    }

    Ok(ExampleManifest { entries })
}

fn parse_manifest_bool(value: &str, line: usize, field: &str) -> Result<bool, String> {
    match value {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Err(format!(
            "line {line}: {field} must be exactly `true` or `false`"
        )),
    }
}

fn validate_package_name(name: &str) -> Result<(), String> {
    if is_safe_name_stem(name) {
        Ok(())
    } else {
        Err(format!("unsafe package name `{name}`"))
    }
}

fn validate_artifact_name(name: &str) -> Result<(), String> {
    let Some(stem) = name.strip_suffix(".hsaco") else {
        return Err(format!(
            "unsafe artifact name `{name}`; expected a .hsaco basename"
        ));
    };
    if name.len() <= 134 && is_safe_name_stem(stem) {
        Ok(())
    } else {
        Err(format!("unsafe artifact name `{name}`"))
    }
}

fn is_safe_name_stem(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 128
        && name.as_bytes()[0].is_ascii_lowercase()
        && name.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-' || byte == b'_'
        })
}

fn workspace_example_projection(workspace_root: &Path) -> Result<Vec<WorkspaceExample>, String> {
    let cargo = env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let output = Command::new(cargo)
        .args(["metadata", "--locked", "--no-deps", "--format-version", "1"])
        .current_dir(workspace_root)
        .output()
        .map_err(|error| format!("failed to run cargo metadata: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "cargo metadata failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    let metadata: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|error| format!("failed to parse cargo metadata output: {error}"))?;
    let packages = metadata
        .get("packages")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| "cargo metadata output did not contain `packages`".to_string())?;
    let examples_root = workspace_root.join("examples");
    let mut projection = Vec::new();

    for package in packages {
        let name = package
            .get("name")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| "cargo metadata package did not contain `name`".to_string())?;
        let manifest_path = package
            .get("manifest_path")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| {
                format!("cargo metadata package `{name}` did not contain `manifest_path`")
            })?;
        let manifest_path = Path::new(manifest_path);
        let Ok(relative) = manifest_path.strip_prefix(&examples_root) else {
            continue;
        };
        let components = relative.components().collect::<Vec<_>>();
        if components.len() != 2
            || relative.file_name().and_then(|name| name.to_str()) != Some("Cargo.toml")
        {
            continue;
        }

        let package_root = manifest_path
            .parent()
            .ok_or_else(|| format!("example package `{name}` manifest has no parent directory"))?;
        projection.push(WorkspaceExample {
            package: name.to_string(),
            artifacts: source_artifacts(package_root)?,
        });
    }

    projection.sort_by(|left, right| left.package.cmp(&right.package));
    Ok(projection)
}

fn source_artifacts(package_root: &Path) -> Result<Vec<String>, String> {
    let source_root = package_root.join("src");
    let mut source_files = Vec::new();
    collect_rust_sources(&source_root, &mut source_files)?;
    source_files.sort();

    let mut seen = BTreeSet::new();
    for source_path in source_files {
        let source = fs::read_to_string(&source_path)
            .map_err(|error| format!("failed to read {}: {error}", source_path.display()))?;
        for artifact in artifact_join_literals(&source)
            .map_err(|error| format!("failed to inspect {}: {error}", source_path.display()))?
        {
            seen.insert(artifact);
        }
    }
    Ok(seen.into_iter().collect())
}

fn collect_rust_sources(directory: &Path, files: &mut Vec<PathBuf>) -> Result<(), String> {
    let mut entries = fs::read_dir(directory)
        .map_err(|error| format!("failed to read {}: {error}", directory.display()))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("failed to read {}: {error}", directory.display()))?;
    entries.sort_by_key(|entry| entry.file_name());

    for entry in entries {
        let file_type = entry
            .file_type()
            .map_err(|error| format!("failed to inspect {}: {error}", entry.path().display()))?;
        if file_type.is_symlink() {
            return Err(format!(
                "example source tree contains unsupported symlink: {}",
                entry.path().display()
            ));
        }
        if file_type.is_dir() {
            collect_rust_sources(&entry.path(), files)?;
        } else if file_type.is_file()
            && entry.path().extension().and_then(|ext| ext.to_str()) == Some("rs")
        {
            files.push(entry.path());
        }
    }
    Ok(())
}

#[derive(Default)]
struct ArtifactJoinVisitor {
    artifacts: Vec<String>,
}

impl<'ast> Visit<'ast> for ArtifactJoinVisitor {
    fn visit_expr_method_call(&mut self, node: &'ast ExprMethodCall) {
        if node.method == "join"
            && node.args.len() == 1
            && let Some(Expr::Lit(argument)) = node.args.first()
            && let Lit::Str(literal) = &argument.lit
            && literal.value().to_ascii_lowercase().ends_with(".hsaco")
        {
            self.artifacts.push(literal.value());
        }
        visit::visit_expr_method_call(self, node);
    }
}

fn artifact_join_literals(source: &str) -> Result<Vec<String>, String> {
    let file = syn::parse_file(source).map_err(|error| format!("invalid Rust source: {error}"))?;
    let mut visitor = ArtifactJoinVisitor::default();
    visitor.visit_file(&file);

    let mut artifacts = BTreeSet::new();
    for artifact in visitor.artifacts {
        validate_artifact_name(&artifact)
            .map_err(|_| format!("non-canonical HSACO join argument `{artifact}`"))?;
        artifacts.insert(artifact);
    }
    Ok(artifacts.into_iter().collect())
}

fn validate_example_projection(
    manifest: &ExampleManifest,
    workspace_examples: &[WorkspaceExample],
) -> Result<(), String> {
    let declared = manifest
        .entries
        .iter()
        .map(|entry| (entry.package.as_str(), entry))
        .collect::<BTreeMap<_, _>>();
    let current = workspace_examples
        .iter()
        .map(|entry| (entry.package.as_str(), entry))
        .collect::<BTreeMap<_, _>>();

    for package in declared.keys() {
        if !current.contains_key(package) {
            return Err(format!(
                "declared package `{package}` is not a direct examples workspace package"
            ));
        }
    }
    for package in current.keys() {
        if !declared.contains_key(package) {
            return Err(format!(
                "workspace example package `{package}` is missing from the manifest"
            ));
        }
    }
    for (package, declared_entry) in declared {
        let current_entry = current[package];
        if declared_entry.artifacts != current_entry.artifacts {
            return Err(format!(
                "package `{package}` artifact drift: declared [{}], current [{}]",
                declared_entry.artifacts.join(","),
                current_entry.artifacts.join(",")
            ));
        }
    }

    Ok(())
}

fn clean_command(args: &[String]) -> ExitCode {
    let options = match clean::parse_options(args) {
        Ok(options) => options,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::FAILURE;
        }
    };
    let workspace_root = match find_workspace_root() {
        Ok(path) => path,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::FAILURE;
        }
    };
    let plan = match clean::plan(&workspace_root) {
        Ok(plan) => plan,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::FAILURE;
        }
    };

    match clean::execute(&plan, options) {
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

fn cargo_with_backend(command: &str, args: &[String]) -> ExitCode {
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
    let manifest = match load_example_manifest(&workspace_root) {
        Ok(manifest) => manifest,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::FAILURE;
        }
    };
    let context = match BackendRunContext::prepare(workspace_root) {
        Ok(context) => context,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::FAILURE;
        }
    };

    for entry in manifest.entries.iter().filter(|entry| entry.gpu_smoke) {
        let package = &entry.package;
        eprintln!("cargo fe2o3 smoke: running {package}");
        let args = ["-p".to_string(), package.clone()];
        if let Err(error) = clean_explicit_packages(&context.workspace_root, &args) {
            eprintln!("{error}");
            return ExitCode::FAILURE;
        }
        if let Err(error) = run_cargo_with_backend(&context, "run", &args) {
            eprintln!("{error}");
            return ExitCode::FAILURE;
        }
    }

    ExitCode::SUCCESS
}

fn cargo_with_backend_result(command: &str, args: &[String]) -> Result<(), String> {
    let workspace_root = find_workspace_root()?;

    clean_explicit_packages(&workspace_root, args)?;
    let context = BackendRunContext::prepare(workspace_root)?;
    run_cargo_with_backend(&context, command, args)
}

#[derive(Debug)]
struct BackendRunContext {
    target: String,
    workspace_root: PathBuf,
    backend: PathBuf,
    artifact_dir: PathBuf,
    rustflags: String,
}

impl BackendRunContext {
    fn prepare(workspace_root: PathBuf) -> Result<Self, String> {
        let target = amd_gpu_target();
        let backend = find_or_build_backend(&workspace_root)?;
        let artifact_dir = workspace_root.join("target/fe2o3");
        if let Err(error) = std::fs::create_dir_all(&artifact_dir) {
            return Err(format!(
                "failed to create fe2o3 artifact directory {}: {error}",
                artifact_dir.display()
            ));
        }

        let rustflags = append_rustflags(&[
            format!("-Zcodegen-backend={}", backend.display()),
            "-Zmir-enable-passes=-JumpThreading".to_string(),
        ]);

        Ok(Self {
            target,
            workspace_root,
            backend,
            artifact_dir,
            rustflags,
        })
    }
}

fn run_cargo_with_backend(
    context: &BackendRunContext,
    command: &str,
    args: &[String],
) -> Result<(), String> {
    eprintln!(
        "cargo fe2o3 {command}: using backend {} for target {}",
        context.backend.display(),
        context.target
    );

    let status = Command::new("cargo")
        .arg(command)
        .args(args)
        .env("RUSTFLAGS", &context.rustflags)
        .env(HSACO_DIR_ENV, &context.artifact_dir)
        .env(TARGET_ENV, &context.target)
        .env("FE2O3_HOST_PASSTHROUGH", "0")
        .status();

    match status {
        Ok(status) if status.success() => Ok(()),
        Ok(status) => Err(format!("cargo {command} failed with status {status}")),
        Err(error) => Err(format!("failed to run cargo: {error}")),
    }
}

fn clean_explicit_packages(workspace_root: &Path, args: &[String]) -> Result<(), String> {
    let packages = explicit_packages(args);
    if packages.is_empty() {
        return Ok(());
    }

    eprintln!(
        "cargo fe2o3: cleaning package artifact(s) for {}",
        packages.join(", ")
    );

    let mut command = Command::new("cargo");
    command.arg("clean");
    for package in packages {
        command.args(["-p", &package]);
    }

    let status = command
        .current_dir(workspace_root)
        .env_remove("RUSTFLAGS")
        .env_remove("CARGO_ENCODED_RUSTFLAGS")
        .status()
        .map_err(|error| format!("failed to clean package artifacts: {error}"))?;

    status
        .success()
        .then_some(())
        .ok_or_else(|| "failed to clean package artifacts".to_string())
}

fn explicit_packages(args: &[String]) -> Vec<String> {
    let mut packages = Vec::new();
    let mut args = args.iter();

    while let Some(arg) = args.next() {
        if arg == "--" {
            break;
        }

        let package = if arg == "-p" || arg == "--package" {
            args.next().map(String::as_str)
        } else if let Some(package) = arg.strip_prefix("--package=") {
            Some(package)
        } else if arg.starts_with("-p") && !arg.starts_with("--") && arg.len() > 2 {
            Some(&arg[2..])
        } else {
            None
        };

        if let Some(package) = package
            && !package.is_empty()
            && !packages.iter().any(|existing| existing == package)
        {
            packages.push(package.to_string());
        }
    }

    packages
}

fn find_or_build_backend(workspace_root: &Path) -> Result<PathBuf, String> {
    if let Ok(path) = env::var(BACKEND_ENV) {
        let path = PathBuf::from(path);
        if path.is_file() {
            return Ok(path);
        }
        return Err(format!(
            "{BACKEND_ENV} points to {}, but that file does not exist",
            path.display()
        ));
    }

    let backend = dylib_path(workspace_root);
    eprintln!("building rustc-codegen-fe2o3 backend...");
    let status = Command::new("cargo")
        .args(["build", "-p", "rustc-codegen-fe2o3"])
        .current_dir(workspace_root)
        .env_remove("RUSTFLAGS")
        .env_remove("CARGO_ENCODED_RUSTFLAGS")
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

fn dylib_path(workspace_root: &Path) -> PathBuf {
    let target_dir = env::var("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| workspace_root.join("target"));
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

fn append_rustflags(extra: &[String]) -> String {
    let mut flags = env::var("RUSTFLAGS").unwrap_or_default();
    for flag in extra {
        if !flags.is_empty() {
            flags.push(' ');
        }
        flags.push_str(flag);
    }
    flags
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
        "usage: cargo fe2o3 <command>\n\ncommands:\n  doctor              check ROCm/HIP toolchain discovery\n  build               build with the fe2o3 rustc backend\n  run                 run with the fe2o3 rustc backend\n  smoke               run manifest-selected GPU examples\n  examples            validate or query the example regression manifest\n  clean [--dry-run]   preview or remove target/fe2o3 artifacts (removal requires Unix)"
    );
}

#[cfg(test)]
mod tests {
    use super::{
        EXAMPLE_MANIFEST_COLUMNS, EXAMPLE_MANIFEST_VERSION, ExampleLane, WorkspaceExample,
        artifact_join_literals, explicit_packages, load_example_manifest, normalize_invocation,
        parse_example_manifest, parse_rocminfo_target, validate_example_projection,
    };
    use std::path::Path;

    fn example_manifest(rows: &str) -> String {
        format!("{EXAMPLE_MANIFEST_VERSION}\n{EXAMPLE_MANIFEST_COLUMNS}\n{rows}")
    }

    #[test]
    fn parses_strict_example_manifest_and_lane_projection() {
        let manifest = parse_example_manifest(&example_manifest(
            "fe2o3-alpha|true|true|false|alpha.hsaco\n\
             fe2o3-pipeline|true|true|true|bias_stage.hsaco,scale_stage.hsaco\n\
             verus-vecadd|true|false|false|-\n",
        ))
        .expect("valid manifest");

        assert_eq!(manifest.entries.len(), 3);
        assert_eq!(
            manifest.entries[1].artifacts,
            ["bias_stage.hsaco", "scale_stage.hsaco"]
        );
        assert!(manifest.entries[0].participates(ExampleLane::RustcCheck));
        assert!(manifest.entries[0].participates(ExampleLane::All));
        assert!(manifest.entries[0].participates(ExampleLane::RocmCompile));
        assert!(!manifest.entries[0].participates(ExampleLane::GpuSmoke));
        assert!(manifest.entries[2].artifacts.is_empty());
    }

    #[test]
    fn rejects_malformed_manifest_structure_and_fields() {
        let cases = [
            (
                "wrong-version\npackage|rustc_check|rocm_compile|gpu_smoke|artifacts\na|true|false|false|-\n".to_string(),
                "first line",
            ),
            (
                format!("{EXAMPLE_MANIFEST_VERSION}\npackage|unknown\na|true|false|false|-\n"),
                "second line",
            ),
            (example_manifest("a|true|false|false|-"), "end with a newline"),
            (example_manifest("\n"), "blank lines"),
            (
                example_manifest("a|true|false|false|-|extra\n"),
                "exactly five",
            ),
            (
                example_manifest("a|yes|false|false|-\n"),
                "rustc_check must be exactly",
            ),
            (
                example_manifest("b|true|false|false|-\na|true|false|false|-\n"),
                "sorted lexicographically",
            ),
            (
                example_manifest("a|true|false|false|-\na|true|false|false|-\n"),
                "duplicate package",
            ),
            (example_manifest(""), "at least one package"),
        ];

        for (contents, expected) in cases {
            let error = parse_example_manifest(&contents).expect_err("manifest must fail");
            assert!(
                error.contains(expected),
                "expected `{expected}` in `{error}`"
            );
        }
    }

    #[test]
    fn rejects_unsafe_names_duplicates_and_inconsistent_lanes() {
        let cases = [
            ("-package|true|false|false|-\n", "unsafe package name"),
            ("a|true|true|true|../escape.hsaco\n", "unsafe artifact name"),
            ("a|true|true|true|alpha.o\n", "expected a .hsaco basename"),
            (
                "a|true|true|true|same.hsaco\nb|true|true|true|same.hsaco\n",
                "duplicate artifact",
            ),
            (
                "a|true|true|true|zeta.hsaco,alpha.hsaco\n",
                "artifacts must be sorted lexicographically",
            ),
            ("a|true|false|true|-\n", "gpu_smoke requires rocm_compile"),
            (
                "a|true|true|false|-\n",
                "rocm_compile must have one or more artifacts",
            ),
            (
                "a|true|false|false|alpha.hsaco\n",
                "CPU-only entries must use `-`",
            ),
        ];

        for (rows, expected) in cases {
            let error = parse_example_manifest(&example_manifest(rows))
                .expect_err("manifest must fail closed");
            assert!(
                error.contains(expected),
                "expected `{expected}` in `{error}`"
            );
        }
    }

    #[test]
    fn structurally_extracts_and_canonicalizes_hsaco_join_arguments() {
        let source = r##"
fn inspect(root: &std::path::Path, dynamic: &str) {
    // root.join("commented.hsaco");
    let _quote = '\"';
    let _bytes = b"bytes.hsaco";
    let _raw_bytes = br#"raw_bytes.hsaco"#;
    let _detached = "detached.hsaco";
    ignored!(root.join("macro_tokens.hsaco"));
    let _beta = root.join(r#"beta.hsaco"#);
    let _dynamic = root.join(dynamic);
    let _not_join = root.with_file_name("not_joined.hsaco");
    let _alpha = root.join("alpha.hsaco");
}
"##;
        let artifacts = artifact_join_literals(source).expect("inspect Rust syntax");

        assert_eq!(artifacts, ["alpha.hsaco", "beta.hsaco"]);
    }

    #[test]
    fn structured_artifact_projection_rejects_bad_syntax_and_unsafe_joins() {
        let malformed = artifact_join_literals("fn main( {").expect_err("invalid Rust");
        let unsafe_join = artifact_join_literals(
            r#"fn main() { std::path::Path::new(".").join("../escape.hsaco"); }"#,
        )
        .expect_err("unsafe join");

        assert!(malformed.contains("invalid Rust source"));
        assert!(unsafe_join.contains("non-canonical HSACO join argument"));
    }

    #[test]
    fn validates_manifest_against_workspace_projection() {
        let manifest = parse_example_manifest(&example_manifest(
            "fe2o3-alpha|true|true|true|alpha.hsaco\n\
             verus-vecadd|true|false|false|-\n",
        ))
        .expect("valid manifest");
        let workspace = [
            WorkspaceExample {
                package: "fe2o3-alpha".to_string(),
                artifacts: vec!["alpha.hsaco".to_string()],
            },
            WorkspaceExample {
                package: "verus-vecadd".to_string(),
                artifacts: Vec::new(),
            },
        ];

        validate_example_projection(&manifest, &workspace).expect("matching projection");
    }

    #[test]
    fn rejects_missing_extra_and_drifted_workspace_projection() {
        let manifest = parse_example_manifest(&example_manifest(
            "fe2o3-alpha|true|true|true|alpha.hsaco\n",
        ))
        .expect("valid manifest");
        let cases = [
            (
                vec![],
                "declared package `fe2o3-alpha` is not a direct examples workspace package",
            ),
            (
                vec![
                    WorkspaceExample {
                        package: "fe2o3-alpha".to_string(),
                        artifacts: vec!["alpha.hsaco".to_string()],
                    },
                    WorkspaceExample {
                        package: "fe2o3-beta".to_string(),
                        artifacts: vec!["beta.hsaco".to_string()],
                    },
                ],
                "workspace example package `fe2o3-beta` is missing from the manifest",
            ),
            (
                vec![WorkspaceExample {
                    package: "fe2o3-alpha".to_string(),
                    artifacts: vec!["renamed.hsaco".to_string()],
                }],
                "artifact drift",
            ),
        ];

        for (workspace, expected) in cases {
            let error = validate_example_projection(&manifest, &workspace)
                .expect_err("projection must fail closed");
            assert!(
                error.contains(expected),
                "expected `{expected}` in `{error}`"
            );
        }
    }

    #[test]
    fn checked_in_manifest_matches_current_repository() {
        let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .expect("canonical workspace root");
        let manifest = load_example_manifest(&workspace_root).expect("checked manifest validates");
        let pipeline = manifest
            .entries
            .iter()
            .find(|entry| entry.package == "fe2o3-pipeline")
            .expect("pipeline entry");
        let verus = manifest
            .entries
            .iter()
            .find(|entry| entry.package == "verus-vecadd")
            .expect("verus entry");

        assert_eq!(manifest.entries.len(), 25);
        assert_eq!(
            pipeline.artifacts,
            ["bias_stage.hsaco", "scale_stage.hsaco"]
        );
        assert!(verus.rustc_check);
        assert!(!verus.rocm_compile);
        assert!(!verus.gpu_smoke);
        assert!(verus.artifacts.is_empty());
    }

    #[test]
    fn normalizes_direct_and_cargo_subcommand_invocations() {
        for command in ["doctor", "build", "run", "smoke", "clean"] {
            let direct = vec![command.to_string(), "argument".to_string()];
            let cargo = vec![
                "fe2o3".to_string(),
                command.to_string(),
                "argument".to_string(),
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
    fn parses_explicit_package_args() {
        let args = [
            "-p",
            "fe2o3-vecadd",
            "--package=fe2o3-scale",
            "-pfe2o3-saxpy",
        ]
        .map(str::to_string);

        assert_eq!(
            explicit_packages(&args),
            ["fe2o3-vecadd", "fe2o3-scale", "fe2o3-saxpy"]
        );
    }

    #[test]
    fn ignores_package_args_after_program_separator() {
        let args = ["-p", "fe2o3-vecadd", "--", "-p", "program-arg"].map(str::to_string);

        assert_eq!(explicit_packages(&args), ["fe2o3-vecadd"]);
    }
}
