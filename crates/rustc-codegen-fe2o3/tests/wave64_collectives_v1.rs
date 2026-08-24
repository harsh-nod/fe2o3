use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};

const PIPELINE: &str = "collected-wave64-collectives-v1";
const CRATE_NAME: &str = "fe2o3_collected_wave64_collectives_v1_fixture";
const REVIEWED_METADATA: &str = "fe2o3-wave64-collectives-v1-reviewed";
const COMPILER_CRATE_BINDING: &str =
    "ba3fa024069d9cee1b86cf6fc1ad80a77d9de5457de020b70182cdc265e64569";
const CARGO_METADATA_OBSERVATION: &str =
    "c1ab2dc02fa023687ac7394e15746c39668b5d46ad47c40eae012bc3f42d05c0";
const SOURCE_REMAP: &str = "/fe2o3-reviewed-workspace/wave64-collectives-v1.rs";
const WORKSPACE_REMAP: &str = "/fe2o3-reviewed-workspace";
const SOURCE: &str = include_str!("../../../examples/wave64_collectives_v1/src/kernel.rs");
const REPORT_AUTHORITY_ENV: &str = "FE2O3_WAVE64_REPORT_AUTHORITY";

static NEXT_OUTPUT: AtomicU64 = AtomicU64::new(0);
static FRONTEND_DEPENDENCIES: OnceLock<Result<(), String>> = OnceLock::new();

struct TestOutput {
    path: PathBuf,
}

impl TestOutput {
    fn new(workspace: &Path) -> Self {
        let path = cargo_target(workspace).join(format!(
            "wave64-collectives-v1-{}-{}",
            std::process::id(),
            NEXT_OUTPUT.fetch_add(1, Ordering::Relaxed)
        ));
        if path.exists() {
            std::fs::remove_dir_all(&path).expect("remove stale Wave64 test output");
        }
        std::fs::create_dir_all(&path).expect("create Wave64 test output");
        Self { path }
    }
}

impl Drop for TestOutput {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

struct CleanCargoTarget {
    path: PathBuf,
}

impl CleanCargoTarget {
    fn new(workspace: &Path, label: &str) -> Self {
        let path = cargo_target(workspace).join(format!(
            "wave64-collectives-v1-clean-{label}-{}",
            std::process::id()
        ));
        if path.exists() {
            std::fs::remove_dir_all(&path).expect("remove stale clean Wave64 target");
        }
        Self { path }
    }
}

impl Drop for CleanCargoTarget {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

#[derive(Clone, Copy)]
struct CompilerProfile<'a> {
    target: &'a str,
    crate_name: &'a str,
    metadata: &'a str,
    crate_binding: &'a str,
    overflow_checks: bool,
}

impl Default for CompilerProfile<'static> {
    fn default() -> Self {
        Self {
            target: "gfx942:xnack-",
            crate_name: CRATE_NAME,
            metadata: REVIEWED_METADATA,
            crate_binding: COMPILER_CRATE_BINDING,
            overflow_checks: false,
        }
    }
}

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("canonical workspace")
}

fn cargo_target(workspace: &Path) -> PathBuf {
    match std::env::var_os("CARGO_TARGET_DIR") {
        Some(path) if Path::new(&path).is_absolute() => PathBuf::from(path),
        Some(path) => workspace.join(path),
        None => workspace.join("target"),
    }
}

fn profile_name() -> &'static str {
    if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    }
}

fn frontend_target(workspace: &Path) -> PathBuf {
    cargo_target(workspace).join("wave64-collectives-v1-frontend-target")
}

fn build_frontend_dependencies(workspace: &Path) -> Result<(), String> {
    FRONTEND_DEPENDENCIES
        .get_or_init(|| {
            let mut command = Command::new(env!("CARGO"));
            command.current_dir(workspace).args([
                "build",
                "--locked",
                "-p",
                "fe2o3-device",
                "-p",
                "fe2o3-host",
            ]);
            command
                .arg("--target-dir")
                .arg(frontend_target(workspace))
                .env("CARGO_INCREMENTAL", "0");
            let output = command.output().map_err(|error| error.to_string())?;
            if output.status.success() {
                Ok(())
            } else {
                Err(format!(
                    "frontend dependency build failed:\n{}",
                    String::from_utf8_lossy(&output.stderr)
                ))
            }
        })
        .clone()
}

fn compile(
    workspace: &Path,
    output: &TestOutput,
    label: &str,
    source: &str,
    profile: CompilerProfile<'_>,
) -> Output {
    build_frontend_dependencies(workspace).expect("build Wave64 frontend dependencies");
    let backend_target = cargo_target(workspace).join(profile_name());
    let frontend_target = frontend_target(workspace).join("debug");
    let backend = backend_target.join("librustc_codegen_fe2o3.so");
    let device = frontend_target.join("libfe2o3_device.rlib");
    let host = frontend_target.join("libfe2o3_host.rlib");
    for required in [&backend, &device, &host] {
        assert!(required.is_file(), "missing {}", required.display());
    }

    let source_path = output.path.join(format!("{label}.rs"));
    std::fs::write(&source_path, source).expect("write exact or hostile Wave64 fixture");
    let artifact_dir = output.path.join(format!("{label}-artifacts"));
    std::fs::create_dir_all(&artifact_dir).expect("create empty Wave64 artifact directory");
    let fixture_manifest =
        workspace.join("crates/rustc-codegen-fe2o3/tests/fixtures/collected-wave64-collectives-v1");

    Command::new(std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()))
        .current_dir(workspace)
        .arg(&source_path)
        .arg(format!(
            "--remap-path-prefix={}={SOURCE_REMAP}",
            source_path.display()
        ))
        .arg(format!(
            "--remap-path-prefix={}={WORKSPACE_REMAP}",
            workspace.display()
        ))
        .args(["--edition=2024", "--crate-type=lib", "--crate-name"])
        .arg(profile.crate_name)
        .arg("--extern")
        .arg(format!("fe2o3_device={}", device.display()))
        .arg("--extern")
        .arg(format!("fe2o3_host={}", host.display()))
        .arg("-L")
        .arg(format!(
            "dependency={}",
            frontend_target.join("deps").display()
        ))
        .arg(format!("-Coverflow-checks={}", profile.overflow_checks))
        .arg(format!("-Cmetadata={}", profile.metadata))
        .arg("-Zmir-enable-passes=-JumpThreading")
        .arg(format!("-Zcodegen-backend={}", backend.display()))
        .arg("-o")
        .arg(output.path.join(format!("lib{label}.rlib")))
        .env("CARGO_MANIFEST_DIR", fixture_manifest)
        .env("FE2O3_CRATE_BINDING_ID_V1", profile.crate_binding)
        .env(
            "FE2O3_CARGO_METADATA_BUILD_OBSERVATION_V2",
            CARGO_METADATA_OBSERVATION,
        )
        .env("FE2O3_HSACO_DIR", &artifact_dir)
        .env("FE2O3_TARGET", profile.target)
        .env("FE2O3_CODEGEN_PIPELINE", PIPELINE)
        .output()
        .expect("run Wave64 compiler fixture")
}

fn mutation(source: &str, old: &str, new: &str) -> String {
    assert_eq!(source.matches(old).count(), 1, "non-unique mutation anchor");
    source.replacen(old, new, 1)
}

fn assert_rejected(result: &Output, label: &str) {
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(!result.status.success(), "hostile case `{label}` compiled");
    assert!(
        !stderr.contains("authenticated exact source bytes"),
        "hostile case `{label}` reached the authenticated receipt:\n{stderr}"
    );
}

fn run_clean_exact(
    workspace: &Path,
    target: &CleanCargoTarget,
    incremental: Option<&str>,
    rustflags: Option<&str>,
) -> Output {
    let mut command = Command::new(env!("CARGO"));
    command
        .current_dir(workspace)
        .args([
            "test",
            "--locked",
            "-p",
            "rustc-codegen-fe2o3",
            "--test",
            "wave64_collectives_v1",
            "exact_phase_a_source_authenticates_complete_wave64_profile",
            "--",
            "--exact",
            "--nocapture",
            "--test-threads=1",
        ])
        .env("CARGO_TARGET_DIR", &target.path)
        .env_remove("CARGO_ENCODED_RUSTFLAGS")
        .env("FE2O3_VERBOSE", "1")
        .env(REPORT_AUTHORITY_ENV, "1");
    match incremental {
        Some(value) => {
            command.env("CARGO_INCREMENTAL", value);
        }
        None => {
            command.env_remove("CARGO_INCREMENTAL");
        }
    }
    match rustflags {
        Some(value) => {
            command.env("RUSTFLAGS", value);
        }
        None => {
            command.env_remove("RUSTFLAGS");
        }
    }
    command.output().expect("run clean Wave64 exact fixture")
}

fn command_text(output: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

fn admitted_closure(text: &str) -> String {
    let marker = "complete reachable portable-MIR closure ";
    text.split_once(marker)
        .unwrap_or_else(|| panic!("missing admitted closure marker:\n{text}"))
        .1
        .chars()
        .take_while(char::is_ascii_hexdigit)
        .take(64)
        .collect()
}

fn internal_helper_exports(text: &str) -> Vec<String> {
    text.lines()
        .filter_map(|line| line.strip_prefix("  [internal-helper] "))
        .map(str::to_owned)
        .collect()
}

#[test]
fn clean_build_modes_admit_the_same_wave64_closure() {
    let workspace = workspace();
    let default_target = CleanCargoTarget::new(&workspace, "incremental-default");
    let disabled_target = CleanCargoTarget::new(&workspace, "incremental-disabled");
    let metadata_target = CleanCargoTarget::new(&workspace, "provider-metadata-varied");

    let default = run_clean_exact(&workspace, &default_target, None, None);
    let default_text = command_text(&default);
    assert!(
        default.status.success(),
        "clean default-incremental Wave64 run failed:\n{default_text}"
    );

    let disabled = run_clean_exact(&workspace, &disabled_target, Some("0"), None);
    let disabled_text = command_text(&disabled);
    assert!(
        disabled.status.success(),
        "clean CARGO_INCREMENTAL=0 Wave64 run failed:\n{disabled_text}"
    );

    assert_eq!(
        admitted_closure(&default_text),
        admitted_closure(&disabled_text),
        "nonsemantic Cargo incremental mode changed the admitted MIR closure"
    );

    let metadata = run_clean_exact(
        &workspace,
        &metadata_target,
        None,
        Some("-Cmetadata=fe2o3-provider-disambiguator-regression"),
    );
    let metadata_text = command_text(&metadata);
    assert!(
        metadata.status.success(),
        "clean provider-metadata-varied Wave64 run failed:\n{metadata_text}"
    );
    assert_eq!(
        admitted_closure(&default_text),
        admitted_closure(&metadata_text),
        "nonsemantic provider crate metadata changed the admitted MIR closure"
    );
    let default_helpers = internal_helper_exports(&default_text);
    let metadata_helpers = internal_helper_exports(&metadata_text);
    assert_eq!(
        default_helpers.len(),
        2,
        "unexpected default helper closure"
    );
    assert_eq!(
        metadata_helpers.len(),
        2,
        "unexpected varied helper closure"
    );
    assert_ne!(
        default_helpers, metadata_helpers,
        "provider metadata variation did not change the rustc export disambiguator"
    );
}

#[test]
fn exact_phase_a_source_authenticates_complete_wave64_profile() {
    let workspace = workspace();
    let output = TestOutput::new(&workspace);
    let result = compile(
        &workspace,
        &output,
        "exact",
        SOURCE,
        CompilerProfile::default(),
    );
    let stderr = String::from_utf8_lossy(&result.stderr);
    if std::env::var_os(REPORT_AUTHORITY_ENV).is_some() {
        eprint!("{stderr}");
    }
    assert!(
        !result.status.success(),
        "admission-only pipeline unexpectedly emitted code"
    );
    for marker in [
        "authenticated exact source bytes",
        "separate reviewed profile namespace, distinct compiler-derived ordinary #[kernel(typed)] root",
        "complete reachable portable-MIR closure 55043a3ac1aa25bd5e47588b61c0b5fedd0c9f4ebd1c59255d0cfdbbd306414c",
        "3 ordered collectives, 3 lane-owned outputs",
        "exact grid [1, 1, 1]",
        "reviewed source-to-profile correspondence only",
        "no compiler-refinement proof, generic-IR substitution, LLVM lowering, Worker V2",
    ] {
        assert!(stderr.contains(marker), "missing `{marker}`:\n{stderr}");
    }
}

#[test]
fn hostile_source_and_compiler_profile_mutations_fail_closed() {
    let workspace = workspace();
    let output = TestOutput::new(&workspace);
    let source_mutations = [
        ("source-bytes", format!("{SOURCE}\n// hostile byte\n")),
        (
            "explicit-namespace",
            mutation(
                SOURCE,
                "    typed,\n",
                "    typed,\n    namespace = \"2863304ebf7f501a7f177c5b8f5a456261ee34760472727ba3f0205ccf5ce9cc\",\n",
            ),
        ),
        (
            "mask-type",
            mutation(
                &mutation(SOURCE, "active_mask: u64", "active_mask: u32"),
                "1_u64 << lane",
                "1_u32 << lane",
            ),
        ),
        (
            "mask-use",
            mutation(
                SOURCE,
                "let active = active_mask & (1_u64 << lane) != 0;",
                "let active = true;",
            ),
        ),
        (
            "workgroup-width",
            SOURCE.replace("[64, 1, 1]", "[32, 1, 1]"),
        ),
        (
            "collective-kind",
            mutation(
                SOURCE,
                "reduce_sum(&context, contribution)",
                "inclusive_scan_sum(&context, contribution)",
            ),
        ),
        (
            "collective-order",
            mutation(
                SOURCE,
                "let reduction = wave.reduce_sum(&context, contribution);\n    let inclusive = wave.inclusive_scan_sum(&context, contribution);",
                "let inclusive = wave.inclusive_scan_sum(&context, contribution);\n    let reduction = wave.reduce_sum(&context, contribution);",
            ),
        ),
        (
            "collective-count",
            mutation(
                SOURCE,
                "let exclusive = wave.exclusive_scan_sum(&context, contribution);",
                "let exclusive = 0.0_f32;",
            ),
        ),
        (
            "inactive-output",
            mutation(SOURCE, "if active { reduction } else { 0.0 }", "reduction"),
        ),
        (
            "output-role",
            mutation(
                SOURCE,
                "reduction_output.get_mut(lane_index)",
                "inclusive_output.get_mut(lane_index)",
            ),
        ),
        (
            "output-ownership",
            mutation(
                SOURCE,
                "exclusive_output.get_mut(thread::index_1d())",
                "unsafe { exclusive_output.get_mut_at(0) }",
            ),
        ),
    ];
    for (label, source) in source_mutations {
        let result = compile(
            &workspace,
            &output,
            label,
            &source,
            CompilerProfile::default(),
        );
        assert_rejected(&result, label);
    }

    let profiles = [
        (
            "target",
            CompilerProfile {
                target: "gfx942",
                ..CompilerProfile::default()
            },
        ),
        (
            "compiler-crate-name",
            CompilerProfile {
                crate_name: "fe2o3_wave64_collectives_impostor",
                ..CompilerProfile::default()
            },
        ),
        (
            "descriptor-metadata",
            CompilerProfile {
                metadata: "fe2o3-wave64-collectives-v1-unreviewed",
                ..CompilerProfile::default()
            },
        ),
        (
            "crate-binding",
            CompilerProfile {
                crate_binding: "aa3fa024069d9cee1b86cf6fc1ad80a77d9de5457de020b70182cdc265e64569",
                ..CompilerProfile::default()
            },
        ),
        (
            "compiler-overflow-policy",
            CompilerProfile {
                overflow_checks: true,
                ..CompilerProfile::default()
            },
        ),
    ];
    for (label, profile) in profiles {
        let result = compile(&workspace, &output, label, SOURCE, profile);
        assert_rejected(&result, label);
    }
}
