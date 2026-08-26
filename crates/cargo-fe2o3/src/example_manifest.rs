use reserved_fe2o3_symbols::CrateBindingIdV1;
use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use syn::parse::Parser;
use syn::visit::{self, Visit};
use syn::{Attribute, Expr, ExprMethodCall, ItemFn, Lit, Meta, Token, punctuated::Punctuated};

const MANIFEST_PATH: &str = "examples/regression-manifest-v1.txt";
const MANIFEST_VERSION: &str = "fe2o3-example-regressions-v1";
const MANIFEST_COLUMNS: &str = "package|rustc_check|rocm_compile|gpu_smoke|artifacts";

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
    let workspace_root = crate::find_workspace_root()?;
    let manifest = load(&workspace_root)?;

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
        [command, package] if command == "check-artifacts" => {
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

fn load(workspace_root: &Path) -> Result<Manifest, String> {
    let path = workspace_root.join(MANIFEST_PATH);
    let contents = fs::read_to_string(&path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    let manifest =
        parse(&contents).map_err(|error| format!("invalid {}: {error}", path.display()))?;
    let workspace_examples = workspace_projection(workspace_root)?;
    validate_projection(&manifest, &workspace_examples)
        .map_err(|error| format!("invalid {}: {error}", path.display()))?;
    Ok(manifest)
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
        for artifact in source_artifact_literals(&source)
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
struct SourceArtifactVisitor {
    artifacts: Vec<String>,
}

impl<'ast> Visit<'ast> for SourceArtifactVisitor {
    fn visit_item_fn(&mut self, node: &'ast ItemFn) {
        if node.attrs.iter().any(is_kernel_attribute) {
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

fn is_kernel_attribute(attribute: &Attribute) -> bool {
    is_kernel_meta(&attribute.meta)
}

fn is_kernel_meta(meta: &Meta) -> bool {
    if let Meta::Path(path) = meta {
        return path
            .segments
            .last()
            .is_some_and(|segment| segment.ident == "kernel" && segment.arguments.is_empty());
    }
    let Meta::List(list) = meta else { return false };
    let Some(segment) = list.path.segments.last() else {
        return false;
    };
    if segment.ident == "cfg_attr" && segment.arguments.is_empty() {
        let Ok(arguments) =
            Punctuated::<Meta, Token![,]>::parse_terminated.parse2(list.tokens.clone())
        else {
            return false;
        };
        return arguments
            .into_iter()
            .skip(1)
            .any(|meta| is_kernel_meta(&meta));
    }
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
    typed || namespace || list.tokens.is_empty()
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
        Lane, MANIFEST_COLUMNS, MANIFEST_VERSION, WorkspaceExample, load, parse,
        source_artifact_literals, validate_projection,
    };
    use std::path::Path;

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

#[cfg_attr(
    not(feature = "qualification"),
    kernel(
        typed,
        namespace = "7c0e8b256bc76d2d17529f43ca8e2ee3480c40dfd019491bd4fb1fc22c4f5f2d"
    )
)]
pub fn configured() {}

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
                "configured.hsaco",
                "namespaced.hsaco",
                "nested.hsaco",
                "ordinary.hsaco",
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
        let verus = manifest
            .entries
            .iter()
            .find(|entry| entry.package == "verus-vecadd")
            .expect("verus entry");

        assert_eq!(manifest.entries.len(), 26);
        assert_eq!(
            pipeline.artifacts,
            ["bias_stage.hsaco", "scale_stage.hsaco"]
        );
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
