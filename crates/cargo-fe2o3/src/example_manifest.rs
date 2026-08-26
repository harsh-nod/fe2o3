use reserved_fe2o3_symbols::CrateBindingIdV1;
use rustix::fs::{FileType, Mode, OFlags, fstat, open, openat};
use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::fs::File;
use std::io::Read as _;
use std::os::unix::ffi::{OsStrExt as _, OsStringExt as _};
use std::os::unix::fs::MetadataExt as _;
use std::path::{Component, Path, PathBuf};
use std::process::{Command, ExitCode, Output, Stdio};
use syn::parse::Parser;
use syn::visit::{self, Visit};
use syn::{Attribute, Expr, ExprMethodCall, ItemFn, Lit, Meta, Token, punctuated::Punctuated};

const MANIFEST_PATH: &str = "examples/regression-manifest-v2.txt";
const MANIFEST_VERSION: &str = "fe2o3-example-regressions-v2";
const MANIFEST_COLUMNS: &str = "package|rustc_check|artifact_qualification|source_artifacts";
const MAX_PACKAGE_SOURCE_DEPTH: usize = 64;
const MAX_PACKAGE_SOURCE_ENTRIES: usize = 65_536;
const MAX_PACKAGE_SOURCE_FILES: usize = 16_384;
const MAX_PACKAGE_SOURCE_FILE_BYTES: u64 = 8 * 1024 * 1024;
const MAX_PACKAGE_SOURCE_TREE_BYTES: u64 = 128 * 1024 * 1024;
const MAX_PACKAGE_SOURCE_NAME_BYTES: usize = 4 * 1024 * 1024;
const MAX_PACKAGE_SOURCE_MODULE_EDGES: usize = 65_536;
const MAX_PACKAGE_SOURCE_TOKEN_DEPTH: usize = 128;
const MAX_PACKAGE_SOURCE_TOKENS: usize = 1_000_000;
const MAX_WORKSPACE_PACKAGES: usize = 2_048;
const MAX_WORKSPACE_TARGETS: usize = 65_536;
const MAX_WORKSPACE_SOURCE_ENTRIES: usize = 1_000_000;
const MAX_WORKSPACE_SOURCE_FILES: usize = 262_144;
const MAX_WORKSPACE_SOURCE_BYTES: u64 = 512 * 1024 * 1024;
const MAX_WORKSPACE_SOURCE_NAME_BYTES: usize = 64 * 1024 * 1024;
const MAX_CARGO_METADATA_BYTES: usize = 32 * 1024 * 1024;
const MAX_CARGO_METADATA_STDERR_BYTES: usize = 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Lane {
    All,
    RustcCheck,
    ArtifactQualification,
    KernelIrV1,
}

impl Lane {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "all" => Ok(Self::All),
            "rustc-check" => Ok(Self::RustcCheck),
            "artifact-qualification" => Ok(Self::ArtifactQualification),
            "artifact-kernel-ir-v1" => Ok(Self::KernelIrV1),
            _ => Err(format!(
                "unknown example lane `{value}`; expected all, rustc-check, artifact-qualification, or artifact-kernel-ir-v1"
            )),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ArtifactQualification {
    None,
    KernelIrV1,
}

impl ArtifactQualification {
    fn parse(value: &str, line: usize) -> Result<Self, String> {
        match value {
            "none" => Ok(Self::None),
            "kernel-ir-v1" => Ok(Self::KernelIrV1),
            _ => Err(format!(
                "line {line}: artifact_qualification must be exactly `none` or `kernel-ir-v1`"
            )),
        }
    }

    const fn produces_artifacts(self) -> bool {
        !matches!(self, Self::None)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Entry {
    package: String,
    rustc_check: bool,
    artifact_qualification: ArtifactQualification,
    artifacts: Vec<String>,
}

impl Entry {
    fn participates(&self, lane: Lane) -> bool {
        match lane {
            Lane::All => true,
            Lane::RustcCheck => self.rustc_check,
            Lane::ArtifactQualification => self.artifact_qualification.produces_artifacts(),
            Lane::KernelIrV1 => self.artifact_qualification == ArtifactQualification::KernelIrV1,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Manifest {
    entries: Vec<Entry>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CpuTestLane {
    Raw,
    WrapperManaged,
}

impl CpuTestLane {
    fn parse(value: &str) -> Option<Self> {
        match value {
            "cpu-test-raw" => Some(Self::Raw),
            "cpu-test-wrapper-managed" => Some(Self::WrapperManaged),
            _ => None,
        }
    }
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
    if let [command, packages @ ..] = args
        && command == "check-wrapper-managed"
    {
        for package in packages {
            validate_package_name(package)?;
        }
        if packages.windows(2).any(|pair| pair[0] >= pair[1]) {
            return Err(
                "wrapper-managed package arguments must be strictly sorted and unique".to_owned(),
            );
        }
        let observed = workspace_binding_managed_packages(&workspace_root)?;
        if packages != observed.as_slice() {
            return Err(format!(
                "wrapper-managed package projection changed: expected [{}], observed [{}]",
                packages.join(","),
                observed.join(",")
            ));
        }
        return Ok(vec![format!(
            "wrapper-managed projection: {} package(s)",
            observed.len()
        )]);
    }
    // Artifact inspection consumes the exact root admitted by its caller. It must not
    // re-resolve Cargo or PATH after the build and accidentally inspect another target.
    let manifest = if artifact_inspection {
        load_manifest_file(&workspace_root)?
    } else {
        load(&workspace_root)?
    };

    if let [command, packages @ ..] = args
        && command == "check-cpu-test-partition"
    {
        let separators = packages
            .iter()
            .enumerate()
            .filter_map(|(index, package)| (package == "--").then_some(index))
            .collect::<Vec<_>>();
        let [separator] = separators.as_slice() else {
            return Err(
                "CPU test partition requires exactly one `--` separator between raw and wrapper-managed packages"
                    .to_owned(),
            );
        };
        let raw = &packages[..*separator];
        let wrapper_managed = &packages[*separator + 1..];
        validate_sorted_unique_package_list(raw, "raw CPU test")?;
        validate_sorted_unique_package_list(wrapper_managed, "wrapper-managed CPU test")?;

        let managed = workspace_binding_managed_packages(&workspace_root)?;
        let (expected_raw, expected_wrapper_managed) = cpu_test_partitions(&manifest, &managed)?;
        if raw != expected_raw.as_slice() || wrapper_managed != expected_wrapper_managed.as_slice()
        {
            return Err(format!(
                "CPU test package partition changed: expected raw [{}] and wrapper-managed [{}], observed raw [{}] and wrapper-managed [{}]",
                expected_raw.join(","),
                expected_wrapper_managed.join(","),
                raw.join(","),
                wrapper_managed.join(",")
            ));
        }
        return Ok(vec![format!(
            "CPU test partition: {} raw, {} wrapper-managed package(s)",
            raw.len(),
            wrapper_managed.len()
        )]);
    }

    if let [command, lane] = args
        && command == "list"
        && let Some(lane) = CpuTestLane::parse(lane)
    {
        let managed = workspace_binding_managed_packages(&workspace_root)?;
        let (raw, wrapper_managed) = cpu_test_partitions(&manifest, &managed)?;
        return Ok(match lane {
            CpuTestLane::Raw => raw,
            CpuTestLane::WrapperManaged => wrapper_managed,
        });
    }

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
            if !entry.artifact_qualification.produces_artifacts() {
                return Err(format!(
                    "package `{package}` has no artifact qualification route"
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
            "usage: cargo fe2o3 examples <check|list <all|rustc-check|artifact-qualification|artifact-kernel-ir-v1|wrapper-managed|cpu-test-raw|cpu-test-wrapper-managed>|check-artifacts <package> <absolute-artifact-directory>|check-cpu-test-partition <raw-package>... -- <wrapper-managed-package>...|check-wrapper-managed <package>...|check-wrapper-namespaces <package>...>"
                .to_string(),
        ),
    }
}

fn validate_sorted_unique_package_list(packages: &[String], kind: &str) -> Result<(), String> {
    for package in packages {
        validate_package_name(package)?;
    }
    if packages.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(format!(
            "{kind} package arguments must be strictly sorted and unique"
        ));
    }
    Ok(())
}

fn cpu_test_partitions(
    manifest: &Manifest,
    wrapper_managed_packages: &[String],
) -> Result<(Vec<String>, Vec<String>), String> {
    let wrapper_managed = wrapper_managed_packages
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    if wrapper_managed.len() != wrapper_managed_packages.len() {
        return Err("wrapper-managed package projection is not unique".to_owned());
    }

    let eligible = manifest
        .entries
        .iter()
        .filter(|entry| entry.rustc_check && !entry.artifact_qualification.produces_artifacts())
        .map(|entry| entry.package.as_str())
        .collect::<BTreeSet<_>>();
    let raw = eligible
        .difference(&wrapper_managed)
        .map(|package| (*package).to_owned())
        .collect::<Vec<_>>();
    let managed = eligible
        .intersection(&wrapper_managed)
        .map(|package| (*package).to_owned())
        .collect::<Vec<_>>();

    let reconstructed = raw
        .iter()
        .chain(&managed)
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    if reconstructed != eligible || raw.iter().any(|package| managed.contains(package)) {
        return Err("CPU test package partition is not disjoint and exhaustive".to_owned());
    }
    Ok((raw, managed))
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
    Ok(manifest)
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
        let [package, rustc_check, artifact_qualification, artifact_field] = fields.as_slice()
        else {
            return Err(format!(
                "line {line_number}: expected exactly four pipe-delimited fields"
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
        let artifact_qualification =
            ArtifactQualification::parse(artifact_qualification, line_number)?;
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

        if artifact_qualification.produces_artifacts() && entry_artifacts.is_empty() {
            return Err(format!(
                "line {line_number}: artifact qualification requires one or more source artifacts"
            ));
        }

        entries.push(Entry {
            package: package.to_string(),
            rustc_check,
            artifact_qualification,
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
    let output = bounded_command_output(
        &mut command,
        MAX_CARGO_METADATA_BYTES,
        MAX_CARGO_METADATA_STDERR_BYTES,
    )?;
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
    Ok(workspace_binding_projection(workspace_root, None)?.managed_package_names())
}

pub(crate) fn pinned_workspace_binding_projection(
    workspace_root: &Path,
    cargo: &crate::pinned_executable::PinnedExecutable,
) -> Result<crate::binding_check_projection::Projection, String> {
    workspace_binding_projection(workspace_root, Some(cargo))
}

fn workspace_binding_projection(
    workspace_root: &Path,
    pinned_cargo: Option<&crate::pinned_executable::PinnedExecutable>,
) -> Result<crate::binding_check_projection::Projection, String> {
    let workspace_root =
        canonical_contained_path(workspace_root, workspace_root, "workspace root")?;
    let workspace = crate::project::PinnedDirectory::open_existing(
        workspace_root.clone(),
        "wrapper-managed workspace root",
    )?;
    let metadata = if let Some(cargo) = pinned_cargo {
        let mut command = cargo
            .command()
            .map_err(|error| format!("failed to prepare pinned Cargo metadata: {error}"))?;
        command
            .as_command_mut()
            .args(["metadata", "--locked", "--no-deps", "--format-version", "1"])
            .current_dir(&workspace_root);
        crate::remove_dynamic_loader_environment(command.as_command_mut());
        let output = bounded_command_output(
            command.as_command_mut(),
            MAX_CARGO_METADATA_BYTES,
            MAX_CARGO_METADATA_STDERR_BYTES,
        )?;
        if !output.status.success() {
            return Err(format!(
                "pinned Cargo metadata failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }
        serde_json::from_slice(&output.stdout)
            .map_err(|error| format!("failed to parse pinned Cargo metadata: {error}"))?
    } else {
        cargo_metadata(&workspace_root)?
    };
    let metadata_workspace = metadata
        .get("workspace_root")
        .and_then(serde_json::Value::as_str)
        .map(PathBuf::from)
        .ok_or_else(|| "cargo metadata did not contain workspace_root".to_owned())?;
    if canonical_contained_path(
        &metadata_workspace,
        &workspace_root,
        "Cargo metadata workspace root",
    )? != workspace_root
    {
        return Err("Cargo metadata reported a substituted workspace root".to_owned());
    }
    let target_directory = validated_metadata_target_directory(&metadata)?;
    let pinned_target_directory = if target_directory.exists() {
        Some(crate::project::PinnedDirectory::open_existing(
            target_directory.clone(),
            "Cargo target directory projection exclusion",
        )?)
    } else {
        None
    };
    let member_records = metadata
        .get("workspace_members")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| "cargo metadata did not contain workspace_members".to_owned())?;
    if member_records.is_empty() || member_records.len() > MAX_WORKSPACE_PACKAGES {
        return Err(format!(
            "Cargo metadata workspace member count exceeds the {MAX_WORKSPACE_PACKAGES}-package bound"
        ));
    }
    let members = member_records
        .iter()
        .map(|member| {
            member
                .as_str()
                .map(str::to_owned)
                .ok_or_else(|| "cargo metadata workspace member was not a string".to_owned())
        })
        .collect::<Result<BTreeSet<_>, _>>()?;
    let mut unseen_members = members.clone();
    let packages = metadata
        .get("packages")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| "cargo metadata did not contain a packages array".to_owned())?;
    if packages.len() > MAX_WORKSPACE_PACKAGES {
        return Err(format!(
            "Cargo metadata package count exceeds the {MAX_WORKSPACE_PACKAGES}-package bound"
        ));
    }
    let mut metadata_target_count = 0_usize;
    for package in packages {
        let id = package
            .get("id")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| "cargo metadata package has no id".to_owned())?;
        if members.contains(id) {
            let targets = package
                .get("targets")
                .and_then(serde_json::Value::as_array)
                .ok_or_else(|| format!("cargo metadata workspace package `{id}` has no targets"))?;
            metadata_target_count =
                admit_workspace_target_count(metadata_target_count, targets.len())?;
        }
    }
    let mut projection_targets = Vec::new();
    projection_targets
        .try_reserve_exact(metadata_target_count)
        .map_err(|_| "failed to reserve bounded workspace target projection".to_owned())?;
    let mut package_names = BTreeSet::new();
    let mut workspace_scan = WorkspaceSourceScanBudget::default();
    for package in packages {
        let id = package
            .get("id")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| "cargo metadata package has no id".to_owned())?;
        if !members.contains(id) {
            continue;
        }
        if !unseen_members.remove(id) {
            return Err(format!(
                "cargo metadata repeats workspace package id `{id}`"
            ));
        }
        let Some(name) = package.get("name").and_then(serde_json::Value::as_str) else {
            return Err("cargo metadata package has no name".to_string());
        };
        if !package_names.insert(name.to_owned()) {
            return Err(format!(
                "cargo metadata repeats workspace package name `{name}`"
            ));
        }
        let manifest = package
            .get("manifest_path")
            .and_then(serde_json::Value::as_str)
            .map(PathBuf::from)
            .ok_or_else(|| format!("cargo metadata package `{name}` has no manifest_path"))?;
        let manifest = canonical_contained_path(
            &manifest,
            &workspace_root,
            &format!("Cargo manifest for package `{name}`"),
        )?;
        if manifest.file_name() != Some(OsStr::new("Cargo.toml")) {
            return Err(format!(
                "Cargo manifest for package `{name}` is not named Cargo.toml: {}",
                manifest.display()
            ));
        }
        let package_root = manifest
            .parent()
            .map(Path::to_path_buf)
            .ok_or_else(|| format!("cargo metadata package `{name}` has no package root"))?;
        let package_directory = crate::project::PinnedDirectory::open_existing(
            package_root.clone(),
            &format!("wrapper-managed package `{name}` root"),
        )?;
        let manifest_descriptor = openat(
            package_directory.file(),
            "Cargo.toml",
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(|error| format!("failed to retain manifest for package `{name}`: {error}"))?;
        let manifest_initial = fstat(&manifest_descriptor)
            .map(|stat| SourceObjectSnapshot::from_stat(&stat))
            .map_err(|error| format!("failed to inspect manifest for package `{name}`: {error}"))?;
        if FileType::from_raw_mode(manifest_initial.mode as _) != FileType::RegularFile {
            return Err(format!(
                "Cargo manifest for package `{name}` is not a regular file"
            ));
        }
        let manifest_descriptor = File::from(manifest_descriptor);

        let mut target_sources = validate_package_targets(
            package,
            &package_directory,
            &workspace_root,
            &package_root,
            name,
            Some(&target_directory),
        )?;
        let (requires_binding, usage) = package_source_tree_projection_with_targets(
            &workspace,
            &package_directory,
            name,
            &mut target_sources,
            Some(&target_directory),
            pinned_target_directory
                .as_ref()
                .map(crate::project::PinnedDirectory::identity_parts),
            &mut |_| {},
        )?;
        workspace_scan.admit(usage)?;
        revalidate_source_object(
            &manifest_descriptor,
            manifest_initial,
            name,
            Path::new("Cargo.toml"),
        )?;
        revalidate_retained_child(
            &package_directory,
            Path::new("Cargo.toml"),
            manifest_initial,
            name,
            "Cargo manifest",
        )?;
        package_directory.validate_path(&format!("wrapper-managed package `{name}` root"))?;
        workspace.validate_path("wrapper-managed workspace root")?;
        let (package_device, package_inode) = package_directory.identity_parts();
        for target in target_sources {
            let stat = fstat(&target.file).map_err(|error| {
                format!("failed to bind Cargo target source for package `{name}`: {error}")
            })?;
            projection_targets.push(crate::binding_check_projection::TargetSource {
                package_name: name.to_owned(),
                package_root: package_root.clone(),
                package_device,
                package_inode,
                source_path: target.path,
                source_identity: crate::binding_check_projection::ObjectIdentity::from_stat(&stat)?,
                managed: requires_binding,
            });
        }
    }
    if !unseen_members.is_empty() {
        return Err(format!(
            "cargo metadata omitted workspace member package records: {}",
            unseen_members.into_iter().collect::<Vec<_>>().join(", ")
        ));
    }
    if let Some(target) = &pinned_target_directory {
        target.validate_path("Cargo target directory projection exclusion")?;
    }
    let (workspace_device, workspace_inode) = workspace.identity_parts();
    projection_targets.sort_by(|left, right| {
        left.source_path
            .as_os_str()
            .as_bytes()
            .cmp(right.source_path.as_os_str().as_bytes())
    });
    let projection = crate::binding_check_projection::Projection {
        workspace_root,
        workspace_device,
        workspace_inode,
        targets: projection_targets,
    };
    projection.validate_and_encode()?;
    Ok(projection)
}

fn validated_metadata_target_directory(metadata: &serde_json::Value) -> Result<PathBuf, String> {
    let target_directory = metadata
        .get("target_directory")
        .and_then(serde_json::Value::as_str)
        .map(PathBuf::from)
        .ok_or_else(|| "cargo metadata did not contain target_directory".to_owned())?;
    if !target_directory.is_absolute() {
        return Err("cargo metadata target_directory is not absolute".to_owned());
    }
    let normalized = lexical_normalize_absolute(&target_directory)?;
    if normalized != target_directory {
        return Err("cargo metadata target_directory is not canonically spelled".to_owned());
    }
    if target_directory.exists() {
        let canonical = target_directory.canonicalize().map_err(|error| {
            format!(
                "failed to canonicalize Cargo target directory {}: {error}",
                target_directory.display()
            )
        })?;
        if canonical != target_directory {
            return Err(format!(
                "Cargo target directory contains a symlink or noncanonical component: {}",
                target_directory.display()
            ));
        }
    }
    Ok(target_directory)
}

fn bounded_command_output(
    command: &mut Command,
    stdout_limit: usize,
    stderr_limit: usize,
) -> Result<Output, String> {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = crate::process_execution::spawn(command)
        .map_err(|error| format!("failed to spawn bounded Cargo metadata command: {error}"))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "bounded Cargo metadata command has no stdout pipe".to_owned())?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| "bounded Cargo metadata command has no stderr pipe".to_owned())?;
    let stdout_reader = std::thread::spawn(move || read_bounded_pipe(stdout, stdout_limit));
    let stderr_reader = std::thread::spawn(move || read_bounded_pipe(stderr, stderr_limit));
    let status = child
        .wait()
        .map_err(|error| format!("failed to wait for Cargo metadata: {error}"))?;
    let stdout = stdout_reader
        .join()
        .map_err(|_| "Cargo metadata stdout reader panicked".to_owned())??;
    let stderr = stderr_reader
        .join()
        .map_err(|_| "Cargo metadata stderr reader panicked".to_owned())??;
    Ok(Output {
        status,
        stdout,
        stderr,
    })
}

fn read_bounded_pipe(mut pipe: impl std::io::Read, limit: usize) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(limit)
        .map_err(|_| "failed to reserve bounded Cargo metadata output".to_owned())?;
    let mut buffer = [0_u8; 8192];
    let mut overflow = false;
    loop {
        let count = pipe
            .read(&mut buffer)
            .map_err(|error| format!("failed to read Cargo metadata output: {error}"))?;
        if count == 0 {
            break;
        }
        let remaining = limit.saturating_sub(bytes.len());
        let retained = remaining.min(count);
        bytes.extend_from_slice(&buffer[..retained]);
        overflow |= retained != count;
    }
    if overflow {
        return Err(format!(
            "Cargo metadata output exceeds its {limit}-byte bound"
        ));
    }
    Ok(bytes)
}

fn admit_workspace_target_count(current: usize, additional: usize) -> Result<usize, String> {
    current
        .checked_add(additional)
        .filter(|count| *count <= MAX_WORKSPACE_TARGETS)
        .ok_or_else(|| {
            format!(
                "Cargo metadata target count exceeds the {MAX_WORKSPACE_TARGETS}-target workspace bound"
            )
        })
}

#[derive(Debug)]
struct RetainedTargetSource {
    path: PathBuf,
    file: File,
    initial: SourceObjectSnapshot,
}

fn validate_package_targets(
    package: &serde_json::Value,
    package_directory: &crate::project::PinnedDirectory,
    workspace_root: &Path,
    package_root: &Path,
    package_name: &str,
    cargo_target_directory: Option<&Path>,
) -> Result<Vec<RetainedTargetSource>, String> {
    let targets = package
        .get("targets")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| format!("cargo metadata package `{package_name}` has no targets"))?;
    let mut retained = Vec::new();
    let mut seen = BTreeSet::new();
    for target in targets {
        let source = target
            .get("src_path")
            .and_then(serde_json::Value::as_str)
            .map(PathBuf::from)
            .ok_or_else(|| {
                format!("cargo metadata target in package `{package_name}` has no src_path")
            })?;
        let source = canonical_contained_path(
            &source,
            workspace_root,
            &format!("Cargo target source in package `{package_name}`"),
        )?;
        if source.to_str().is_none() || source.extension() != Some(OsStr::new("rs")) {
            return Err(format!(
                "Cargo target source in package `{package_name}` must be a UTF-8 .rs path: {}",
                source.display()
            ));
        }
        if !source.starts_with(package_root) {
            return Err(format!(
                "Cargo target source in package `{package_name}` escapes package root {}: {}",
                package_root.display(),
                source.display()
            ));
        }
        if cargo_target_directory.is_some_and(|target| source.starts_with(target)) {
            return Err(format!(
                "Cargo target source in package `{package_name}` is beneath generated Cargo target directory {}",
                cargo_target_directory
                    .expect("checked target directory")
                    .display()
            ));
        }
        reject_target_in_nested_package_root(package_directory, &source, package_name)?;
        let file = open_contained_regular_file(
            package_directory,
            &source,
            &format!("Cargo target source in package `{package_name}`"),
        )?;
        if seen.insert(source.clone()) {
            let initial = fstat(&file)
                .map(|stat| SourceObjectSnapshot::from_stat(&stat))
                .map_err(|error| {
                    format!(
                        "failed to inspect Cargo target source in package `{package_name}`: {error}"
                    )
                })?;
            retained.push(RetainedTargetSource {
                path: source,
                file,
                initial,
            });
        }
    }
    Ok(retained)
}

fn reject_target_in_nested_package_root(
    package_directory: &crate::project::PinnedDirectory,
    source: &Path,
    package_name: &str,
) -> Result<(), String> {
    let relative = source
        .strip_prefix(package_directory.display_path())
        .map_err(|_| {
            format!(
                "Cargo target source in package `{package_name}` escaped its retained package root"
            )
        })?;
    let mut ancestor = package_directory.display_path().to_path_buf();
    let components = relative.components().collect::<Vec<_>>();
    for component in components.iter().take(components.len().saturating_sub(1)) {
        let Component::Normal(component) = component else {
            return Err(format!(
                "Cargo target source in package `{package_name}` has a non-normal component"
            ));
        };
        ancestor.push(component);
        let directory =
            open_contained_entry(package_directory, &ancestor, true, "Cargo target ancestor")?;
        match openat(
            &directory,
            "Cargo.toml",
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
            Mode::empty(),
        ) {
            Ok(manifest) => {
                let stat = fstat(&manifest)
                    .map_err(|error| format!("failed to inspect nested Cargo manifest: {error}"))?;
                if FileType::from_raw_mode(stat.st_mode) == FileType::RegularFile {
                    return Err(format!(
                        "Cargo target source in package `{package_name}` enters nested Cargo package root {}",
                        ancestor.display()
                    ));
                }
                return Err(format!(
                    "nested Cargo.toml below package `{package_name}` has an unsupported file type"
                ));
            }
            Err(rustix::io::Errno::NOENT) => {}
            Err(error) => {
                return Err(format!(
                    "failed to inspect nested Cargo ownership boundary below package `{package_name}`: {error}"
                ));
            }
        }
    }
    Ok(())
}

pub(crate) fn open_contained_regular_file(
    root: &crate::project::PinnedDirectory,
    path: &Path,
    kind: &str,
) -> Result<File, String> {
    open_contained_entry(root, path, false, kind)
}

fn revalidate_retained_child(
    directory: &crate::project::PinnedDirectory,
    relative: &Path,
    initial: SourceObjectSnapshot,
    package_name: &str,
    kind: &str,
) -> Result<(), String> {
    let path = directory.display_path().join(relative);
    let descriptor = open_contained_regular_file(directory, &path, kind).map_err(|error| {
        format!(
            "failed to reopen {kind} for package `{package_name}` at {}: {error}",
            relative.display()
        )
    })?;
    let reopened = fstat(&descriptor)
        .map(|stat| SourceObjectSnapshot::from_stat(&stat))
        .map_err(|error| {
            format!("failed to re-inspect {kind} for package `{package_name}`: {error}")
        })?;
    if reopened != initial {
        return Err(format!(
            "{kind} for package `{package_name}` was substituted while inspected: {}",
            relative.display()
        ));
    }
    Ok(())
}

fn open_contained_entry(
    root: &crate::project::PinnedDirectory,
    path: &Path,
    final_directory: bool,
    kind: &str,
) -> Result<File, String> {
    let relative = path.strip_prefix(root.display_path()).map_err(|_| {
        format!(
            "{kind} escapes retained root {}: {}",
            root.display_path().display(),
            path.display()
        )
    })?;
    let components = relative.components().collect::<Vec<_>>();
    if components.is_empty() {
        if !final_directory {
            return Err(format!("{kind} names the retained directory itself"));
        }
        return root.try_clone_for_transfer();
    }
    let mut directory = root.try_clone_for_transfer()?;
    for (index, component) in components.iter().enumerate() {
        let Component::Normal(component) = component else {
            return Err(format!(
                "{kind} has a non-normal path component: {}",
                path.display()
            ));
        };
        let final_component = index + 1 == components.len();
        let flags = if final_component && !final_directory {
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC
        } else {
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC
        };
        let entry = openat(&directory, Path::new(component), flags, Mode::empty())
            .map(File::from)
            .map_err(|error| {
                format!(
                    "failed to open retained {kind} component {:?}: {error}",
                    component
                )
            })?;
        let stat =
            fstat(&entry).map_err(|error| format!("failed to inspect retained {kind}: {error}"))?;
        let expected = if final_component && !final_directory {
            FileType::RegularFile
        } else {
            FileType::Directory
        };
        if FileType::from_raw_mode(stat.st_mode) != expected {
            return Err(format!("retained {kind} has an unsupported file type"));
        }
        directory = entry;
    }
    Ok(directory)
}

fn canonical_contained_path(path: &Path, root: &Path, kind: &str) -> Result<PathBuf, String> {
    if !path.is_absolute() || !root.is_absolute() {
        return Err(format!("{kind} must be absolute: {}", path.display()));
    }
    let normalized = lexical_normalize_absolute(path)?;
    let canonical = path
        .canonicalize()
        .map_err(|error| format!("failed to canonicalize {kind} {}: {error}", path.display()))?;
    if canonical != normalized {
        return Err(format!(
            "{kind} contains a symlink or noncanonical component: {}",
            path.display()
        ));
    }
    let canonical_root = root.canonicalize().map_err(|error| {
        format!(
            "failed to canonicalize containment root {}: {error}",
            root.display()
        )
    })?;
    if canonical_root != lexical_normalize_absolute(root)?
        || !canonical.starts_with(&canonical_root)
    {
        return Err(format!(
            "{kind} escapes canonical containment root {}: {}",
            canonical_root.display(),
            path.display()
        ));
    }
    Ok(canonical)
}

fn lexical_normalize_absolute(path: &Path) -> Result<PathBuf, String> {
    let mut normalized = PathBuf::from("/");
    for component in path.components() {
        match component {
            Component::RootDir | Component::CurDir => {}
            Component::Normal(component) => normalized.push(component),
            Component::ParentDir => {
                if !normalized.pop() {
                    return Err(format!(
                        "absolute path escapes filesystem root: {}",
                        path.display()
                    ));
                }
            }
            Component::Prefix(_) => {
                return Err(format!(
                    "unsupported absolute path prefix: {}",
                    path.display()
                ));
            }
        }
    }
    Ok(normalized)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SourceObjectSnapshot {
    device: u128,
    inode: u128,
    mode: u64,
    size: i128,
    modified_seconds: i128,
    modified_nanoseconds: i128,
    changed_seconds: i128,
    changed_nanoseconds: i128,
}

impl SourceObjectSnapshot {
    fn from_stat(stat: &rustix::fs::Stat) -> Self {
        Self {
            device: stat.st_dev as u128,
            inode: stat.st_ino as u128,
            mode: stat.st_mode as u64,
            size: stat.st_size as i128,
            modified_seconds: stat.st_mtime as i128,
            modified_nanoseconds: stat.st_mtime_nsec as i128,
            changed_seconds: stat.st_ctime as i128,
            changed_nanoseconds: stat.st_ctime_nsec as i128,
        }
    }
}

#[derive(Default)]
struct PackageSourceScanState {
    entries: usize,
    files: usize,
    bytes: u64,
    name_bytes: usize,
    rust_sources: BTreeSet<PathBuf>,
    source_snapshots: BTreeMap<PathBuf, SourceObjectSnapshot>,
    module_edges: Vec<(PathBuf, SourceModuleEdge)>,
    has_wrapper_binding: bool,
    has_explicit_namespace: bool,
    package_root: PathBuf,
    excluded_subtree: Option<PathBuf>,
    excluded_subtree_identity: Option<(u64, u64)>,
}

#[derive(Clone, Copy)]
struct PackageSourceScanUsage {
    entries: usize,
    files: usize,
    bytes: u64,
    name_bytes: usize,
}

#[derive(Default)]
struct WorkspaceSourceScanBudget {
    entries: usize,
    files: usize,
    bytes: u64,
    name_bytes: usize,
}

impl WorkspaceSourceScanBudget {
    fn admit(&mut self, usage: PackageSourceScanUsage) -> Result<(), String> {
        self.entries = self
            .entries
            .checked_add(usage.entries)
            .filter(|value| *value <= MAX_WORKSPACE_SOURCE_ENTRIES)
            .ok_or_else(|| "workspace source projection exceeds its entry bound".to_owned())?;
        self.files = self
            .files
            .checked_add(usage.files)
            .filter(|value| *value <= MAX_WORKSPACE_SOURCE_FILES)
            .ok_or_else(|| "workspace source projection exceeds its file bound".to_owned())?;
        self.bytes = self
            .bytes
            .checked_add(usage.bytes)
            .filter(|value| *value <= MAX_WORKSPACE_SOURCE_BYTES)
            .ok_or_else(|| "workspace source projection exceeds its byte bound".to_owned())?;
        self.name_bytes = self
            .name_bytes
            .checked_add(usage.name_bytes)
            .filter(|value| *value <= MAX_WORKSPACE_SOURCE_NAME_BYTES)
            .ok_or_else(|| "workspace source projection exceeds its name-byte bound".to_owned())?;
        Ok(())
    }
}

impl PackageSourceScanState {
    fn admit_name(&mut self, name: &OsStr) -> Result<(), String> {
        self.entries = self
            .entries
            .checked_add(1)
            .filter(|count| *count <= MAX_PACKAGE_SOURCE_ENTRIES)
            .ok_or_else(|| "package source tree exceeds its entry bound".to_owned())?;
        self.name_bytes = self
            .name_bytes
            .checked_add(name.as_bytes().len())
            .filter(|bytes| *bytes <= MAX_PACKAGE_SOURCE_NAME_BYTES)
            .ok_or_else(|| "package source tree exceeds its name-byte bound".to_owned())?;
        Ok(())
    }

    fn admit_source(&mut self, size: u64) -> Result<(), String> {
        self.files = self
            .files
            .checked_add(1)
            .filter(|count| *count <= MAX_PACKAGE_SOURCE_FILES)
            .ok_or_else(|| "package source tree exceeds its Rust-file bound".to_owned())?;
        if size > MAX_PACKAGE_SOURCE_FILE_BYTES {
            return Err(format!(
                "package Rust source exceeds {MAX_PACKAGE_SOURCE_FILE_BYTES} bytes"
            ));
        }
        self.bytes = self
            .bytes
            .checked_add(size)
            .filter(|bytes| *bytes <= MAX_PACKAGE_SOURCE_TREE_BYTES)
            .ok_or_else(|| "package source tree exceeds its byte bound".to_owned())?;
        Ok(())
    }

    fn admit_module_edges(
        &mut self,
        source: &Path,
        edges: Vec<SourceModuleEdge>,
    ) -> Result<(), String> {
        let total = self
            .module_edges
            .len()
            .checked_add(edges.len())
            .filter(|count| *count <= MAX_PACKAGE_SOURCE_MODULE_EDGES)
            .ok_or_else(|| "package source tree exceeds its module-edge bound".to_owned())?;
        self.module_edges.reserve(total - self.module_edges.len());
        self.module_edges
            .extend(edges.into_iter().map(|edge| (source.to_path_buf(), edge)));
        Ok(())
    }
}

#[cfg(test)]
fn package_source_tree_requires_binding(
    workspace: &crate::project::PinnedDirectory,
    package: &crate::project::PinnedDirectory,
    package_name: &str,
) -> Result<bool, String> {
    package_source_tree_requires_binding_with_targets(
        workspace,
        package,
        package_name,
        &mut [],
        &mut |_| {},
    )
}

#[cfg(test)]
fn package_source_tree_requires_binding_with_hook(
    workspace: &crate::project::PinnedDirectory,
    package: &crate::project::PinnedDirectory,
    package_name: &str,
    hook: &mut impl FnMut(&Path),
) -> Result<bool, String> {
    package_source_tree_requires_binding_with_targets(
        workspace,
        package,
        package_name,
        &mut [],
        hook,
    )
}

#[cfg(test)]
fn package_source_tree_requires_binding_with_targets(
    _workspace: &crate::project::PinnedDirectory,
    package: &crate::project::PinnedDirectory,
    package_name: &str,
    targets: &mut [RetainedTargetSource],
    hook: &mut impl FnMut(&Path),
) -> Result<bool, String> {
    package_source_tree_projection_with_targets(
        _workspace,
        package,
        package_name,
        targets,
        None,
        None,
        hook,
    )
    .map(|(managed, _)| managed)
}

fn package_source_tree_projection_with_targets(
    _workspace: &crate::project::PinnedDirectory,
    package: &crate::project::PinnedDirectory,
    package_name: &str,
    targets: &mut [RetainedTargetSource],
    cargo_target_directory: Option<&Path>,
    cargo_target_directory_identity: Option<(u64, u64)>,
    hook: &mut impl FnMut(&Path),
) -> Result<(bool, PackageSourceScanUsage), String> {
    let root = package.try_clone_for_transfer()?;
    let mut state = PackageSourceScanState {
        package_root: package.display_path().to_path_buf(),
        excluded_subtree: cargo_target_directory
            .filter(|target| target.starts_with(package.display_path()))
            .map(Path::to_path_buf),
        excluded_subtree_identity: cargo_target_directory_identity,
        ..PackageSourceScanState::default()
    };
    if state.excluded_subtree.as_deref() == Some(package.display_path()) {
        return Err(format!(
            "Cargo target directory cannot equal package `{package_name}` root"
        ));
    }
    for target in targets.iter_mut() {
        inspect_package_rust_source(
            &mut target.file,
            target.initial,
            &target.path,
            &mut state,
            package_name,
        )?;
    }
    scan_package_source_directory(
        &root,
        Path::new(""),
        0,
        false,
        &mut state,
        package_name,
        hook,
    )?;
    for target in targets.iter() {
        let relative = target.path.strip_prefix(package.display_path()).map_err(|_| {
            format!(
                "retained Cargo target for package `{package_name}` escaped its package directory"
            )
        })?;
        revalidate_source_object(&target.file, target.initial, package_name, &target.path)?;
        revalidate_retained_child(
            package,
            relative,
            target.initial,
            package_name,
            "Cargo target source",
        )?;
    }
    let external_edge = state.module_edges.iter().any(|(source, edge)| {
        !resolved_package_module_edges(source, edge)
            .iter()
            .any(|resolved| {
                resolved.starts_with(&state.package_root) && state.rust_sources.contains(resolved)
            })
    });
    if state.has_explicit_namespace && state.has_wrapper_binding {
        return Err(format!(
            "package `{package_name}` mixes an explicit fallback namespace with compiler-derived binding"
        ));
    }
    let managed = state.has_wrapper_binding || (!state.has_explicit_namespace && external_edge);
    let usage = PackageSourceScanUsage {
        entries: state.entries,
        files: state.files,
        bytes: state.bytes,
        name_bytes: state.name_bytes,
    };
    Ok((managed, usage))
}

fn resolve_literal_package_module_edge(source: &Path, edge: &Path) -> Option<PathBuf> {
    if edge.is_absolute() {
        return None;
    }
    let mut resolved = source
        .parent()
        .unwrap_or_else(|| Path::new(""))
        .to_path_buf();
    for component in edge.components() {
        match component {
            Component::CurDir => {}
            Component::Normal(component) => resolved.push(component),
            Component::ParentDir => {
                if !resolved.pop() {
                    return None;
                }
            }
            Component::RootDir | Component::Prefix(_) => return None,
        }
    }
    Some(resolved)
}

fn resolved_package_module_edges(source: &Path, edge: &SourceModuleEdge) -> Vec<PathBuf> {
    match edge {
        SourceModuleEdge::Literal(edge) => resolve_literal_package_module_edge(source, edge)
            .into_iter()
            .collect(),
        SourceModuleEdge::Conventional(name) => {
            let parent = source.parent().unwrap_or_else(|| Path::new(""));
            let stem_root = source.with_extension("");
            let mut candidates = BTreeSet::new();
            for root in [parent, stem_root.as_path()] {
                candidates.insert(root.join(name).with_extension("rs"));
                candidates.insert(root.join(name).join("mod.rs"));
            }
            candidates.into_iter().collect()
        }
        SourceModuleEdge::Unresolved => Vec::new(),
    }
}

fn scan_package_source_directory(
    directory: &File,
    relative: &Path,
    depth: usize,
    nested: bool,
    state: &mut PackageSourceScanState,
    package_name: &str,
    hook: &mut impl FnMut(&Path),
) -> Result<(), String> {
    if depth > MAX_PACKAGE_SOURCE_DEPTH {
        return Err(format!(
            "package `{package_name}` source tree exceeds depth {MAX_PACKAGE_SOURCE_DEPTH}"
        ));
    }
    let initial = fstat(directory)
        .map(|stat| SourceObjectSnapshot::from_stat(&stat))
        .map_err(|error| {
            format!("failed to inspect package `{package_name}` source directory: {error}")
        })?;
    if FileType::from_raw_mode(initial.mode as _) != FileType::Directory {
        return Err(format!(
            "package `{package_name}` source root is not a directory"
        ));
    }
    if nested && directory_has_nested_manifest(directory, relative, package_name)? {
        revalidate_source_object(directory, initial, package_name, relative)?;
        return Ok(());
    }

    for name in sorted_package_source_names(directory, state, package_name)? {
        let child_relative = relative.join(&name);
        let child_absolute = state.package_root.join(&child_relative);
        if state.excluded_subtree.as_ref() == Some(&child_absolute) {
            let expected = state.excluded_subtree_identity.ok_or_else(|| {
                format!(
                    "Cargo target directory appeared after metadata for package `{package_name}`"
                )
            })?;
            let excluded = openat(
                directory,
                Path::new(&name),
                OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::empty(),
            )
            .map_err(|error| {
                format!(
                    "failed to retain generated Cargo target directory for package `{package_name}`: {error}"
                )
            })?;
            let stat = fstat(&excluded).map_err(|error| {
                format!("failed to inspect generated Cargo target directory: {error}")
            })?;
            if (stat.st_dev, stat.st_ino) != expected {
                return Err(format!(
                    "generated Cargo target directory was substituted for package `{package_name}`"
                ));
            }
            continue;
        }
        hook(&child_relative);
        let descriptor = openat(
            directory,
            Path::new(&name),
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(|error| {
            format!(
                "package `{package_name}` source entry {} is unreadable or symlinked: {error}",
                child_relative.display()
            )
        })?;
        let entry_initial = fstat(&descriptor)
            .map(|stat| SourceObjectSnapshot::from_stat(&stat))
            .map_err(|error| {
                format!(
                    "failed to inspect package `{package_name}` source entry {}: {error}",
                    child_relative.display()
                )
            })?;
        match FileType::from_raw_mode(entry_initial.mode as _) {
            FileType::Directory => {
                let child = File::from(descriptor);
                scan_package_source_directory(
                    &child,
                    &child_relative,
                    depth + 1,
                    true,
                    state,
                    package_name,
                    hook,
                )?;
                revalidate_source_object(&child, entry_initial, package_name, &child_relative)?;
            }
            FileType::RegularFile => {
                if Path::new(&name).extension() == Some(OsStr::new("rs")) {
                    let mut source = File::from(descriptor);
                    inspect_package_rust_source(
                        &mut source,
                        entry_initial,
                        &child_relative,
                        state,
                        package_name,
                    )?;
                }
            }
            _ => {
                return Err(format!(
                    "package `{package_name}` source entry {} is not a regular file or directory",
                    child_relative.display()
                ));
            }
        }
    }
    revalidate_source_object(directory, initial, package_name, relative)
}

fn directory_has_nested_manifest(
    directory: &File,
    relative: &Path,
    package_name: &str,
) -> Result<bool, String> {
    let descriptor = match openat(
        directory,
        "Cargo.toml",
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
        Mode::empty(),
    ) {
        Ok(descriptor) => descriptor,
        Err(rustix::io::Errno::NOENT) => return Ok(false),
        Err(error) => {
            return Err(format!(
                "failed to inspect nested package boundary in package `{package_name}` at {}: {error}",
                relative.display()
            ));
        }
    };
    let initial = fstat(&descriptor)
        .map(|stat| SourceObjectSnapshot::from_stat(&stat))
        .map_err(|error| format!("failed to inspect nested Cargo.toml: {error}"))?;
    if FileType::from_raw_mode(initial.mode as _) != FileType::RegularFile {
        return Err(format!(
            "nested Cargo.toml in package `{package_name}` at {} is not a regular file",
            relative.display()
        ));
    }
    let file = File::from(descriptor);
    revalidate_source_object(&file, initial, package_name, &relative.join("Cargo.toml"))?;
    Ok(true)
}

fn sorted_package_source_names(
    directory: &File,
    state: &mut PackageSourceScanState,
    package_name: &str,
) -> Result<Vec<OsString>, String> {
    let scan = openat(
        directory,
        ".",
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(|error| {
        format!("failed to retain package `{package_name}` source directory: {error}")
    })?;
    let mut entries = rustix::fs::Dir::read_from(&scan).map_err(|error| {
        format!("failed to enumerate package `{package_name}` source tree: {error}")
    })?;
    let mut names = Vec::new();
    for entry in &mut entries {
        let entry = entry.map_err(|error| {
            format!("failed to enumerate package `{package_name}` source tree: {error}")
        })?;
        let bytes = entry.file_name().to_bytes();
        if matches!(bytes, b"." | b"..") {
            continue;
        }
        let name = OsString::from_vec(bytes.to_vec());
        state.admit_name(&name)?;
        names.push(name);
    }
    names.sort_by(|left, right| left.as_bytes().cmp(right.as_bytes()));
    Ok(names)
}

fn inspect_package_rust_source(
    source: &mut File,
    initial: SourceObjectSnapshot,
    relative: &Path,
    state: &mut PackageSourceScanState,
    package_name: &str,
) -> Result<(), String> {
    let source_path = if relative.is_absolute() {
        relative.to_path_buf()
    } else {
        state.package_root.join(relative)
    };
    if let Some(retained) = state.source_snapshots.get(&source_path) {
        revalidate_source_object(source, initial, package_name, relative)?;
        if retained != &initial {
            return Err(format!(
                "package `{package_name}` Rust source was substituted while retained: {}",
                relative.display()
            ));
        }
        return Ok(());
    }
    let size = u64::try_from(initial.size).map_err(|_| {
        format!(
            "package `{package_name}` Rust source has a negative size: {}",
            relative.display()
        )
    })?;
    state.admit_source(size)?;
    let mut bytes = Vec::with_capacity(usize::try_from(size.min(64 * 1024)).unwrap_or(0));
    source
        .take(size + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| {
            format!(
                "failed to read package `{package_name}` Rust source {}: {error}",
                relative.display()
            )
        })?;
    if bytes.len() as u64 != size {
        return Err(format!(
            "package `{package_name}` Rust source changed length while read: {}",
            relative.display()
        ));
    }
    revalidate_source_object(source, initial, package_name, relative)?;
    let contents = std::str::from_utf8(&bytes).map_err(|_| {
        format!(
            "package `{package_name}` Rust source is not UTF-8: {}",
            relative.display()
        )
    })?;
    state.source_snapshots.insert(source_path.clone(), initial);
    state.rust_sources.insert(source_path.clone());
    state.has_wrapper_binding |= source_requires_wrapper_binding(contents).map_err(|error| {
        format!(
            "failed to structurally classify package `{package_name}` Rust source {}: {error}",
            relative.display()
        )
    })?;
    state.has_explicit_namespace |=
        source_has_explicit_kernel_namespace(contents).map_err(|error| {
            format!(
                "failed to classify explicit namespace in package `{package_name}` Rust source {}: {error}",
                relative.display()
            )
        })?;
    let edges = source_module_edges(contents).map_err(|error| {
        format!(
            "failed to classify module edges in package `{package_name}` Rust source {}: {error}",
            relative.display()
        )
    })?;
    state.admit_module_edges(&source_path, edges)?;
    Ok(())
}

fn revalidate_source_object(
    object: &File,
    initial: SourceObjectSnapshot,
    package_name: &str,
    relative: &Path,
) -> Result<(), String> {
    let final_snapshot = fstat(object)
        .map(|stat| SourceObjectSnapshot::from_stat(&stat))
        .map_err(|error| {
            format!(
                "failed to re-inspect package `{package_name}` source entry {}: {error}",
                relative.display()
            )
        })?;
    if final_snapshot != initial {
        return Err(format!(
            "package `{package_name}` source entry changed while inspected: {}",
            relative.display()
        ));
    }
    Ok(())
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
    error: Option<syn::Error>,
}

#[derive(Default)]
struct WrapperBindingVisitor {
    found: bool,
    error: Option<syn::Error>,
}

#[derive(Default)]
struct ExternalModuleEdgeVisitor {
    edges: Vec<SourceModuleEdge>,
    error: Option<syn::Error>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum SourceModuleEdge {
    Literal(PathBuf),
    Conventional(String),
    Unresolved,
}

impl<'ast> Visit<'ast> for ExplicitKernelNamespaceVisitor {
    fn visit_attribute(&mut self, attribute: &'ast Attribute) {
        let Meta::List(list) = &attribute.meta else {
            return;
        };
        match tokens_contain_namespace_assignment(list.tokens.clone()) {
            Ok(found) => self.found |= found,
            Err(error) => self.error = Some(error),
        }
        visit::visit_attribute(self, attribute);
    }

    fn visit_macro(&mut self, invocation: &'ast syn::Macro) {
        let mut found = false;
        let result = walk_macro_token_body(
            invocation.tokens.clone(),
            |attribute| {
                if tokens_contain_namespace_assignment(attribute)? {
                    found = true;
                }
                Ok(())
            },
            |_| Ok(()),
        );
        self.found |= found;
        if let Err(error) = result {
            self.error = Some(error);
        }
        visit::visit_macro(self, invocation);
    }
}

fn token_bound_error(kind: &str) -> syn::Error {
    syn::Error::new(
        proc_macro2::Span::call_site(),
        format!("{kind} exceeds the bounded source-token policy"),
    )
}

fn walk_attribute_tokens(
    tokens: proc_macro2::TokenStream,
    mut inspect: impl FnMut(&[proc_macro2::TokenTree]),
) -> Result<(), syn::Error> {
    let mut pending = vec![(tokens, 0usize)];
    let mut total = 0usize;
    while let Some((stream, depth)) = pending.pop() {
        if depth > MAX_PACKAGE_SOURCE_TOKEN_DEPTH {
            return Err(token_bound_error("attribute nesting"));
        }
        let tokens = stream.into_iter().collect::<Vec<_>>();
        total = total
            .checked_add(tokens.len())
            .filter(|count| *count <= MAX_PACKAGE_SOURCE_TOKENS)
            .ok_or_else(|| token_bound_error("attribute token count"))?;
        inspect(&tokens);
        for token in tokens.into_iter().rev() {
            if let proc_macro2::TokenTree::Group(group) = token {
                pending.push((group.stream(), depth + 1));
            }
        }
    }
    Ok(())
}

fn token_slice_has_assignment(tokens: &[proc_macro2::TokenTree], name: &str) -> bool {
    tokens.iter().enumerate().any(|(index, token)| {
        matches!(token, proc_macro2::TokenTree::Ident(ident) if ident == name)
            && matches!(
                tokens.get(index + 1),
                Some(proc_macro2::TokenTree::Punct(punct)) if punct.as_char() == '='
            )
    })
}

fn tokens_contain_namespace_assignment(
    tokens: proc_macro2::TokenStream,
) -> Result<bool, syn::Error> {
    let mut found = false;
    walk_attribute_tokens(tokens, |tokens| {
        let typed = tokens
            .iter()
            .any(|token| matches!(token, proc_macro2::TokenTree::Ident(ident) if ident == "typed"));
        found |= typed && token_slice_has_assignment(tokens, "namespace");
    })?;
    Ok(found)
}

fn tokens_require_wrapper_binding(tokens: proc_macro2::TokenStream) -> Result<bool, syn::Error> {
    let mut found = false;
    walk_attribute_tokens(tokens, |tokens| {
        let typed = tokens
            .iter()
            .any(|token| matches!(token, proc_macro2::TokenTree::Ident(ident) if ident == "typed"));
        if typed && !token_slice_has_assignment(tokens, "namespace") {
            found = true;
        }
    })?;
    Ok(found)
}

fn collect_module_edges_from_meta(
    meta: &Meta,
    edges: &mut Vec<SourceModuleEdge>,
) -> Result<(), syn::Error> {
    match meta {
        Meta::NameValue(value) if value.path.is_ident("path") => {
            let edge = match &value.value {
                Expr::Lit(literal) => match &literal.lit {
                    Lit::Str(literal) => SourceModuleEdge::Literal(PathBuf::from(literal.value())),
                    _ => SourceModuleEdge::Unresolved,
                },
                _ => SourceModuleEdge::Unresolved,
            };
            push_bounded_module_edge(edges, edge)?;
        }
        Meta::List(list) if list.path.is_ident("cfg_attr") => {
            let nested =
                Punctuated::<Meta, Token![,]>::parse_terminated.parse2(list.tokens.clone())?;
            for meta in nested.into_iter().skip(1) {
                collect_module_edges_from_meta(&meta, edges)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn collect_module_edges_from_attribute_tokens(
    tokens: proc_macro2::TokenStream,
    edges: &mut Vec<SourceModuleEdge>,
) -> Result<(), syn::Error> {
    let meta = syn::parse2::<Meta>(tokens)?;
    collect_module_edges_from_meta(&meta, edges)
}

fn walk_macro_token_body(
    tokens: proc_macro2::TokenStream,
    mut attribute: impl FnMut(proc_macro2::TokenStream) -> Result<(), syn::Error>,
    mut include: impl FnMut(proc_macro2::TokenStream) -> Result<(), syn::Error>,
) -> Result<(), syn::Error> {
    let mut pending = vec![(tokens, 0usize)];
    let mut total = 0usize;
    while let Some((stream, depth)) = pending.pop() {
        if depth > MAX_PACKAGE_SOURCE_TOKEN_DEPTH {
            return Err(token_bound_error("macro nesting"));
        }
        let tokens = stream.into_iter().collect::<Vec<_>>();
        total = total
            .checked_add(tokens.len())
            .filter(|count| *count <= MAX_PACKAGE_SOURCE_TOKENS)
            .ok_or_else(|| token_bound_error("macro token count"))?;
        for (index, token) in tokens.iter().enumerate() {
            let proc_macro2::TokenTree::Group(group) = token else {
                continue;
            };
            let attribute_group = group.delimiter() == proc_macro2::Delimiter::Bracket
                && (matches!(
                    tokens.get(index.wrapping_sub(1)),
                    Some(proc_macro2::TokenTree::Punct(punct)) if punct.as_char() == '#'
                ) || (matches!(
                    tokens.get(index.wrapping_sub(1)),
                    Some(proc_macro2::TokenTree::Punct(punct)) if punct.as_char() == '!'
                ) && matches!(
                    tokens.get(index.wrapping_sub(2)),
                    Some(proc_macro2::TokenTree::Punct(punct)) if punct.as_char() == '#'
                )));
            let include_group = index >= 2
                && matches!(
                    tokens.get(index - 2),
                    Some(proc_macro2::TokenTree::Ident(ident)) if ident == "include"
                )
                && matches!(
                    tokens.get(index - 1),
                    Some(proc_macro2::TokenTree::Punct(punct)) if punct.as_char() == '!'
                );
            if attribute_group {
                attribute(group.stream())?;
            } else if include_group {
                include(group.stream())?;
            } else {
                pending.push((group.stream(), depth + 1));
            }
        }
    }
    Ok(())
}

fn source_has_explicit_kernel_namespace(source: &str) -> Result<bool, syn::Error> {
    let file = syn::parse_file(source)?;
    let mut visitor = ExplicitKernelNamespaceVisitor::default();
    visitor.visit_file(&file);
    if let Some(error) = visitor.error {
        return Err(error);
    }
    Ok(visitor.found)
}

impl<'ast> Visit<'ast> for WrapperBindingVisitor {
    fn visit_attribute(&mut self, attribute: &'ast Attribute) {
        let Meta::List(list) = &attribute.meta else {
            return;
        };
        match tokens_require_wrapper_binding(list.tokens.clone()) {
            Ok(found) => self.found |= found,
            Err(error) => self.error = Some(error),
        }
        visit::visit_attribute(self, attribute);
    }

    fn visit_macro(&mut self, invocation: &'ast syn::Macro) {
        let mut found = false;
        let result = walk_macro_token_body(
            invocation.tokens.clone(),
            |attribute| {
                if tokens_require_wrapper_binding(attribute)? {
                    found = true;
                }
                Ok(())
            },
            |_| Ok(()),
        );
        self.found |= found;
        if let Err(error) = result {
            self.error = Some(error);
        }
        visit::visit_macro(self, invocation);
    }
}

fn source_requires_wrapper_binding(source: &str) -> Result<bool, syn::Error> {
    let file = syn::parse_file(source)?;
    let mut visitor = WrapperBindingVisitor::default();
    visitor.visit_file(&file);
    if let Some(error) = visitor.error {
        return Err(error);
    }
    Ok(visitor.found)
}

fn push_bounded_module_edge(
    edges: &mut Vec<SourceModuleEdge>,
    edge: SourceModuleEdge,
) -> Result<(), syn::Error> {
    if edges.len() >= MAX_PACKAGE_SOURCE_MODULE_EDGES {
        return Err(token_bound_error("module-edge count"));
    }
    edges.push(edge);
    Ok(())
}

impl<'ast> Visit<'ast> for ExternalModuleEdgeVisitor {
    fn visit_attribute(&mut self, attribute: &'ast Attribute) {
        if let Err(error) = collect_module_edges_from_meta(&attribute.meta, &mut self.edges) {
            self.error = Some(error);
        }
        visit::visit_attribute(self, attribute);
    }

    fn visit_macro(&mut self, invocation: &'ast syn::Macro) {
        let mut edges = Vec::new();
        if invocation
            .path
            .segments
            .last()
            .is_some_and(|segment| segment.ident == "include")
        {
            edges.push(
                syn::parse2::<syn::LitStr>(invocation.tokens.clone())
                    .ok()
                    .map(|literal| SourceModuleEdge::Literal(PathBuf::from(literal.value())))
                    .unwrap_or(SourceModuleEdge::Unresolved),
            );
        }
        let mut attribute_edges = Vec::new();
        let mut include_edges = Vec::new();
        let result = walk_macro_token_body(
            invocation.tokens.clone(),
            |attribute| match collect_module_edges_from_attribute_tokens(
                attribute,
                &mut attribute_edges,
            ) {
                Ok(()) => Ok(()),
                Err(_) => {
                    push_bounded_module_edge(&mut attribute_edges, SourceModuleEdge::Unresolved)
                }
            },
            |include| {
                push_bounded_module_edge(
                    &mut include_edges,
                    syn::parse2::<syn::LitStr>(include)
                        .ok()
                        .map(|literal| SourceModuleEdge::Literal(PathBuf::from(literal.value())))
                        .unwrap_or(SourceModuleEdge::Unresolved),
                )
            },
        );
        edges.extend(attribute_edges);
        edges.extend(include_edges);
        if let Err(error) = result {
            self.error = Some(error);
        }
        for edge in edges {
            if let Err(error) = push_bounded_module_edge(&mut self.edges, edge) {
                self.error = Some(error);
                break;
            }
        }
        visit::visit_macro(self, invocation);
    }

    fn visit_item_mod(&mut self, module: &'ast syn::ItemMod) {
        if module.content.is_none() {
            let has_unconditional_path = module.attrs.iter().any(
                |attribute| {
                    matches!(&attribute.meta, Meta::NameValue(value) if value.path.is_ident("path"))
                },
            );
            if !has_unconditional_path
                && let Err(error) = push_bounded_module_edge(
                    &mut self.edges,
                    SourceModuleEdge::Conventional(module.ident.to_string()),
                )
            {
                self.error = Some(error);
            }
        }
        visit::visit_item_mod(self, module);
    }
}

#[cfg(test)]
fn source_has_external_module_edge(source: &str) -> Result<bool, syn::Error> {
    Ok(!source_module_edges(source)?.is_empty())
}

fn source_module_edges(source: &str) -> Result<Vec<SourceModuleEdge>, syn::Error> {
    let file = syn::parse_file(source)?;
    let mut visitor = ExternalModuleEdgeVisitor::default();
    visitor.visit_file(&file);
    if let Some(error) = visitor.error {
        return Err(error);
    }
    Ok(visitor.edges)
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

#[cfg(test)]
mod tests {
    use super::{
        ArtifactQualification, Lane, MANIFEST_COLUMNS, MANIFEST_VERSION, MAX_PACKAGE_SOURCE_DEPTH,
        MAX_PACKAGE_SOURCE_FILE_BYTES, MAX_PACKAGE_SOURCE_MODULE_EDGES,
        MAX_PACKAGE_SOURCE_TOKEN_DEPTH, PackageSourceScanState, SourceObjectSnapshot,
        WorkspaceExample, canonical_contained_path, collect_rust_sources, cpu_test_partitions,
        load, package_source_tree_requires_binding, package_source_tree_requires_binding_with_hook,
        package_source_tree_requires_binding_with_targets, parse, revalidate_retained_child,
        source_artifact_literals, source_has_explicit_kernel_namespace,
        source_has_external_module_edge, source_requires_wrapper_binding, validate_package_targets,
        validate_projection,
    };
    use crate::project::PinnedDirectory;
    use serde_json::json;
    use std::fs::OpenOptions;
    use std::path::{Path, PathBuf};
    use std::process::Command;
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

    fn write(root: &Path, relative: &str, contents: &str) {
        let path = root.join(relative);
        std::fs::create_dir_all(path.parent().expect("test file parent"))
            .expect("create test file parent");
        std::fs::write(path, contents).expect("write test source");
    }

    fn scan_test_package(root: &Path) -> Result<bool, String> {
        let pinned = PinnedDirectory::open_existing(
            root.canonicalize().expect("canonical test package"),
            "test package",
        )?;
        package_source_tree_requires_binding(&pinned, &pinned, "test-package")
    }

    fn scan_test_workspace(workspace_root: &Path, package_root: &Path) -> Result<bool, String> {
        let workspace = PinnedDirectory::open_existing(
            workspace_root
                .canonicalize()
                .expect("canonical test workspace"),
            "test workspace",
        )?;
        let package = PinnedDirectory::open_existing(
            package_root.canonicalize().expect("canonical test package"),
            "test package",
        )?;
        package_source_tree_requires_binding(&workspace, &package, "test-package")
    }

    #[test]
    fn parses_strict_example_manifest_and_lane_projection() {
        let manifest = parse(&example_manifest(
            "fe2o3-alpha|true|kernel-ir-v1|alpha.hsaco\n\
             fe2o3-pipeline|true|none|bias_stage.hsaco,scale_stage.hsaco\n\
             verus-vecadd|true|none|-\n",
        ))
        .expect("valid manifest");

        assert_eq!(manifest.entries.len(), 3);
        assert_eq!(
            manifest.entries[1].artifacts,
            ["bias_stage.hsaco", "scale_stage.hsaco"]
        );
        assert!(manifest.entries[0].participates(Lane::RustcCheck));
        assert!(manifest.entries[0].participates(Lane::All));
        assert!(manifest.entries[0].participates(Lane::ArtifactQualification));
        assert!(manifest.entries[0].participates(Lane::KernelIrV1));
        assert!(!manifest.entries[1].participates(Lane::ArtifactQualification));
        assert!(manifest.entries[2].artifacts.is_empty());
    }

    #[test]
    fn cpu_test_partition_is_sorted_disjoint_and_exhaustive() {
        let manifest = parse(&example_manifest(
            "fe2o3-managed|true|none|-\n\
             fe2o3-raw|true|none|-\n\
             fe2o3-rocm|true|kernel-ir-v1|rocm.hsaco\n\
             fe2o3-unchecked|false|none|-\n",
        ))
        .expect("valid manifest");
        let managed = vec![
            "fe2o3-managed".to_owned(),
            "fe2o3-non-example-managed".to_owned(),
        ];

        let (raw, wrapper_managed) =
            cpu_test_partitions(&manifest, &managed).expect("valid partition");
        assert_eq!(raw, ["fe2o3-raw"]);
        assert_eq!(wrapper_managed, ["fe2o3-managed"]);

        let (raw, wrapper_managed) =
            cpu_test_partitions(&manifest, &["fe2o3-non-example-managed".to_owned()])
                .expect("valid empty wrapper intersection");
        assert_eq!(raw, ["fe2o3-managed", "fe2o3-raw"]);
        assert!(wrapper_managed.is_empty());

        let duplicate = vec!["fe2o3-managed".to_owned(), "fe2o3-managed".to_owned()];
        assert!(
            cpu_test_partitions(&manifest, &duplicate)
                .expect_err("duplicate structural projection must fail closed")
                .contains("not unique")
        );
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
    fn sibling_namespace_attributes_do_not_create_a_package_binding_conflict() {
        let cleanup = test_directory();
        write(
            &cleanup.0,
            "Cargo.toml",
            "[package]\nname = \"sibling-namespace\"\nversion = \"0.1.0\"\n",
        );
        write(
            &cleanup.0,
            "src/lib.rs",
            "#[cfg_attr(any(), kernel(typed), other(namespace = \"00\"))]\npub fn managed() {}\n",
        );
        assert!(
            scan_test_package(&cleanup.0).expect("classify sibling namespace package"),
            "the typed attribute still requires wrapper-derived binding"
        );
    }

    #[test]
    fn structurally_detects_external_module_edges() {
        for source in [
            "#[path = \"../outside.rs\"] mod outside;",
            "#[cfg_attr(feature = \"gpu\", path = \"generated.rs\")] mod generated;",
            "include!(\"generated.rs\");",
        ] {
            assert!(source_has_external_module_edge(source).expect("valid edge source"));
        }
        assert!(source_has_external_module_edge("mod ordinary;").expect("module source"));
        assert!(
            !source_has_external_module_edge("mod ordinary { pub fn value() {} }")
                .expect("inline module source")
        );
    }

    #[test]
    fn complete_package_scan_finds_feature_gated_and_shared_test_sources() {
        let cleanup = test_directory();
        let root = &cleanup.0;
        write(
            root,
            "Cargo.toml",
            "[package]\nname = \"fixture\"\nversion = \"0.1.0\"\n",
        );
        write(root, "src/lib.rs", "pub fn ordinary() {}\n");
        write(
            root,
            "tests/common/mod.rs",
            "#[cfg(feature = \"gpu\")] #[renamed(typed)] pub fn managed() {}\n",
        );

        assert!(scan_test_package(root).expect("scan complete package source tree"));
    }

    #[test]
    fn external_path_and_include_edges_conservatively_select_the_package() {
        for (case, source) in [
            ("path", "#[path = \"../outside.rs\"] mod outside;\n"),
            (
                "include",
                "include!(concat!(env!(\"OUT_DIR\"), \"/generated.rs\"));\n",
            ),
        ] {
            let cleanup = test_directory();
            let root = &cleanup.0;
            write(
                root,
                "Cargo.toml",
                "[package]\nname = \"fixture\"\nversion = \"0.1.0\"\n",
            );
            write(root, "src/lib.rs", source);
            assert!(
                scan_test_package(root).expect("scan edge package"),
                "missed {case} edge"
            );
        }
    }

    #[test]
    fn macro_token_bodies_cannot_hide_generated_binding_or_include_edges() {
        for source in [
            "macro_rules! generated { () => { #[renamed(typed)] pub fn hidden() {} }; }",
            "macro_rules! generated { () => { include!(concat!(env!(\"OUT_DIR\"), \"/hidden.rs\")); }; }",
            "macro_rules! generated { () => { #[path = concat!(env!(\"OUT_DIR\"), \"/hidden.rs\")] mod hidden; }; }",
        ] {
            let cleanup = test_directory();
            let root = &cleanup.0;
            write(
                root,
                "Cargo.toml",
                "[package]\nname = \"macro-edge\"\nversion = \"0.1.0\"\n",
            );
            write(root, "src/lib.rs", source);
            assert!(
                scan_test_package(root).expect("scan macro token body"),
                "macro token body escaped managed selection: {source}"
            );
        }
    }

    #[test]
    fn unrelated_macro_tokens_do_not_conflict_with_an_explicit_fallback_namespace() {
        let namespace = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        let cleanup = test_directory();
        write(
            &cleanup.0,
            "Cargo.toml",
            "[package]\nname = \"fallback\"\nversion = \"0.1.0\"\n",
        );
        write(
            &cleanup.0,
            "src/lib.rs",
            &format!(
                "#[kernel(typed, namespace = \"{namespace}\")] pub fn fallback() {{}}\n\
                 macro_rules! log {{ () => {{ debug!(typed); tool!(path = \"config\"); }}; }}\n"
            ),
        );
        assert!(
            !scan_test_package(&cleanup.0).expect("unrelated macro tokens remain ordinary"),
            "unrelated macro arguments must not select wrapper-derived binding"
        );
    }

    #[test]
    fn resolved_internal_edges_preserve_explicit_fallback_namespace_packages() {
        let namespace = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        let cleanup = test_directory();
        let root = &cleanup.0;
        write(
            root,
            "Cargo.toml",
            "[package]\nname = \"fallback\"\nversion = \"0.1.0\"\n",
        );
        write(
            root,
            "src/lib.rs",
            &format!(
                "#[kernel(typed, namespace = \"{namespace}\")] pub fn fallback() {{}}\ninclude!(\"helper.rs\");\n"
            ),
        );
        write(root, "src/helper.rs", "pub fn helper() {}\n");
        assert!(
            !scan_test_package(root).expect("resolved internal include preserves fallback"),
            "an internal literal edge must not force compiler-derived binding"
        );

        write(
            root,
            "src/lib.rs",
            &format!(
                "#[kernel(typed, namespace = \"{namespace}\")] pub fn fallback() {{}}\ninclude!(concat!(env!(\"OUT_DIR\"), \"/generated.rs\"));\n"
            ),
        );
        assert!(
            !scan_test_package(root)
                .expect("observed fallback owns package despite unresolved external edge"),
            "observed fallback must prevent conservative wrapper selection"
        );
    }

    #[test]
    fn observed_fallback_wins_over_unrelated_external_edges_but_not_managed_sources() {
        let namespace = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        let cleanup = test_directory();
        let root = &cleanup.0;
        write(
            root,
            "Cargo.toml",
            "[package]\nname = \"fallback-package\"\nversion = \"0.1.0\"\n",
        );
        write(
            root,
            "tests/fallback.rs",
            &format!("#[kernel(typed, namespace = \"{namespace}\")] pub fn fallback() {{}}\n"),
        );
        write(
            root,
            "src/lib.rs",
            "#[path = \"../../external-support.rs\"] mod support;\n",
        );
        assert!(
            !scan_test_package(root).expect("fallback plus external support stays unmanaged"),
            "external content is uninspected and must not override an observed fallback"
        );

        write(
            root,
            "tests/managed.rs",
            "#[kernel(typed)] pub fn managed() {}\n",
        );
        let error =
            scan_test_package(root).expect_err("direct managed and fallback sources must conflict");
        assert!(
            error.contains("mixes an explicit fallback namespace"),
            "{error}"
        );
    }

    #[test]
    fn edge_into_nested_cargo_root_is_an_uninspected_selection_boundary() {
        let cleanup = test_directory();
        let root = &cleanup.0;
        write(
            root,
            "Cargo.toml",
            "[package]\nname = \"parent\"\nversion = \"0.1.0\"\n",
        );
        write(root, "src/lib.rs", "mod nested;\npub fn ordinary() {}\n");
        write(
            root,
            "src/nested/Cargo.toml",
            "[package]\nname = \"nested\"\nversion = \"0.1.0\"\n",
        );
        write(
            root,
            "src/nested/mod.rs",
            "#[renamed(typed)] pub fn reached_nested_source() {}\n",
        );
        std::os::unix::fs::symlink("missing", root.join("src/nested/ignored-link"))
            .expect("nested source symlink");

        assert!(scan_test_package(root).expect("skip nested package deterministically"));
    }

    #[test]
    fn unrelated_nested_cargo_root_does_not_select_the_parent_package() {
        let cleanup = test_directory();
        write(
            &cleanup.0,
            "Cargo.toml",
            "[package]\nname = \"parent\"\nversion = \"0.1.0\"\n",
        );
        write(&cleanup.0, "src/lib.rs", "pub fn ordinary() {}\n");
        write(
            &cleanup.0,
            "vendor/nested/Cargo.toml",
            "[package]\nname = \"nested\"\nversion = \"0.1.0\"\n",
        );
        write(
            &cleanup.0,
            "vendor/nested/src/lib.rs",
            "#[kernel(typed)] pub fn unrelated() {}\n",
        );
        assert!(
            !scan_test_package(&cleanup.0).expect("skip unrelated nested package"),
            "an unrelated nested Cargo package must not select its parent"
        );
    }

    #[test]
    fn workspace_shared_modules_are_uninspected_external_selection_boundaries() {
        let namespace = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        for (case, shared) in [
            (
                "typed",
                "#[renamed(typed)] pub fn shared_kernel() {}\n".to_owned(),
            ),
            (
                "fallback",
                format!(
                    "#[kernel(typed, namespace = \"{namespace}\")] pub fn shared_kernel() {{}}\n"
                ),
            ),
        ] {
            let cleanup = test_directory();
            let workspace = &cleanup.0;
            let package = workspace.join("package");
            write(
                &package,
                "Cargo.toml",
                "[package]\nname = \"package\"\nversion = \"0.1.0\"\n",
            );
            write(
                &package,
                "src/lib.rs",
                "#[path = \"../../shared.rs\"] mod shared;\n",
            );
            write(workspace, "shared.rs", &shared);
            let result = scan_test_workspace(workspace, &package);
            assert!(
                result.expect("scan shared workspace edge"),
                "external edge did not select package for {case}"
            );
        }
    }

    #[test]
    fn package_scan_rejects_intermediate_symlinks_and_entry_replacement() {
        let cleanup = test_directory();
        let root = &cleanup.0;
        write(
            root,
            "Cargo.toml",
            "[package]\nname = \"fixture\"\nversion = \"0.1.0\"\n",
        );
        let outside = root.join("outside");
        std::fs::create_dir(&outside).expect("outside source directory");
        write(&outside, "lib.rs", "pub fn ordinary() {}\n");
        std::os::unix::fs::symlink(&outside, root.join("src")).expect("source directory symlink");
        let error = scan_test_package(root).expect_err("intermediate symlink must fail closed");
        assert!(
            error.contains("symlinked") || error.contains("unreadable"),
            "{error}"
        );

        std::fs::remove_file(root.join("src")).expect("remove source symlink");
        write(root, "src/lib.rs", "pub fn ordinary() {}\n");
        let pinned = PinnedDirectory::open_existing(
            root.canonicalize().expect("canonical replacement root"),
            "replacement package",
        )
        .expect("pin replacement package");
        let mut replaced = false;
        let error = package_source_tree_requires_binding_with_hook(
            &pinned,
            &pinned,
            "replacement-package",
            &mut |relative| {
                if !replaced && relative == Path::new("src/lib.rs") {
                    replaced = true;
                    std::fs::rename(root.join(relative), root.join("held.rs"))
                        .expect("replace source");
                    std::os::unix::fs::symlink(root.join("held.rs"), root.join(relative))
                        .expect("install replacement symlink");
                }
            },
        )
        .expect_err("check/read substitution must fail closed");
        assert!(
            error.contains("symlinked") || error.contains("changed"),
            "{error}"
        );
    }

    #[test]
    fn package_scan_enforces_file_depth_and_aggregate_bounds() {
        let cleanup = test_directory();
        let root = &cleanup.0;
        write(
            root,
            "Cargo.toml",
            "[package]\nname = \"fixture\"\nversion = \"0.1.0\"\n",
        );
        std::fs::create_dir(root.join("src")).expect("source root");
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(root.join("src/oversized.rs"))
            .expect("oversized source")
            .set_len(MAX_PACKAGE_SOURCE_FILE_BYTES + 1)
            .expect("extend oversized source");
        let error = scan_test_package(root).expect_err("oversized source must fail");
        assert!(error.contains("exceeds"), "{error}");

        std::fs::remove_file(root.join("src/oversized.rs")).expect("remove oversized source");
        let mut directory = root.join("src");
        for index in 0..=MAX_PACKAGE_SOURCE_DEPTH {
            directory.push(format!("d{index}"));
            std::fs::create_dir(&directory).expect("create bounded depth fixture");
        }
        let error = scan_test_package(root).expect_err("deep tree must fail");
        assert!(error.contains("depth"), "{error}");

        let mut state = PackageSourceScanState::default();
        state.bytes = super::MAX_PACKAGE_SOURCE_TREE_BYTES;
        assert!(state.admit_source(1).is_err());
        state = PackageSourceScanState::default();
        state.files = super::MAX_PACKAGE_SOURCE_FILES;
        assert!(state.admit_source(0).is_err());

        for usage in [
            super::PackageSourceScanUsage {
                entries: super::MAX_WORKSPACE_SOURCE_ENTRIES + 1,
                files: 0,
                bytes: 0,
                name_bytes: 0,
            },
            super::PackageSourceScanUsage {
                entries: 0,
                files: super::MAX_WORKSPACE_SOURCE_FILES + 1,
                bytes: 0,
                name_bytes: 0,
            },
            super::PackageSourceScanUsage {
                entries: 0,
                files: 0,
                bytes: super::MAX_WORKSPACE_SOURCE_BYTES + 1,
                name_bytes: 0,
            },
            super::PackageSourceScanUsage {
                entries: 0,
                files: 0,
                bytes: 0,
                name_bytes: super::MAX_WORKSPACE_SOURCE_NAME_BYTES + 1,
            },
        ] {
            assert!(
                super::WorkspaceSourceScanBudget::default()
                    .admit(usage)
                    .is_err()
            );
        }
        assert!(super::admit_workspace_target_count(0, super::MAX_WORKSPACE_TARGETS).is_ok());
        assert!(super::admit_workspace_target_count(super::MAX_WORKSPACE_TARGETS, 1).is_err());

        let too_many_edges = (0..=MAX_PACKAGE_SOURCE_MODULE_EDGES)
            .map(|index| format!("mod module_{index};"))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            super::source_module_edges(&too_many_edges)
                .expect_err("module-edge bound must reject")
                .to_string()
                .contains("module-edge")
        );

        let mut nested = "#[kernel(".to_owned();
        for _ in 0..=MAX_PACKAGE_SOURCE_TOKEN_DEPTH {
            nested.push('(');
        }
        nested.push_str("typed");
        for _ in 0..=MAX_PACKAGE_SOURCE_TOKEN_DEPTH {
            nested.push(')');
        }
        nested.push_str(")] pub fn deep() {}");
        assert!(
            source_requires_wrapper_binding(&nested)
                .expect_err("token-depth bound must reject")
                .to_string()
                .contains("bounded source-token")
        );
    }

    #[test]
    fn metadata_paths_must_remain_in_their_canonical_workspace_and_package() {
        let cleanup = test_directory();
        let workspace_root = cleanup.0.canonicalize().expect("canonical workspace");
        let package_root = workspace_root.join("package");
        std::fs::create_dir(&package_root).expect("package root");
        write(
            &package_root,
            "Cargo.toml",
            "[package]\nname = \"fixture\"\nversion = \"0.1.0\"\n",
        );
        write(&package_root, "src/lib.rs", "pub fn ordinary() {}\n");
        write(&workspace_root, "outside.rs", "pub fn outside() {}\n");
        let workspace = PinnedDirectory::open_existing(workspace_root.clone(), "test workspace")
            .expect("pin workspace");
        let package_directory =
            PinnedDirectory::open_existing(package_root.clone(), "test package")
                .expect("pin package");
        let package = json!({
            "targets": [{ "src_path": workspace_root.join("outside.rs").to_str().unwrap() }]
        });
        let error = validate_package_targets(
            &package,
            &package_directory,
            &workspace_root,
            &package_root,
            "fixture",
            None,
        )
        .expect_err("cross-package target must fail");
        assert!(error.contains("escapes package root"), "{error}");

        write(
            &package_root,
            "nested/Cargo.toml",
            "[package]\nname = \"nested\"\nversion = \"0.1.0\"\n",
        );
        write(
            &package_root,
            "nested/src/owned.rs",
            "pub fn nested_owned() {}\n",
        );
        let package = json!({
            "targets": [{
                "src_path": package_root.join("nested/src/owned.rs").to_str().unwrap()
            }]
        });
        let error = validate_package_targets(
            &package,
            &package_directory,
            &workspace_root,
            &package_root,
            "fixture",
            None,
        )
        .expect_err("target into nested Cargo package must fail");
        assert!(
            error.contains("enters nested Cargo package root"),
            "{error}"
        );

        let non_rs = package_root.join("src/kernel.source");
        write(
            &package_root,
            "src/kernel.source",
            "#[kernel(typed)] pub fn nonstandard_target() {}\n",
        );
        let package = json!({
            "targets": [{ "src_path": non_rs.to_str().unwrap() }]
        });
        let error = validate_package_targets(
            &package,
            &package_directory,
            &workspace_root,
            &package_root,
            "fixture",
            None,
        )
        .expect_err("non-rs target must fail the rustc invocation contract");
        assert!(error.contains("UTF-8 .rs path"), "{error}");

        write(
            &package_root,
            "target/generated.rs",
            "#[kernel(typed)] pub fn generated() {}\n",
        );
        let cargo_target = package_root.join("target");
        let package = json!({
            "targets": [{ "src_path": cargo_target.join("generated.rs").to_str().unwrap() }]
        });
        let error = validate_package_targets(
            &package,
            &package_directory,
            &workspace_root,
            &package_root,
            "fixture",
            Some(&cargo_target),
        )
        .expect_err("declared target beneath generated target root must fail");
        assert!(
            error.contains("beneath generated Cargo target directory"),
            "{error}"
        );

        write(
            &package_root,
            "src/lib.rs",
            "#[kernel(typed, namespace = \"0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef\")] pub fn fallback() {}\n",
        );
        write(
            &package_root,
            "src/kernel.rs",
            "#[kernel(typed)] pub fn managed() {}\n",
        );
        let package = json!({
            "targets": [{ "src_path": package_root.join("src/kernel.rs").to_str().unwrap() }]
        });
        let mut targets = validate_package_targets(
            &package,
            &package_directory,
            &workspace_root,
            &package_root,
            "fixture",
            None,
        )
        .expect("retain mixed target");
        let error = package_source_tree_requires_binding_with_targets(
            &workspace,
            &package_directory,
            "fixture",
            &mut targets,
            &mut |_| {},
        )
        .expect_err("package-global mixed binding ownership must fail");
        assert!(
            error.contains("mixes an explicit fallback namespace"),
            "{error}"
        );

        let outside = test_directory();
        write(
            &outside.0,
            "Cargo.toml",
            "[package]\nname = \"outside\"\nversion = \"0.1.0\"\n",
        );
        let error = canonical_contained_path(
            &outside.0.join("Cargo.toml"),
            &workspace_root,
            "outside manifest",
        )
        .expect_err("outside manifest must fail");
        assert!(error.contains("escapes canonical containment"), "{error}");
    }

    #[test]
    fn retained_manifest_path_rejects_name_substitution() {
        let cleanup = test_directory();
        write(
            &cleanup.0,
            "Cargo.toml",
            "[package]\nname = \"fixture\"\nversion = \"0.1.0\"\n",
        );
        let root = cleanup.0.canonicalize().expect("canonical fixture root");
        let directory =
            PinnedDirectory::open_existing(root.clone(), "manifest replacement fixture")
                .expect("pin fixture");
        let descriptor = std::fs::File::open(root.join("Cargo.toml")).expect("open manifest");
        let initial = SourceObjectSnapshot::from_stat(
            &rustix::fs::fstat(&descriptor).expect("inspect manifest"),
        );
        std::fs::rename(root.join("Cargo.toml"), root.join("Cargo.held"))
            .expect("retain old manifest name");
        write(
            &root,
            "Cargo.toml",
            "[package]\nname = \"replacement\"\nversion = \"0.1.0\"\n",
        );
        let error = revalidate_retained_child(
            &directory,
            Path::new("Cargo.toml"),
            initial,
            "fixture",
            "Cargo manifest",
        )
        .expect_err("manifest name substitution must fail");
        assert!(error.contains("substituted"), "{error}");
    }

    #[test]
    fn retained_target_recheck_rejects_intermediate_directory_substitution() {
        let cleanup = test_directory();
        let root = cleanup.0.canonicalize().expect("canonical target fixture");
        write(
            &root,
            "Cargo.toml",
            "[package]\nname = \"fixture\"\nversion = \"0.1.0\"\n",
        );
        write(&root, "src/lib.rs", "pub fn ordinary() {}\n");
        write(
            &root,
            "src/kernel.rs",
            "#[kernel(typed)] pub fn target() {}\n",
        );
        let directory = PinnedDirectory::open_existing(root.clone(), "target replacement fixture")
            .expect("pin fixture");
        let package = json!({
            "targets": [{ "src_path": root.join("src/kernel.rs").to_str().unwrap() }]
        });
        let targets = validate_package_targets(&package, &directory, &root, &root, "fixture", None)
            .expect("retain target");
        std::fs::rename(root.join("src"), root.join("held-src"))
            .expect("retain old source directory");
        std::os::unix::fs::symlink(root.join("held-src"), root.join("src"))
            .expect("replace source directory with symlink");
        let error = revalidate_retained_child(
            &directory,
            Path::new("src/kernel.rs"),
            targets[0].initial,
            "fixture",
            "Cargo target source",
        )
        .expect_err("intermediate target-directory substitution must fail");
        assert!(
            error.contains("failed to reopen Cargo target source")
                || error.contains("symlink")
                || error.contains("unsupported file type")
                || error.contains("Not a directory"),
            "{error}"
        );
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
    fn cargo_metadata_capture_drains_and_rejects_each_output_overflow() {
        for script in ["printf '%065d' 0", "printf '%065d' 0 >&2"] {
            let mut command = Command::new("/bin/sh");
            command.args(["-c", script]);
            let error = super::bounded_command_output(&mut command, 64, 64)
                .expect_err("oversized metadata stream must fail");
            assert!(error.contains("exceeds its 64-byte bound"), "{error}");
        }
        let mut command = Command::new("/bin/sh");
        command.args(["-c", "printf ok; printf diagnostic >&2"]);
        let output =
            super::bounded_command_output(&mut command, 64, 64).expect("bounded metadata streams");
        assert!(output.status.success());
        assert_eq!(output.stdout, b"ok");
        assert_eq!(output.stderr, b"diagnostic");

        for target in ["relative-target", "/tmp/../tmp/noncanonical-target"] {
            let error = super::validated_metadata_target_directory(&json!({
                "target_directory": target
            }))
            .expect_err("malformed target_directory must fail");
            assert!(
                error.contains("not absolute") || error.contains("not canonically spelled"),
                "{error}"
            );
        }
    }

    #[test]
    fn rejects_malformed_manifest_structure_and_fields() {
        let cases = [
            (
                "wrong-version\npackage|rustc_check|artifact_qualification|source_artifacts\na|true|none|-\n".to_string(),
                "first line",
            ),
            (
                format!("{MANIFEST_VERSION}\npackage|unknown\na|true|none|-\n"),
                "second line",
            ),
            (example_manifest("a|true|none|-"), "end with a newline"),
            (example_manifest("\n"), "blank lines"),
            (
                example_manifest("a|true|none|-|extra\n"),
                "exactly four",
            ),
            (
                example_manifest("a|yes|none|-\n"),
                "rustc_check must be exactly",
            ),
            (
                example_manifest("b|true|none|-\na|true|none|-\n"),
                "sorted lexicographically",
            ),
            (
                example_manifest("a|true|none|-\na|true|none|-\n"),
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
            ("-package|true|none|-\n", "unsafe package name"),
            (
                "a|true|kernel-ir-v1|../escape.hsaco\n",
                "unsafe artifact name",
            ),
            (
                "a|true|kernel-ir-v1|alpha.o\n",
                "expected a .hsaco basename",
            ),
            (
                "a|true|kernel-ir-v1|same.hsaco\nb|true|kernel-ir-v1|same.hsaco\n",
                "duplicate artifact",
            ),
            (
                "a|true|kernel-ir-v1|zeta.hsaco,alpha.hsaco\n",
                "artifacts must be sorted lexicographically",
            ),
            (
                "a|true|unknown|alpha.hsaco\n",
                "artifact_qualification must be exactly",
            ),
            (
                "a|true|kernel-ir-v1|-\n",
                "artifact qualification requires one or more source artifacts",
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
            "fe2o3-alpha|true|kernel-ir-v1|alpha.hsaco\n\
             verus-vecadd|true|none|-\n",
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
            "fe2o3-alpha|true|kernel-ir-v1|alpha.hsaco\n",
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
        let pipeline = manifest
            .entries
            .iter()
            .find(|entry| entry.package == "fe2o3-pipeline")
            .expect("pipeline entry");
        let scalar_gemm = manifest
            .entries
            .iter()
            .find(|entry| entry.package == "fe2o3-scalar-gemm-v1")
            .expect("scalar GEMM entry");
        let fill = manifest
            .entries
            .iter()
            .find(|entry| entry.package == "fe2o3-fill")
            .expect("fill entry");
        let verus = manifest
            .entries
            .iter()
            .find(|entry| entry.package == "verus-vecadd")
            .expect("verus entry");

        assert_eq!(manifest.entries.len(), 26);
        assert_eq!(
            manifest
                .entries
                .iter()
                .filter(|entry| entry.artifact_qualification.produces_artifacts())
                .map(|entry| entry.package.as_str())
                .collect::<Vec<_>>(),
            ["fe2o3-fill"]
        );
        assert_eq!(
            pipeline.artifacts,
            ["bias_stage.hsaco", "scale_stage.hsaco"]
        );
        assert_eq!(pipeline.artifact_qualification, ArtifactQualification::None);
        assert_eq!(
            fill.artifact_qualification,
            ArtifactQualification::KernelIrV1
        );
        assert_eq!(fill.artifacts, ["fill.hsaco"]);
        assert!(verus.rustc_check);
        assert_eq!(verus.artifact_qualification, ArtifactQualification::None);
        assert!(verus.artifacts.is_empty());
        assert!(scalar_gemm.rustc_check);
        assert_eq!(
            scalar_gemm.artifact_qualification,
            ArtifactQualification::None
        );
        assert!(scalar_gemm.artifacts.is_empty());
    }
}
