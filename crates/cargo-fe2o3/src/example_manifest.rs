use reserved_fe2o3_symbols::CrateBindingIdV1;
use rustix::fs::{FileType, Mode, OFlags, fstat, open, openat};
use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::os::unix::fs::MetadataExt as _;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use syn::parse::Parser;
use syn::visit::{self, Visit};
use syn::{Attribute, Expr, ExprMethodCall, ItemFn, Lit, Meta, Token, punctuated::Punctuated};

const MANIFEST_PATH: &str = "examples/regression-manifest-v1.txt";
const MANIFEST_VERSION: &str = "fe2o3-example-regressions-v1";
const MANIFEST_COLUMNS: &str = "package|rustc_check|rocm_compile|gpu_smoke|artifacts";
const KERNEL_IR_QUALIFICATION_PACKAGES: [&str; 2] = ["fe2o3-fill", "fe2o3-vecadd"];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Lane {
    All,
    RustcCheck,
    RocmCompile,
    GpuSmoke,
}

impl Lane {
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
struct Entry {
    package: String,
    rustc_check: bool,
    rocm_compile: bool,
    gpu_smoke: bool,
    artifacts: Vec<String>,
}

impl Entry {
    fn participates(&self, lane: Lane) -> bool {
        match lane {
            Lane::All => true,
            Lane::RustcCheck => self.rustc_check,
            Lane::RocmCompile => self.rocm_compile,
            Lane::GpuSmoke => self.gpu_smoke,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Manifest {
    entries: Vec<Entry>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct WorkspaceExample {
    package: String,
    artifacts: Vec<String>,
}

pub(crate) fn command(args: &[String]) -> ExitCode {
    match command_result(args) {
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

pub(crate) fn gpu_smoke_packages(workspace_root: &Path) -> Result<Vec<String>, String> {
    let manifest = load(workspace_root)?;
    Ok(manifest
        .entries
        .iter()
        .filter(|entry| entry.gpu_smoke)
        .map(|entry| entry.package.clone())
        .collect())
}

fn command_result(args: &[String]) -> Result<Vec<String>, String> {
    let artifact_inspection = matches!(args, [command, _, _] if command == "check-artifacts");
    let workspace_root = if artifact_inspection {
        find_manifest_root_without_cargo()?
    } else {
        crate::find_workspace_root()?
    };
    if let [command, packages @ ..] = args
        && command == "check-wrapper-namespaces"
        && !packages.is_empty()
    {
        validate_wrapper_derived_namespaces(&workspace_root, packages)?;
        return Ok(vec![format!(
            "managed wrapper namespaces: {} package(s)",
            packages.len()
        )]);
    }
    if matches!(args, [command, lane] if command == "list" && lane == "wrapper-managed") {
        return workspace_binding_managed_packages(&workspace_root);
    }
    // Artifact inspection consumes the exact root admitted by its caller. It must not
    // re-resolve Cargo or PATH after the build and accidentally inspect another target.
    let manifest = if artifact_inspection {
        let manifest = load_manifest_file(&workspace_root)?;
        validate_kernel_ir_qualification_lanes(&manifest)?;
        manifest
    } else {
        load(&workspace_root)?
    };

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
            let lane = Lane::parse(lane)?;
            Ok(manifest
                .entries
                .iter()
                .filter(|entry| entry.participates(lane))
                .map(|entry| entry.package.clone())
                .collect())
        }
        [command, package, artifact_directory] if command == "check-artifacts" => {
            let entry = manifest
                .entries
                .iter()
                .find(|entry| entry.package == *package)
                .ok_or_else(|| format!("package `{package}` is not in {MANIFEST_PATH}"))?;
            if !entry.rocm_compile {
                return Err(format!(
                    "package `{package}` does not participate in ROCm compilation"
                ));
            }

            let artifact_dir = PathBuf::from(artifact_directory);
            if !artifact_dir.is_absolute() {
                return Err(format!(
                    "artifact directory must be absolute: {}",
                    artifact_dir.display()
                ));
            }
            let metadata = artifact_dir.symlink_metadata().map_err(|error| {
                format!(
                    "failed to inspect artifact directory {}: {error}",
                    artifact_dir.display()
                )
            })?;
            if !metadata.file_type().is_dir() || metadata.file_type().is_symlink() {
                return Err(format!(
                    "artifact directory must be a non-symlink directory: {}",
                    artifact_dir.display()
                ));
            }
            let canonical = artifact_dir.canonicalize().map_err(|error| {
                format!(
                    "failed to canonicalize artifact directory {}: {error}",
                    artifact_dir.display()
                )
            })?;
            if canonical != artifact_dir {
                return Err(format!(
                    "artifact directory must already be canonical: {}",
                    artifact_dir.display()
                ));
            }
            let directory = open(
                &artifact_dir,
                OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::empty(),
            )
            .map_err(|error| {
                format!(
                    "failed to open artifact directory {}: {error}",
                    artifact_dir.display()
                )
            })?;
            let descriptor_stat = fstat(&directory).map_err(|error| {
                format!(
                    "failed to inspect opened artifact directory {}: {error}",
                    artifact_dir.display()
                )
            })?;
            let final_metadata = artifact_dir.symlink_metadata().map_err(|error| {
                format!(
                    "failed to re-inspect artifact directory {}: {error}",
                    artifact_dir.display()
                )
            })?;
            if FileType::from_raw_mode(descriptor_stat.st_mode) != FileType::Directory
                || descriptor_stat.st_dev != metadata.dev()
                || descriptor_stat.st_ino != metadata.ino()
                || descriptor_stat.st_dev != final_metadata.dev()
                || descriptor_stat.st_ino != final_metadata.ino()
            {
                return Err(format!(
                    "artifact directory changed while it was admitted: {}",
                    artifact_dir.display()
                ));
            }
            for artifact in &entry.artifacts {
                let path = artifact_dir.join(artifact);
                let artifact_file = openat(
                    &directory,
                    artifact,
                    OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
                    Mode::empty(),
                )
                .map_err(|_| {
                    format!(
                        "expected artifact for package `{package}` was not produced as a regular non-symlink file: {}",
                        path.display()
                    )
                })?;
                let artifact_stat = fstat(&artifact_file).map_err(|error| {
                    format!("failed to inspect artifact {}: {error}", path.display())
                })?;
                if FileType::from_raw_mode(artifact_stat.st_mode) != FileType::RegularFile {
                    return Err(format!(
                        "expected artifact for package `{package}` was not produced as a regular non-symlink file: {}",
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
            "usage: cargo fe2o3 examples <check|list <all|rustc-check|rocm-compile|gpu-smoke|wrapper-managed>|check-artifacts <package> <absolute-artifact-directory>|check-wrapper-namespaces <package>...>"
                .to_string(),
        ),
    }
}

fn find_manifest_root_without_cargo() -> Result<PathBuf, String> {
    let current = env::current_dir()
        .map_err(|error| format!("failed to resolve current directory: {error}"))?
        .canonicalize()
        .map_err(|error| format!("failed to canonicalize current directory: {error}"))?;
    current
        .ancestors()
        .find(|candidate| candidate.join(MANIFEST_PATH).is_file())
        .map(Path::to_path_buf)
        .ok_or_else(|| {
            format!(
                "could not find {MANIFEST_PATH} from invocation directory {}",
                current.display()
            )
        })
}

fn load(workspace_root: &Path) -> Result<Manifest, String> {
    let manifest = load_manifest_file(workspace_root)?;
    let path = workspace_root.join(MANIFEST_PATH);
    let workspace_examples = workspace_projection(workspace_root)?;
    validate_projection(&manifest, &workspace_examples)
        .map_err(|error| format!("invalid {}: {error}", path.display()))?;
    validate_kernel_ir_qualification_lanes(&manifest)
        .map_err(|error| format!("invalid {}: {error}", path.display()))?;
    Ok(manifest)
}

fn validate_kernel_ir_qualification_lanes(manifest: &Manifest) -> Result<(), String> {
    let expected = KERNEL_IR_QUALIFICATION_PACKAGES
        .into_iter()
        .collect::<BTreeSet<_>>();
    let lanes: [(&str, fn(&Entry) -> bool); 2] = [
        ("rocm_compile", |entry: &Entry| entry.rocm_compile),
        ("gpu_smoke", |entry: &Entry| entry.gpu_smoke),
    ];
    for (lane, enabled) in lanes {
        let actual = manifest
            .entries
            .iter()
            .filter(|entry| enabled(entry))
            .map(|entry| entry.package.as_str())
            .collect::<BTreeSet<_>>();
        if actual != expected {
            return Err(format!(
                "{lane} must select exactly the reviewed kernel-ir-v1 profiles [{}], found [{}]",
                expected.iter().copied().collect::<Vec<_>>().join(","),
                actual.iter().copied().collect::<Vec<_>>().join(",")
            ));
        }
    }
    Ok(())
}

fn load_manifest_file(workspace_root: &Path) -> Result<Manifest, String> {
    let path = workspace_root.join(MANIFEST_PATH);
    let contents = fs::read_to_string(&path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    parse(&contents).map_err(|error| format!("invalid {}: {error}", path.display()))
}

fn parse(contents: &str) -> Result<Manifest, String> {
    if !contents.ends_with('\n') {
        return Err("manifest must end with a newline".to_string());
    }
    if contents.contains('\r') {
        return Err("carriage returns are not permitted".to_string());
    }

    let mut lines = contents.lines();
    if lines.next() != Some(MANIFEST_VERSION) {
        return Err(format!("first line must be `{MANIFEST_VERSION}`"));
    }
    if lines.next() != Some(MANIFEST_COLUMNS) {
        return Err(format!("second line must be `{MANIFEST_COLUMNS}`"));
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

        let rustc_check = parse_bool(rustc_check, line_number, "rustc_check")?;
        let rocm_compile = parse_bool(rocm_compile, line_number, "rocm_compile")?;
        let gpu_smoke = parse_bool(gpu_smoke, line_number, "gpu_smoke")?;
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
        if rocm_compile == entry_artifacts.is_empty() {
            return Err(format!(
                "line {line_number}: rocm_compile must have one or more artifacts, and CPU-only entries must use `-`"
            ));
        }

        entries.push(Entry {
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

    Ok(Manifest { entries })
}

fn parse_bool(value: &str, line: usize, field: &str) -> Result<bool, String> {
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

fn workspace_projection(workspace_root: &Path) -> Result<Vec<WorkspaceExample>, String> {
    let metadata = cargo_metadata(workspace_root)?;
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

fn validate_wrapper_derived_namespaces(
    workspace_root: &Path,
    package_names: &[String],
) -> Result<(), String> {
    let metadata = cargo_metadata(workspace_root)?;
    let packages = metadata
        .get("packages")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| "cargo metadata did not contain a packages array".to_string())?;

    let mut requested = package_names.iter().cloned().collect::<BTreeSet<_>>();
    if requested.len() != package_names.len() {
        return Err("managed-wrapper package list contains duplicates".to_string());
    }
    for package in packages {
        let Some(name) = package.get("name").and_then(serde_json::Value::as_str) else {
            return Err("cargo metadata package has no name".to_string());
        };
        if !requested.remove(name) {
            continue;
        }
        let manifest = package
            .get("manifest_path")
            .and_then(serde_json::Value::as_str)
            .map(PathBuf::from)
            .ok_or_else(|| format!("cargo metadata package `{name}` has no manifest_path"))?;
        let package_root = manifest
            .parent()
            .ok_or_else(|| format!("cargo metadata package `{name}` has no package root"))?;
        let mut sources = Vec::new();
        collect_rust_sources(&package_root.join("src"), &mut sources)?;
        sources.sort();
        for source in sources {
            let contents = fs::read_to_string(&source)
                .map_err(|error| format!("failed to read {}: {error}", source.display()))?;
            if source_has_explicit_kernel_namespace(&contents)
                .map_err(|error| format!("failed to parse {}: {error}", source.display()))?
            {
                return Err(format!(
                    "managed-wrapper package `{name}` retains an explicit kernel namespace in {}",
                    source.display()
                ));
            }
        }
    }
    if !requested.is_empty() {
        return Err(format!(
            "managed-wrapper packages are absent from Cargo metadata: {}",
            requested.into_iter().collect::<Vec<_>>().join(", ")
        ));
    }
    Ok(())
}

fn cargo_metadata(workspace_root: &Path) -> Result<serde_json::Value, String> {
    let cargo = env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let mut command = Command::new(cargo);
    command
        .args(["metadata", "--locked", "--no-deps", "--format-version", "1"])
        .current_dir(workspace_root);
    let output = crate::process_execution::capture_output(&mut command)
        .map_err(|error| format!("failed to run cargo metadata: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "cargo metadata failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    serde_json::from_slice(&output.stdout)
        .map_err(|error| format!("failed to parse cargo metadata output: {error}"))
}

fn workspace_binding_managed_packages(workspace_root: &Path) -> Result<Vec<String>, String> {
    let metadata = cargo_metadata(workspace_root)?;
    let members = metadata
        .get("workspace_members")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| "cargo metadata did not contain workspace_members".to_owned())?
        .iter()
        .map(|member| {
            member
                .as_str()
                .map(str::to_owned)
                .ok_or_else(|| "cargo metadata workspace member was not a string".to_owned())
        })
        .collect::<Result<BTreeSet<_>, _>>()?;
    let packages = metadata
        .get("packages")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| "cargo metadata did not contain a packages array".to_owned())?;
    let mut managed = BTreeSet::new();
    for package in packages {
        let id = package
            .get("id")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| "cargo metadata package has no id".to_owned())?;
        if !members.contains(id) {
            continue;
        }
        let Some(name) = package.get("name").and_then(serde_json::Value::as_str) else {
            return Err("cargo metadata package has no name".to_string());
        };
        let manifest = package
            .get("manifest_path")
            .and_then(serde_json::Value::as_str)
            .map(PathBuf::from)
            .ok_or_else(|| format!("cargo metadata package `{name}` has no manifest_path"))?;
        let package_root = manifest
            .parent()
            .ok_or_else(|| format!("cargo metadata package `{name}` has no package root"))?;
        let sources = package_target_sources(package, package_root, name)?;
        for source in sources {
            let contents = fs::read_to_string(&source)
                .map_err(|error| format!("failed to read {}: {error}", source.display()))?;
            if source_requires_wrapper_binding(&contents)
                .map_err(|error| format!("failed to parse {}: {error}", source.display()))?
            {
                managed.insert(name.to_owned());
                break;
            }
        }
    }
    Ok(managed.into_iter().collect())
}

fn package_target_sources(
    package: &serde_json::Value,
    package_root: &Path,
    package_name: &str,
) -> Result<Vec<PathBuf>, String> {
    let mut sources = BTreeSet::new();
    let targets = package
        .get("targets")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| format!("cargo metadata package `{package_name}` has no targets"))?;
    for target in targets {
        let source = target
            .get("src_path")
            .and_then(serde_json::Value::as_str)
            .map(PathBuf::from)
            .ok_or_else(|| {
                format!("cargo metadata target in package `{package_name}` has no src_path")
            })?;
        let metadata = fs::symlink_metadata(&source)
            .map_err(|error| format!("failed to inspect {}: {error}", source.display()))?;
        if metadata.file_type().is_symlink() || !metadata.file_type().is_file() {
            return Err(format!(
                "Cargo target source must be a regular non-symlink file: {}",
                source.display()
            ));
        }
        sources.insert(source.clone());

        let source_root = package_root.join("src");
        if source.starts_with(&source_root) && source_root.is_dir() {
            let mut files = Vec::new();
            collect_rust_sources(&source_root, &mut files)?;
            sources.extend(files);
            continue;
        }
        let module_directory = if matches!(
            source.file_name().and_then(|name| name.to_str()),
            Some("main.rs" | "lib.rs")
        ) {
            source.parent().map(Path::to_path_buf)
        } else {
            Some(source.with_extension(""))
        };
        if let Some(module_directory) = module_directory
            && module_directory != package_root
            && module_directory.is_dir()
        {
            let mut files = Vec::new();
            collect_rust_sources(&module_directory, &mut files)?;
            sources.extend(files);
        }
    }
    Ok(sources.into_iter().collect())
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
        for artifact in source_artifact_literals(&source)
            .map_err(|error| format!("failed to inspect {}: {error}", source_path.display()))?
        {
            seen.insert(artifact);
        }
    }
    Ok(seen.into_iter().collect())
}

fn collect_rust_sources(directory: &Path, files: &mut Vec<PathBuf>) -> Result<(), String> {
    let root_type = fs::symlink_metadata(directory)
        .map_err(|error| format!("failed to inspect {}: {error}", directory.display()))?
        .file_type();
    if root_type.is_symlink() || !root_type.is_dir() {
        return Err(format!(
            "Rust source root must be a non-symlink directory: {}",
            directory.display()
        ));
    }
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
struct SourceArtifactVisitor {
    artifacts: Vec<String>,
}

#[derive(Default)]
struct ExplicitKernelNamespaceVisitor {
    found: bool,
}

#[derive(Default)]
struct WrapperBindingVisitor {
    found: bool,
}

impl<'ast> Visit<'ast> for ExplicitKernelNamespaceVisitor {
    fn visit_attribute(&mut self, attribute: &'ast Attribute) {
        let Meta::List(list) = &attribute.meta else {
            return;
        };
        if tokens_contain_namespace_assignment(list.tokens.clone()) {
            self.found = true;
        }
        visit::visit_attribute(self, attribute);
    }
}

fn tokens_contain_namespace_assignment(tokens: proc_macro2::TokenStream) -> bool {
    let tokens = tokens.into_iter().collect::<Vec<_>>();
    for (index, token) in tokens.iter().enumerate() {
        match token {
            proc_macro2::TokenTree::Group(group)
                if tokens_contain_namespace_assignment(group.stream()) =>
            {
                return true;
            }
            proc_macro2::TokenTree::Ident(ident)
                if ident == "namespace"
                    && matches!(
                        tokens.get(index + 1),
                        Some(proc_macro2::TokenTree::Punct(punct)) if punct.as_char() == '='
                    ) =>
            {
                return true;
            }
            _ => {}
        }
    }
    false
}

fn tokens_require_wrapper_binding(tokens: proc_macro2::TokenStream) -> bool {
    let tokens = tokens.into_iter().collect::<Vec<_>>();
    let typed = tokens
        .iter()
        .any(|token| matches!(token, proc_macro2::TokenTree::Ident(ident) if ident == "typed"));
    let namespace = tokens.iter().enumerate().any(|(index, token)| {
        matches!(token, proc_macro2::TokenTree::Ident(ident) if ident == "namespace")
            && matches!(
                tokens.get(index + 1),
                Some(proc_macro2::TokenTree::Punct(punct)) if punct.as_char() == '='
            )
    });
    (typed && !namespace)
        || tokens.iter().any(|token| {
            matches!(token, proc_macro2::TokenTree::Group(group) if tokens_require_wrapper_binding(group.stream()))
        })
}

fn source_has_explicit_kernel_namespace(source: &str) -> Result<bool, syn::Error> {
    let file = syn::parse_file(source)?;
    let mut visitor = ExplicitKernelNamespaceVisitor::default();
    visitor.visit_file(&file);
    Ok(visitor.found)
}

impl<'ast> Visit<'ast> for WrapperBindingVisitor {
    fn visit_attribute(&mut self, attribute: &'ast Attribute) {
        let Meta::List(list) = &attribute.meta else {
            return;
        };
        if tokens_require_wrapper_binding(list.tokens.clone()) {
            self.found = true;
        }
        visit::visit_attribute(self, attribute);
    }
}

fn source_requires_wrapper_binding(source: &str) -> Result<bool, syn::Error> {
    let file = syn::parse_file(source)?;
    let mut visitor = WrapperBindingVisitor::default();
    visitor.visit_file(&file);
    Ok(visitor.found)
}

impl<'ast> Visit<'ast> for SourceArtifactVisitor {
    fn visit_item_fn(&mut self, node: &'ast ItemFn) {
        if node.attrs.iter().any(is_typed_kernel_attribute) {
            self.artifacts.push(format!("{}.hsaco", node.sig.ident));
        }
        visit::visit_item_fn(self, node);
    }

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

fn is_typed_kernel_attribute(attribute: &Attribute) -> bool {
    let Meta::List(list) = &attribute.meta else {
        return false;
    };
    let Some(segment) = list.path.segments.last() else {
        return false;
    };
    if segment.ident != "kernel" || !segment.arguments.is_empty() {
        return false;
    }

    let Ok(arguments) = Punctuated::<Meta, Token![,]>::parse_terminated.parse2(list.tokens.clone())
    else {
        return false;
    };
    let mut typed = false;
    let mut namespace = false;
    for argument in arguments {
        match argument {
            Meta::Path(path) if path.is_ident("typed") && !typed => typed = true,
            Meta::NameValue(value) if value.path.is_ident("namespace") && !namespace => {
                let Expr::Lit(literal) = value.value else {
                    return false;
                };
                let Lit::Str(literal) = literal.lit else {
                    return false;
                };
                if CrateBindingIdV1::from_hex(&literal.value()).is_err() {
                    return false;
                }
                namespace = true;
            }
            _ => return false,
        }
    }
    typed
}

fn source_artifact_literals(source: &str) -> Result<Vec<String>, String> {
    let file = syn::parse_file(source).map_err(|error| format!("invalid Rust source: {error}"))?;
    let mut visitor = SourceArtifactVisitor::default();
    visitor.visit_file(&file);

    let mut artifacts = BTreeSet::new();
    for artifact in visitor.artifacts {
        validate_artifact_name(&artifact)
            .map_err(|_| format!("non-canonical HSACO join argument `{artifact}`"))?;
        artifacts.insert(artifact);
    }
    Ok(artifacts.into_iter().collect())
}

fn validate_projection(
    manifest: &Manifest,
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
        if declared_entry.rocm_compile && declared_entry.artifacts != current_entry.artifacts {
            return Err(format!(
                "package `{package}` artifact drift: declared [{}], current [{}]",
                declared_entry.artifacts.join(","),
                current_entry.artifacts.join(",")
            ));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        KERNEL_IR_QUALIFICATION_PACKAGES, Lane, MANIFEST_COLUMNS, MANIFEST_VERSION,
        WorkspaceExample, collect_rust_sources, load, parse, source_artifact_literals,
        source_has_explicit_kernel_namespace, source_requires_wrapper_binding, validate_projection,
    };
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_TEST_DIRECTORY: AtomicU64 = AtomicU64::new(0);

    struct TestDirectory(PathBuf);

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn test_directory() -> TestDirectory {
        loop {
            let suffix = NEXT_TEST_DIRECTORY.fetch_add(1, Ordering::Relaxed);
            let root = std::env::temp_dir().join(format!(
                "fe2o3-example-manifest-source-root-{}-{suffix}",
                std::process::id()
            ));
            match std::fs::create_dir(&root) {
                Ok(()) => return TestDirectory(root),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => panic!("failed to create isolated test root: {error}"),
            }
        }
    }

    fn example_manifest(rows: &str) -> String {
        format!("{MANIFEST_VERSION}\n{MANIFEST_COLUMNS}\n{rows}")
    }

    #[test]
    fn parses_strict_example_manifest_and_lane_projection() {
        let manifest = parse(&example_manifest(
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
        assert!(manifest.entries[0].participates(Lane::RustcCheck));
        assert!(manifest.entries[0].participates(Lane::All));
        assert!(manifest.entries[0].participates(Lane::RocmCompile));
        assert!(!manifest.entries[0].participates(Lane::GpuSmoke));
        assert!(manifest.entries[2].artifacts.is_empty());
    }

    #[test]
    fn structurally_detects_explicit_kernel_namespaces() {
        assert!(
            source_has_explicit_kernel_namespace(
                r#"#[kernel(typed, namespace = "0123456789abcdef")]
pub fn alpha() {}"#,
            )
            .expect("valid source")
        );
        assert!(
            source_has_explicit_kernel_namespace(
                r#"#[k(typed, namespace = "0123456789abcdef")]
pub fn aliased() {}"#,
            )
            .expect("valid alias source")
        );
        assert!(
            source_has_explicit_kernel_namespace(
                r#"#[cfg_attr(any(), kernel(typed, namespace = "0123456789abcdef"))]
pub fn configured() {}"#,
            )
            .expect("valid cfg_attr source")
        );
        assert!(
            !source_has_explicit_kernel_namespace(
                r#"const TEXT: &str = "namespace = hostile";
#[kernel(typed)]
pub fn alpha() {}"#,
            )
            .expect("valid source")
        );
    }

    #[test]
    fn structurally_detects_sources_that_require_wrapper_bindings() {
        for source in [
            "#[kernel(typed)] pub fn direct() {}",
            "#[renamed(typed)] pub fn alias() {}",
            "#[cfg_attr(feature = \"gpu\", kernel(typed))] pub fn configured() {}",
            "#[cfg_attr(feature = \"gpu\", kernel(typed), other(namespace = \"00\"))] pub fn nested() {}",
        ] {
            assert!(source_requires_wrapper_binding(source).expect("valid managed source"));
        }
        for source in [
            "#[kernel(typed, namespace = \"0123456789abcdef\")] pub fn fallback() {}",
            "#[cfg_attr(feature = \"gpu\", kernel(typed, namespace = \"00\"))] pub fn configured() {}",
            "const TEXT: &str = \"typed kernel\";",
        ] {
            assert!(!source_requires_wrapper_binding(source).expect("valid ordinary source"));
        }
    }

    #[test]
    fn source_collection_rejects_a_symlink_root() {
        let cleanup = test_directory();
        let root = &cleanup.0;
        let source = root.join("source");
        std::fs::create_dir(&source).expect("create source root");
        std::fs::write(source.join("lib.rs"), "pub fn ordinary() {}\n").expect("write source");
        let alias = root.join("source-alias");
        std::os::unix::fs::symlink(&source, &alias).expect("create source-root symlink");

        let error = collect_rust_sources(&alias, &mut Vec::new())
            .expect_err("symlink source root must fail closed");
        assert!(error.contains("non-symlink directory"), "{error}");
    }

    #[test]
    fn rejects_malformed_manifest_structure_and_fields() {
        let cases = [
            (
                "wrong-version\npackage|rustc_check|rocm_compile|gpu_smoke|artifacts\na|true|false|false|-\n".to_string(),
                "first line",
            ),
            (
                format!("{MANIFEST_VERSION}\npackage|unknown\na|true|false|false|-\n"),
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
            let error = parse(&contents).expect_err("manifest must fail");
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
            let error = parse(&example_manifest(rows)).expect_err("manifest must fail closed");
            assert!(
                error.contains(expected),
                "expected `{expected}` in `{error}`"
            );
        }
    }

    #[test]
    fn structurally_extracts_and_canonicalizes_source_artifacts() {
        let source = r##"
#[kernel(typed)]
pub fn vecadd() {}

#[fe2o3_device::kernel(typed)]
pub fn nested() {}

#[kernel(
    typed,
    namespace = "7c0e8b256bc76d2d17529f43ca8e2ee3480c40dfd019491bd4fb1fc22c4f5f2d"
)]
pub fn namespaced() {}

#[kernel(
    namespace = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
    typed,
)]
pub fn reordered() {}

#[kernel(typed, namespace = "not-a-binding")]
pub fn invalid_namespace() {}

#[kernel(typed, other = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef")]
pub fn unknown_option() {}

#[kernel]
pub fn ordinary() {}

fn inspect(root: &std::path::Path, dynamic: &str) {
    // root.join("commented.hsaco");
    // #[kernel(typed)] pub fn commented() {}
    let _quote = '\"';
    let _bytes = b"bytes.hsaco";
    let _raw_bytes = br#"raw_bytes.hsaco"#;
    let _detached = "detached.hsaco";
    let _typed_text = "#[kernel(typed)] pub fn string_literal() {}";
    ignored!(root.join("macro_tokens.hsaco"));
    let _beta = root.join(r#"beta.hsaco"#);
    let _dynamic = root.join(dynamic);
    let _not_join = root.with_file_name("not_joined.hsaco");
    let _alpha = root.join("alpha.hsaco");
}
"##;
        let artifacts = source_artifact_literals(source).expect("inspect Rust syntax");

        assert_eq!(
            artifacts,
            [
                "alpha.hsaco",
                "beta.hsaco",
                "namespaced.hsaco",
                "nested.hsaco",
                "reordered.hsaco",
                "vecadd.hsaco",
            ]
        );
    }

    #[test]
    fn structured_artifact_projection_rejects_bad_syntax_and_unsafe_names() {
        let malformed = source_artifact_literals("fn main( {").expect_err("invalid Rust");
        let unsafe_join = source_artifact_literals(
            r#"fn main() { std::path::Path::new(".").join("../escape.hsaco"); }"#,
        )
        .expect_err("unsafe join");
        let unsafe_kernel = source_artifact_literals(r#"#[kernel(typed)] pub fn Uppercase() {}"#)
            .expect_err("unsafe typed kernel name");

        assert!(malformed.contains("invalid Rust source"));
        assert!(unsafe_join.contains("non-canonical HSACO join argument"));
        assert!(unsafe_kernel.contains("non-canonical HSACO join argument"));
    }

    #[test]
    fn validates_manifest_against_workspace_projection() {
        let manifest = parse(&example_manifest(
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

        validate_projection(&manifest, &workspace).expect("matching projection");
    }

    #[test]
    fn rejects_missing_extra_and_drifted_workspace_projection() {
        let manifest = parse(&example_manifest(
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
            let error = validate_projection(&manifest, &workspace)
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
        let manifest = load(&workspace_root).expect("checked manifest validates");
        let scalar_gemm = manifest
            .entries
            .iter()
            .find(|entry| entry.package == "fe2o3-scalar-gemm-v1")
            .expect("scalar GEMM entry");
        let verus = manifest
            .entries
            .iter()
            .find(|entry| entry.package == "verus-vecadd")
            .expect("verus entry");

        assert_eq!(manifest.entries.len(), 26);
        let rocm = manifest
            .entries
            .iter()
            .filter(|entry| entry.rocm_compile)
            .map(|entry| entry.package.as_str())
            .collect::<Vec<_>>();
        assert_eq!(rocm, KERNEL_IR_QUALIFICATION_PACKAGES);
        let gpu = manifest
            .entries
            .iter()
            .filter(|entry| entry.gpu_smoke)
            .map(|entry| entry.package.as_str())
            .collect::<Vec<_>>();
        assert_eq!(gpu, KERNEL_IR_QUALIFICATION_PACKAGES);
        assert!(verus.rustc_check);
        assert!(!verus.rocm_compile);
        assert!(!verus.gpu_smoke);
        assert!(verus.artifacts.is_empty());
        assert!(scalar_gemm.rustc_check);
        assert!(!scalar_gemm.rocm_compile);
        assert!(!scalar_gemm.gpu_smoke);
        assert!(scalar_gemm.artifacts.is_empty());
    }
}
