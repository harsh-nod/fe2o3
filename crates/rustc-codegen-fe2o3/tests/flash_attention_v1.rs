use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};

const PIPELINE: &str = "collected-flash-attention-v1";
const CRATE_NAME: &str = "fe2o3_collected_flash_attention_v1_fixture";
const REVIEWED_METADATA: &str = "fe2o3-flash-attention-v1-reviewed";
const COMPILER_CRATE_BINDING: &str =
    "8b7c5dabd2bbc2855b328b84aa387119d8caae550aa6798779461ee3bed0bfc8";
const CARGO_METADATA_OBSERVATION: &str =
    "c1ab2dc02fa023687ac7394e15746c39668b5d46ad47c40eae012bc3f42d05c0";
const SOURCE_REMAP: &str = "/fe2o3-reviewed-workspace/flash-attention-v1.rs";
const WORKSPACE_REMAP: &str = "/fe2o3-reviewed-workspace";
const SOURCE: &str = include_str!("../../../examples/flash_attention_v1/src/kernel.rs");

static NEXT_OUTPUT: AtomicU64 = AtomicU64::new(0);
static FRONTEND_DEPENDENCIES: OnceLock<Result<(), String>> = OnceLock::new();

struct TestOutput {
    path: PathBuf,
}

impl TestOutput {
    fn new(workspace: &Path) -> Self {
        let path = cargo_target(workspace).join(format!(
            "flash-attention-v1-{}-{}",
            std::process::id(),
            NEXT_OUTPUT.fetch_add(1, Ordering::Relaxed)
        ));
        if path.exists() {
            std::fs::remove_dir_all(&path).expect("remove stale FlashAttention test output");
        }
        std::fs::create_dir_all(&path).expect("create FlashAttention test output");
        Self { path }
    }
}

impl Drop for TestOutput {
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
    cargo_target(workspace).join("flash-attention-v1-frontend-target")
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
    build_frontend_dependencies(workspace).expect("build FlashAttention frontend dependencies");
    let backend_target = cargo_target(workspace).join(profile_name());
    let frontend_target = frontend_target(workspace).join("debug");
    let backend = backend_target.join("librustc_codegen_fe2o3.so");
    let device = frontend_target.join("libfe2o3_device.rlib");
    let host = frontend_target.join("libfe2o3_host.rlib");
    for required in [&backend, &device, &host] {
        assert!(required.is_file(), "missing {}", required.display());
    }

    let contract = workspace.join("examples/flash_attention_v1/src/contract.rs");
    let kernel = output.path.join(format!("{label}-kernel.rs"));
    std::fs::write(&kernel, source).expect("write exact or hostile FlashAttention source");
    let crate_root = output.path.join(format!("{label}.rs"));
    std::fs::write(
        &crate_root,
        format!(
            "#![allow(missing_docs)]\n#[path = {:?}] mod contract;\n#[path = {:?}] mod kernel;\n",
            contract, kernel
        ),
    )
    .expect("write path-only FlashAttention fixture root");
    let artifact_dir = output.path.join(format!("{label}-artifacts"));
    std::fs::create_dir_all(&artifact_dir).expect("create empty artifact directory");
    let fixture_manifest =
        workspace.join("crates/rustc-codegen-fe2o3/tests/fixtures/collected-flash-attention-v1");

    Command::new(std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()))
        .current_dir(workspace)
        .arg(&crate_root)
        .arg(format!(
            "--remap-path-prefix={}={SOURCE_REMAP}",
            output.path.display()
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
        .expect("run FlashAttention compiler fixture")
}

fn mutation(source: &str, old: &str, new: &str) -> String {
    assert_eq!(source.matches(old).count(), 1, "non-unique mutation anchor");
    source.replacen(old, new, 1)
}

fn assert_rejected(result: &Output, label: &str) {
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(!result.status.success(), "hostile case `{label}` compiled");
    assert!(
        !stderr.contains("consumed sealed source authority"),
        "hostile case `{label}` consumed authenticated authority:\n{stderr}"
    );
}

fn copy_tree(source: &Path, destination: &Path) {
    std::fs::create_dir_all(destination)
        .unwrap_or_else(|error| panic!("create {}: {error}", destination.display()));
    for entry in std::fs::read_dir(source)
        .unwrap_or_else(|error| panic!("read {}: {error}", source.display()))
    {
        let entry = entry.expect("read source-tree entry");
        let file_type = entry.file_type().expect("inspect source-tree entry");
        let target = destination.join(entry.file_name());
        if file_type.is_dir() {
            copy_tree(&entry.path(), &target);
        } else if file_type.is_file() {
            std::fs::copy(entry.path(), &target)
                .unwrap_or_else(|error| panic!("copy {}: {error}", entry.path().display()));
        } else {
            panic!(
                "relocated source is not a regular file: {}",
                entry.path().display()
            );
        }
    }
}

fn copy_relocated_workspace(source: &Path, destination: &Path) {
    std::fs::create_dir_all(destination).expect("create relocated workspace");
    for file in ["Cargo.toml", "Cargo.lock", "rust-toolchain.toml"] {
        std::fs::copy(source.join(file), destination.join(file))
            .unwrap_or_else(|error| panic!("copy relocated {file}: {error}"));
    }
    copy_tree(&source.join("crates"), &destination.join("crates"));
    copy_tree(&source.join("examples"), &destination.join("examples"));
}

fn run_relocated_exact(workspace: &Path, target: &Path) -> Output {
    let mut command = Command::new(env!("CARGO"));
    command.current_dir(workspace).args(["test", "--locked"]);
    if !cfg!(debug_assertions) {
        command.arg("--release");
    }
    command
        .args([
            "-p",
            "rustc-codegen-fe2o3",
            "--test",
            "flash_attention_v1",
            "--target-dir",
        ])
        .arg(target)
        .args([
            "exact_phase_a_source_authenticates_complete_flash_attention_profile",
            "--",
            "--nocapture",
        ])
        .env("CARGO_TARGET_DIR", target)
        .env("CARGO_INCREMENTAL", "0")
        .env("FE2O3_FLASH_ATTENTION_REPORT_AUTHORITY", "1")
        .output()
        .expect("run relocated exact FlashAttention profile")
}

fn command_text(output: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

fn authenticated_authority(output: &Output) -> String {
    let text = command_text(output);
    let suffix = text
        .split_once("FLASH_ATTENTION_AUTHORITY ")
        .unwrap_or_else(|| panic!("missing authority marker:\n{text}"))
        .1;
    let authority = suffix.lines().next().expect("authority terminator").trim();
    assert_eq!(authority.len(), 64);
    assert!(authority.bytes().all(|byte| byte.is_ascii_hexdigit()));
    authority.to_owned()
}

#[test]
fn exact_phase_a_source_authenticates_complete_flash_attention_profile() {
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
    assert!(
        stderr.contains("authenticated exact attributed source bytes"),
        "exact admission failed:\n{stderr}"
    );
    assert!(
        !result.status.success(),
        "admission-only pipeline emitted code"
    );
    for marker in [
        "exact rustc FnAbi, location-independent V3 trusted definitions and reviewed semantic-terminal manifest",
        "complete reachable portable-MIR closure modulo those identity-bound terminals 0b017dd135cfce94f3a223126363b42853f5dbbf27c244cceafdd65f49e89e7e",
        "closed causal FlashAttention B1/H1/N8/D16 semantic KIR with 10 ordered recurrence steps",
        "adjacent-pair output ownership",
        "no generic lowering, terminal-body refinement, compiler-refinement proof, LLVM lowering, Worker V2, finalizer, link, host, runtime, artifact, load, launch, Verus refinement, or hardware authority",
    ] {
        assert!(stderr.contains(marker), "missing `{marker}`:\n{stderr}");
    }
    if std::env::var_os("FE2O3_FLASH_ATTENTION_REPORT_AUTHORITY").is_some() {
        let authority = stderr
            .split_once("consumed sealed source authority ")
            .expect("authenticated authority marker")
            .1
            .split_once(" (bound value ")
            .expect("authenticated authority terminator")
            .0;
        println!("FLASH_ATTENTION_AUTHORITY {authority}");
    }
    assert_eq!(
        std::fs::read_dir(output.path.join("exact-artifacts"))
            .expect("read admission-only artifact directory")
            .count(),
        0,
        "admission-only pipeline published an artifact"
    );
}

#[test]
fn hostile_source_mir_profile_and_ownership_mutations_fail_closed() {
    let workspace = workspace();
    let output = TestOutput::new(&workspace);
    let sources = [
        (
            "source-byte",
            format!("{SOURCE}\n// hostile source drift\n"),
        ),
        (
            "namespace",
            mutation(
                SOURCE,
                "4dfe870bb76dd32b49144ee70ec4925eab8677b7cbd1a1bfe99fa2294f85fec8",
                "5dfe870bb76dd32b49144ee70ec4925eab8677b7cbd1a1bfe99fa2294f85fec8",
            ),
        ),
        (
            "launch",
            mutation(
                SOURCE,
                "launch(required = [64, 1, 1], max = [64, 1, 1])",
                "launch(required = [32, 1, 1], max = [32, 1, 1])",
            ),
        ),
        (
            "finite-input",
            mutation(
                SOURCE,
                "|| !k[index].is_finite() || !v[index].is_finite()",
                "|| !k[index].is_finite()",
            ),
        ),
        (
            "dot-operation",
            mutation(SOURCE, "dot += product;", "dot -= product;"),
        ),
        (
            "dot-order",
            mutation(SOURCE, "dot += product;", "dot = product + dot;"),
        ),
        (
            "head-dimension",
            mutation(
                SOURCE,
                "feature < FLASH_ATTENTION_HEAD_DIMENSION_V1",
                "feature + 1 < FLASH_ATTENTION_HEAD_DIMENSION_V1",
            ),
        ),
        (
            "scale",
            mutation(
                SOURCE,
                "f32::from_bits(ATTENTION_SCALE_BITS_V1)",
                "f32::from_bits(0x3f00_0000)",
            ),
        ),
        (
            "causal-mask",
            mutation(
                SOURCE,
                "while key_row <= query_row",
                "while key_row < query_row",
            ),
        ),
        (
            "recurrence-maximum",
            mutation(SOURCE, "score > running_max", "score < running_max"),
        ),
        (
            "recurrence-exp-terminal",
            mutation(
                SOURCE,
                "let current_weight = math.exp_f32(score - next_max);",
                "let current_weight = 1.0_f32;",
            ),
        ),
        (
            "recurrence-order",
            mutation(
                SOURCE,
                "running_sum = running_sum * previous_weight + current_weight;",
                "running_sum = current_weight + running_sum * previous_weight;",
            ),
        ),
        (
            "output-ownership",
            mutation(
                SOURCE,
                "let first_output = lane * FLASH_ATTENTION_OUTPUT_ELEMENTS_PER_LANE_V1;",
                "let first_output = (lane / 2) * FLASH_ATTENTION_OUTPUT_ELEMENTS_PER_LANE_V1;",
            ),
        ),
        (
            "output-index",
            mutation(
                SOURCE,
                "output.get_mut_at(first_output + 1)",
                "output.get_mut_at(first_output)",
            ),
        ),
        (
            "output-value",
            mutation(SOURCE, "*second = values[1];", "*second = values[0];"),
        ),
    ];
    for (label, source) in sources {
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
            "crate-name",
            CompilerProfile {
                crate_name: "fe2o3_flash_attention_impostor",
                ..CompilerProfile::default()
            },
        ),
        (
            "metadata",
            CompilerProfile {
                metadata: "fe2o3-flash-attention-unreviewed",
                ..CompilerProfile::default()
            },
        ),
        (
            "crate-binding",
            CompilerProfile {
                crate_binding: "9b7c5dabd2bbc2855b328b84aa387119d8caae550aa6798779461ee3bed0bfc8",
                ..CompilerProfile::default()
            },
        ),
        (
            "overflow-policy",
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

#[test]
fn authority_is_location_independent_and_provider_source_bound() {
    let workspace = workspace();
    let output = TestOutput::new(&workspace);
    let location_a = output.path.join("canonical-workspace-a");
    let location_b = output.path.join("canonical-workspace-b");
    copy_relocated_workspace(&workspace, &location_a);
    copy_relocated_workspace(&workspace, &location_b);
    let location_a = location_a.canonicalize().expect("canonical workspace A");
    let location_b = location_b.canonicalize().expect("canonical workspace B");
    assert_ne!(location_a, location_b);

    let first = run_relocated_exact(&location_a, &output.path.join("relocated-target-a"));
    let second = run_relocated_exact(&location_b, &output.path.join("relocated-target-b"));
    assert!(
        first.status.success(),
        "workspace A failed:\n{}",
        command_text(&first)
    );
    assert!(
        second.status.success(),
        "workspace B failed:\n{}",
        command_text(&second)
    );
    assert_eq!(
        authenticated_authority(&first),
        authenticated_authority(&second)
    );

    let thread_source = location_b.join("crates/fe2o3-device/src/thread.rs");
    let mut hostile_source =
        std::fs::read_to_string(&thread_source).expect("read relocated device source");
    hostile_source.push_str("\n// hostile provider source substitution\n");
    std::fs::write(&thread_source, hostile_source).expect("mutate provider source");
    let hostile = run_relocated_exact(
        &location_b,
        &output.path.join("relocated-target-hostile-source"),
    );
    let hostile_text = command_text(&hostile);
    assert!(!hostile.status.success(), "mutated provider authenticated");
    assert!(
        hostile_text.contains("trusted-definition/semantic-terminal identity drifted"),
        "provider substitution did not fail at trusted identity:\n{hostile_text}"
    );
}
