use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_OUTPUT_DIRECTORY: AtomicU64 = AtomicU64::new(0);

struct TestOutputDir {
    path: PathBuf,
}

impl TestOutputDir {
    fn new(workspace: &Path) -> Self {
        let path = workspace.join(format!(
            "target/rustc-codegen-fe2o3-test-output/g2-monomorphization-{}-{}",
            std::process::id(),
            NEXT_OUTPUT_DIRECTORY.fetch_add(1, Ordering::Relaxed)
        ));
        if path.exists() {
            std::fs::remove_dir_all(&path).expect("remove stale test output");
        }
        std::fs::create_dir_all(&path).expect("create test output");
        Self { path }
    }
}

impl Drop for TestOutputDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("canonical workspace")
}

fn fixtures(workspace: &Path) -> PathBuf {
    workspace.join("crates/rustc-codegen-fe2o3/tests/fixtures/g2-monomorphization")
}

fn rustc() -> Command {
    Command::new(std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()))
}

fn require_success(label: &str, output: &Output) {
    assert!(
        output.status.success(),
        "{label} failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}

fn compile_rlib(source: &Path, crate_name: &str, output: &Path) {
    let result = rustc()
        .arg(source)
        .args(["--edition=2024", "--crate-type=rlib", "--crate-name"])
        .arg(crate_name)
        .arg("-o")
        .arg(output)
        .output()
        .expect("compile fixture rlib");
    require_success(crate_name, &result);
}

fn compile_frontend(source: &Path, output: &Path, externs: &[(&str, &Path)]) -> Output {
    let mut command = rustc();
    command
        .arg(source)
        .args(["--edition=2024", "--emit=metadata"])
        .arg("-o")
        .arg(output);
    for &(name, path) in externs {
        command
            .arg("--extern")
            .arg(format!("{name}={}", path.display()));
    }
    command.output().expect("compile frontend fixture")
}

fn build_backend(workspace: &Path) -> PathBuf {
    let output = Command::new(env!("CARGO"))
        .current_dir(workspace)
        .args(["build", "--locked", "-p", "rustc-codegen-fe2o3"])
        .output()
        .expect("build backend");
    require_success("backend build", &output);
    let backend = workspace.join("target/debug/librustc_codegen_fe2o3.so");
    assert!(
        backend.is_file(),
        "missing backend at {}",
        backend.display()
    );
    backend
}

fn compile_with_backend(
    source: &Path,
    crate_name: &str,
    backend: &Path,
    output_dir: &Path,
    externs: &[(&str, &Path)],
    extra_args: &[&str],
) -> Output {
    let mut command = rustc();
    command
        .arg(source)
        .args(["--edition=2024", "--crate-name", crate_name])
        .arg(format!("-Zcodegen-backend={}", backend.display()))
        .arg("-Zmir-enable-passes=-JumpThreading")
        .arg("-o")
        .arg(output_dir.join(crate_name))
        .env("FE2O3_VERBOSE", "1")
        .env("FE2O3_HSACO_DIR", output_dir.join("artifacts"))
        .env(
            "FE2O3_TARGET",
            std::env::var("FE2O3_TEST_TARGET").unwrap_or_else(|_| "gfx1100".to_owned()),
        );
    command.args(extra_args);
    for &(name, path) in externs {
        command
            .arg("--extern")
            .arg(format!("{name}={}", path.display()));
    }
    std::fs::create_dir_all(output_dir.join("artifacts")).expect("create artifact directory");
    command.output().expect("compile fixture with backend")
}

fn collection_rows(stderr: &str) -> Vec<String> {
    let mut rows = Vec::new();
    let mut in_collection = false;
    for line in stderr.lines() {
        if line == "=== fe2o3 device function collection ===" {
            in_collection = true;
        } else if line == "========================================" {
            break;
        } else if in_collection
            && (line.trim_start().starts_with("path:")
                || line.trim_start().starts_with("instance:"))
        {
            rows.push(line.trim().to_owned());
        }
    }
    rows
}

fn observation_field(stderr: &str, function: &str, field: &str) -> Option<String> {
    stderr
        .lines()
        .find(|line| line.starts_with("[collector] V1 identity ") && line.contains(function))
        .and_then(|line| {
            line.split_whitespace()
                .find_map(|part| part.strip_prefix(&format!("{field}=")))
        })
        .map(str::to_owned)
}

fn visiting_shape(stderr: &str, function: &str) -> Option<String> {
    stderr
        .lines()
        .find(|line| line.starts_with("[collector] visiting ") && line.contains(function))
        .and_then(|line| line.split_once(" (").map(|(_, shape)| shape.to_owned()))
}

fn observation_counts(stderr: &str, function: &str) -> Option<(usize, usize)> {
    let shape = visiting_shape(stderr, function)?;
    let mut fields = shape.trim_end_matches(')').split(", ");
    fields.next()?.strip_suffix(" basic blocks")?;
    let decisions = fields.next()?.strip_suffix(" V1 decisions")?.parse().ok()?;
    let excluded = fields
        .next()?
        .strip_suffix(" policy-excluded")?
        .parse()
        .ok()?;
    Some((decisions, excluded))
}

#[test]
fn fixture_corpus_clears_the_standard_frontend_without_manifests() {
    let workspace = workspace();
    let fixtures = fixtures(&workspace);
    let output = TestOutputDir::new(&workspace);
    let shared_a = output.path.join("libg2_shared_a.rlib");
    let shared_b = output.path.join("libg2_shared_b.rlib");
    let unavailable = output.path.join("libg2_unavailable_helper.rlib");
    let fmt_lookalike = output.path.join("libg2_fmt_lookalike.rlib");
    compile_rlib(&fixtures.join("shared-a.rs"), "g2_shared_a", &shared_a);
    compile_rlib(&fixtures.join("shared-b.rs"), "g2_shared_b", &shared_b);
    compile_rlib(
        &fixtures.join("unavailable-helper.rs"),
        "g2_unavailable_helper",
        &unavailable,
    );
    compile_rlib(
        &fixtures.join("fmt-lookalike-helper.rs"),
        "g2_fmt_lookalike",
        &fmt_lookalike,
    );

    let collectible = compile_frontend(
        &fixtures.join("collectible.rs"),
        &output.path.join("collectible.rmeta"),
        &[("g2_shared_a", &shared_a), ("g2_shared_b", &shared_b)],
    );
    require_success("collectible frontend", &collectible);
    let unavailable_root = compile_frontend(
        &fixtures.join("unavailable.rs"),
        &output.path.join("unavailable.rmeta"),
        &[("g2_unavailable_helper", &unavailable)],
    );
    require_success("unavailable-MIR frontend", &unavailable_root);
    let malformed = compile_frontend(
        &fixtures.join("malformed-registration.rs"),
        &output.path.join("malformed.rmeta"),
        &[],
    );
    require_success("malformed registration frontend", &malformed);
    let dead_branches = compile_frontend(
        &fixtures.join("dead-branches.rs"),
        &output.path.join("dead-branches.rmeta"),
        &[],
    );
    require_success("dead-branches frontend", &dead_branches);
    let fmt_root = compile_frontend(
        &fixtures.join("fmt-lookalike.rs"),
        &output.path.join("fmt-lookalike.rmeta"),
        &[("g2_fmt_lookalike", fmt_lookalike.as_path())],
    );
    require_success("fmt-lookalike frontend", &fmt_root);
    for fixture in ["semantic-substitution-a.rs", "semantic-substitution-b.rs"] {
        let semantic = compile_frontend(
            &fixtures.join(fixture),
            &output.path.join(format!("{fixture}.rmeta")),
            &[],
        );
        require_success(fixture, &semantic);
    }
}

#[test]
#[ignore = "requires the configured ROCm LLVM toolchain"]
fn collector_resolves_concrete_instances_and_rejects_unavailable_mir_stably() {
    let workspace = workspace();
    let fixtures = fixtures(&workspace);
    let output = TestOutputDir::new(&workspace);
    let backend = build_backend(&workspace);
    let shared_a = output.path.join("libg2_shared_a.rlib");
    let shared_b = output.path.join("libg2_shared_b.rlib");
    let unavailable = output.path.join("libg2_unavailable_helper.rlib");
    compile_rlib(&fixtures.join("shared-a.rs"), "g2_shared_a", &shared_a);
    compile_rlib(&fixtures.join("shared-b.rs"), "g2_shared_b", &shared_b);
    compile_rlib(
        &fixtures.join("unavailable-helper.rs"),
        "g2_unavailable_helper",
        &unavailable,
    );

    let externs = [
        ("g2_shared_a", shared_a.as_path()),
        ("g2_shared_b", shared_b.as_path()),
    ];
    let first = compile_with_backend(
        &fixtures.join("collectible.rs"),
        "g2_collectible",
        &backend,
        &output.path.join("first"),
        &externs,
        &[],
    );
    let first_stderr = String::from_utf8_lossy(&first.stderr);
    let rows = collection_rows(&first_stderr);
    assert!(
        !rows.is_empty(),
        "collector dump is missing:\n{first_stderr}"
    );
    assert_eq!(
        rows.iter()
            .filter(|row| row.ends_with("generic_identity"))
            .count(),
        2,
        "two concrete generic types should be collected once each:\n{first_stderr}"
    );
    assert_eq!(
        rows.iter()
            .filter(|row| row.ends_with("const_bias"))
            .count(),
        2,
        "two concrete const-generic instances should be collected:\n{first_stderr}"
    );
    assert_eq!(
        rows.iter()
            .filter(|row| row.ends_with("recursive_sum"))
            .count(),
        1,
        "the recursive cycle should terminate at one concrete instance:\n{first_stderr}"
    );
    assert!(
        rows.iter().any(|row| row == "path: g2_shared_a::same_name")
            && rows.iter().any(|row| row == "path: g2_shared_b::same_name"),
        "same-name helpers from two crates were not both collected:\n{first_stderr}"
    );
    let identities = rows
        .iter()
        .filter(|row| row.starts_with("instance:"))
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        identities.len() * 2,
        rows.len(),
        "every collected path must have a distinct concrete identity:\n{first_stderr}"
    );

    let second = compile_with_backend(
        &fixtures.join("collectible.rs"),
        "g2_collectible",
        &backend,
        &output.path.join("second"),
        &externs,
        &[],
    );
    let second_stderr = String::from_utf8_lossy(&second.stderr);
    assert_eq!(
        rows,
        collection_rows(&second_stderr),
        "collection order or identities changed"
    );

    let unavailable_result = compile_with_backend(
        &fixtures.join("unavailable.rs"),
        "g2_unavailable",
        &backend,
        &output.path.join("unavailable"),
        &[("g2_unavailable_helper", unavailable.as_path())],
        &[],
    );
    let unavailable_stderr = String::from_utf8_lossy(&unavailable_result.stderr);
    assert!(!unavailable_result.status.success());
    assert!(unavailable_stderr.contains("MIR is unavailable for a device-reachable item"));
    assert!(unavailable_stderr.contains("g2_unavailable::fe2o3_kernel_unavailable"));
    assert!(unavailable_stderr.contains("g2_unavailable::local_bridge"));
    assert!(unavailable_stderr.contains("g2_unavailable_helper::unavailable"));
    assert!(unavailable_stderr.contains("reachable call chain:"));

    let malformed = compile_with_backend(
        &fixtures.join("malformed-registration.rs"),
        "g2_malformed",
        &backend,
        &output.path.join("malformed"),
        &[],
        &[],
    );
    let malformed_stderr = String::from_utf8_lossy(&malformed.stderr);
    assert!(!malformed.status.success());
    assert!(malformed_stderr.contains("does not match registration magic"));
    assert!(!malformed_stderr.contains("[collector] root kernel:"));
}

#[test]
#[ignore = "requires a configured rustc backend"]
fn collector_rejects_non_direct_constants_aliases_and_lookalikes() {
    let workspace = workspace();
    let fixtures = fixtures(&workspace);
    let output = TestOutputDir::new(&workspace);
    let backend = build_backend(&workspace);
    let source = fixtures.join("dead-branches.rs");
    for (configuration, function, diagnostic) in [
        (
            "local_const_panic",
            "local_const_panic",
            "device code reaches a panic path",
        ),
        (
            "local_const_unsupported",
            "local_const_unsupported",
            "indirect function-pointer calls are not permitted",
        ),
    ] {
        let args = ["-Zmir-opt-level=0", "--cfg", configuration];
        let result = compile_with_backend(
            &source,
            &format!("g2_{configuration}"),
            &backend,
            &output.path.join(configuration),
            &[],
            &args,
        );
        let stderr = String::from_utf8_lossy(&result.stderr);
        assert!(
            !result.status.success(),
            "non-direct adversary `{configuration}` compiled successfully:\n{stderr}"
        );
        assert!(
            stderr.contains(diagnostic),
            "adversary `{configuration}` did not fail for `{diagnostic}`:\n{stderr}"
        );
        assert_eq!(
            observation_counts(&stderr, function),
            Some((0, 0)),
            "`{configuration}` received non-direct V1 authority:\n{stderr}"
        );
        let target = observation_field(&stderr, function, "target")
            .unwrap_or_else(|| panic!("missing target identity for `{function}`:\n{stderr}"));
        assert_eq!(target.len(), 64);
        assert_ne!(target, "0".repeat(64));
    }

    // The pinned 2bbdb7f scanner trusted a direct local initialization and
    // ignored a later write through `&mut`. The target case likewise became a
    // local assignment after const evaluation. These are the exact bad counts
    // observed at that revision with this fixture and toolchain.
    for (configuration, function, diagnostic, known_bad_counts) in [
        (
            "alias_panic",
            "alias_panic",
            "device code reaches a panic path",
            (1, 1),
        ),
        (
            "alias_unsupported",
            "alias_unsupported",
            "indirect function-pointer calls are not permitted",
            (1, 2),
        ),
        (
            "target_size",
            "target_size",
            "indirect function-pointer calls are not permitted",
            (1, 1),
        ),
    ] {
        let args = ["-Zmir-opt-level=0", "--cfg", configuration];
        let result = compile_with_backend(
            &source,
            &format!("g2_{configuration}"),
            &backend,
            &output.path.join(configuration),
            &[],
            &args,
        );
        let stderr = String::from_utf8_lossy(&result.stderr);
        assert!(
            !result.status.success(),
            "regression adversary `{configuration}` compiled successfully:\n{stderr}"
        );
        assert!(
            stderr.contains(diagnostic),
            "adversary `{configuration}` did not reach `{diagnostic}`:\n{stderr}"
        );
        let head_counts = observation_counts(&stderr, function)
            .unwrap_or_else(|| panic!("missing observation for `{function}`:\n{stderr}"));
        assert_ne!(
            head_counts, known_bad_counts,
            "`{configuration}` reproduced 2bbdb7f authority {known_bad_counts:?}:\n{stderr}"
        );
        assert_eq!(
            head_counts,
            (0, 0),
            "`{configuration}` received local-assignment V1 authority:\n{stderr}"
        );
        let target = observation_field(&stderr, function, "target")
            .unwrap_or_else(|| panic!("missing target identity for `{function}`:\n{stderr}"));
        assert_eq!(target.len(), 64);
        assert_ne!(target, "0".repeat(64));
    }

    let address_args = ["-Zmir-opt-level=0", "--cfg", "local_const_address"];
    let address = compile_with_backend(
        &source,
        "g2_local_const_address",
        &backend,
        &output.path.join("local_const_address"),
        &[],
        &address_args,
    );
    let address_stderr = String::from_utf8_lossy(&address.stderr);
    assert!(
        !address.status.success(),
        "local-backed address hazard compiled successfully:\n{address_stderr}"
    );
    assert!(
        address_stderr.contains("local_const_address")
            && address_stderr.contains("0 V1 decisions, 0 policy-excluded"),
        "local-backed address branch received V1 authority:\n{address_stderr}"
    );

    let fmt_library = output.path.join("libg2_fmt_lookalike.rlib");
    compile_rlib(
        &fixtures.join("fmt-lookalike-helper.rs"),
        "g2_fmt_lookalike",
        &fmt_library,
    );
    let fmt = compile_with_backend(
        &fixtures.join("fmt-lookalike.rs"),
        "g2_fmt_lookalike_root",
        &backend,
        &output.path.join("fmt-lookalike"),
        &[("g2_fmt_lookalike", fmt_library.as_path())],
        &["-Zmir-opt-level=0"],
    );
    let fmt_stderr = String::from_utf8_lossy(&fmt.stderr);
    assert!(!fmt.status.success());
    assert!(fmt_stderr.contains("g2_fmt_lookalike::fmt::hidden"));
    assert!(fmt_stderr.contains("indirect function-pointer calls are not permitted"));

    let first_source = fixtures.join("semantic-substitution-a.rs");
    let second_source = fixtures.join("semantic-substitution-b.rs");
    let first_text = std::fs::read_to_string(&first_source).unwrap();
    let second_text = std::fs::read_to_string(&second_source).unwrap();
    assert_eq!(first_text.len(), second_text.len());
    assert_eq!(first_text.lines().count(), second_text.lines().count());
    let first_remap = format!(
        "--remap-path-prefix={}=/row23/semantic-substitution.rs",
        first_source.display()
    );
    let second_remap = format!(
        "--remap-path-prefix={}=/row23/semantic-substitution.rs",
        second_source.display()
    );
    let first_args = ["-Zmir-opt-level=0", first_remap.as_str()];
    let second_args = ["-Zmir-opt-level=0", second_remap.as_str()];
    let first = compile_with_backend(
        &first_source,
        "g2_semantic_substitution",
        &backend,
        &output.path.join("semantic-a"),
        &[],
        &first_args,
    );
    let second = compile_with_backend(
        &second_source,
        "g2_semantic_substitution",
        &backend,
        &output.path.join("semantic-b"),
        &[],
        &second_args,
    );
    let first_stderr = String::from_utf8_lossy(&first.stderr);
    let second_stderr = String::from_utf8_lossy(&second.stderr);
    let first_shape = visiting_shape(&first_stderr, "semantic_helper")
        .unwrap_or_else(|| panic!("missing first helper shape:\n{first_stderr}"));
    let second_shape = visiting_shape(&second_stderr, "semantic_helper")
        .unwrap_or_else(|| panic!("missing second helper shape:\n{second_stderr}"));
    assert_eq!(first_shape, second_shape);
    let first_source_identity = observation_field(&first_stderr, "semantic_helper", "source")
        .unwrap_or_else(|| panic!("missing first source identity:\n{first_stderr}"));
    let second_source_identity = observation_field(&second_stderr, "semantic_helper", "source")
        .unwrap_or_else(|| panic!("missing second source identity:\n{second_stderr}"));
    assert_eq!(
        first_source_identity, second_source_identity,
        "same-span substitutions did not preserve the source-location identity"
    );
    let first_mir = observation_field(&first_stderr, "semantic_helper", "mir").unwrap();
    let second_mir = observation_field(&second_stderr, "semantic_helper", "mir").unwrap();
    assert_ne!(
        first_mir, second_mir,
        "semantic MIR substitution was not bound"
    );
    let first_target = observation_field(&first_stderr, "semantic_helper", "target")
        .unwrap_or_else(|| panic!("missing first target identity:\n{first_stderr}"));
    let second_target = observation_field(&second_stderr, "semantic_helper", "target")
        .unwrap_or_else(|| panic!("missing second target identity:\n{second_stderr}"));
    assert_eq!(first_target, second_target);
}
