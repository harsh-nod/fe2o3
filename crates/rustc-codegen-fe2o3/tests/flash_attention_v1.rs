use std::io::Write as _;
use std::os::unix::fs::{MetadataExt as _, OpenOptionsExt as _, PermissionsExt as _};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};

use fe2o3_artifact_transaction::{
    BuildInvocation, BuildSession, ProducerIdentity, begin_build_attempt,
    consume_compiler_module_handoff_v1,
};
use fe2o3_compiler_ffi::CompilerModuleHandoffV2;
use sha2::{Digest as _, Sha256};

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
const HANDOFF_OUTPUT_ENV: &str = "FE2O3_FLASH_ATTENTION_HANDOFF_OUTPUT";
const MODULE_OUTPUT_ENV: &str = "FE2O3_FLASH_ATTENTION_MODULE_OUTPUT";

static NEXT_OUTPUT: AtomicU64 = AtomicU64::new(0);
static FRONTEND_DEPENDENCIES: OnceLock<Result<(), String>> = OnceLock::new();

struct TestOutput {
    path: PathBuf,
    guard_directory: PathBuf,
    guard_identity: String,
}

impl TestOutput {
    fn new(workspace: &Path) -> Self {
        fe2o3_artifact_transaction::enable_same_mount_namespace_artifact_path_guard_v1();
        let path = cargo_target(workspace).join(format!(
            "flash-attention-v1-{}-{}",
            std::process::id(),
            NEXT_OUTPUT.fetch_add(1, Ordering::Relaxed)
        ));
        if path.exists() {
            std::fs::remove_dir_all(&path).expect("remove stale FlashAttention test output");
        }
        std::fs::create_dir_all(&path).expect("create FlashAttention test output");
        let guard_directory = path.join("artifact-path-guard");
        std::fs::create_dir(&guard_directory).expect("create FlashAttention artifact path guard");
        std::fs::set_permissions(&guard_directory, std::fs::Permissions::from_mode(0o700))
            .expect("secure FlashAttention artifact path guard");
        let metadata = std::fs::metadata(&guard_directory)
            .expect("inspect FlashAttention artifact path guard");
        let guard_identity = format!("{:016x}:{:016x}", metadata.dev(), metadata.ino());
        Self {
            path,
            guard_directory,
            guard_identity,
        }
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
    let exact_kernel = workspace.join("examples/flash_attention_v1/src/kernel.rs");
    let kernel = if source == SOURCE {
        exact_kernel
    } else {
        let path = output.path.join(format!("{label}-kernel.rs"));
        std::fs::write(&path, source).expect("write hostile FlashAttention source");
        path
    };
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
    let producer = ProducerIdentity::from_codegen(profile.crate_name, Some(&crate_root))
        .expect("FlashAttention fixture producer");
    let attempt = begin_build_attempt(
        &artifact_dir,
        &producer,
        BuildInvocation::from_bytes(Sha256::digest(source.as_bytes()).into()),
        BuildSession::from_bytes([0x46; 16]),
    )
    .expect("begin FlashAttention managed build attempt");
    let fixture_manifest =
        workspace.join("crates/rustc-codegen-fe2o3/tests/fixtures/collected-flash-attention-v1");

    let result = Command::new(std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()))
        .current_dir(workspace)
        .arg(&crate_root)
        .arg(format!(
            "--remap-path-prefix={}={SOURCE_REMAP}",
            kernel.display()
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
        .env("FE2O3_BUILD_ATTEMPT_V1", attempt.to_env_value())
        .env("FE2O3_ARTIFACT_PATH_GUARD_DIR", &output.guard_directory)
        .env(
            "FE2O3_ARTIFACT_PATH_GUARD_DIR_IDENTITY",
            &output.guard_identity,
        )
        .env("FE2O3_TARGET", profile.target)
        .env("FE2O3_QUALIFICATION_ORACLE_V1", PIPELINE)
        .output()
        .expect("run FlashAttention compiler fixture");
    if result.status.success()
        && let Some(destination) = std::env::var_os(HANDOFF_OUTPUT_ENV)
    {
        let consumed = consume_compiler_module_handoff_v1(&artifact_dir, &producer, attempt)
            .expect("consume exact FlashAttention compiler handoff");
        let decoded = CompilerModuleHandoffV2::decode(consumed.bytes())
            .expect("decode exact FlashAttention compiler handoff");
        assert_eq!(decoded.canonical_bytes(), consumed.bytes());
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(destination)
            .expect("create private FlashAttention handoff output");
        file.write_all(consumed.bytes())
            .expect("write exact FlashAttention handoff output");
        if let Some(module_destination) = std::env::var_os(MODULE_OUTPUT_ENV) {
            let mut module_file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(module_destination)
                .expect("create private FlashAttention LLVM output");
            module_file
                .write_all(decoded.module_bytes())
                .expect("write exact FlashAttention LLVM output");
        }
    }
    result
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
    assert!(result.status.success(), "exact handoff failed:\n{stderr}");
    for marker in [
        "exact rustc FnAbi, location-independent V5 provider-semantic definitions and reviewed semantic-terminal manifest",
        "complete reachable portable-MIR closure modulo those identity-bound terminals 36f26659b1d8e722ee5358d0b87be34b26ddd22a914376f3ec582843da9c0fc9",
        "closed causal FlashAttention B1/H1/N8/D16 semantic KIR with 10 ordered recurrence steps",
        "adjacent-pair output ownership",
        "published an inert Worker V2 compiler handoff",
        "this grants no terminal-body or compiler-refinement proof, exponential-law/IEEE/OCML semantic proof",
    ] {
        assert!(stderr.contains(marker), "missing `{marker}`:\n{stderr}");
    }
    if std::env::var_os("FE2O3_FLASH_ATTENTION_REPORT_AUTHORITY").is_some() {
        let authority = stderr
            .split_once("consumed sealed source authority ")
            .expect("authenticated authority marker")
            .1
            .split_once(" to select closed causal")
            .expect("authenticated authority terminator")
            .0;
        println!("FLASH_ATTENTION_AUTHORITY {authority}");
    }
    assert!(
        std::fs::read_dir(output.path.join("exact-artifacts"))
            .expect("read compiler-handoff artifact directory")
            .next()
            .is_some(),
        "exact pipeline did not publish its managed handoff"
    );
}

#[test]
fn hostile_source_mir_profile_and_ownership_mutations_fail_closed() {
    let workspace = workspace();
    let output = TestOutput::new(&workspace);
    let baseline = compile(
        &workspace,
        &output,
        "hostile-suite-baseline",
        SOURCE,
        CompilerProfile::default(),
    );
    assert!(
        baseline.status.success(),
        "hostile suite baseline failed before any mutation:\n{}",
        command_text(&baseline)
    );
    let sources = [
        (
            "source-byte",
            format!("{SOURCE}\n// hostile source drift\n"),
        ),
        (
            "explicit-namespace",
            mutation(
                SOURCE,
                "    typed,\n",
                "    typed,\n    namespace = \"4dfe870bb76dd32b49144ee70ec4925eab8677b7cbd1a1bfe99fa2294f85fec8\",\n",
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
            "abi-mutability",
            mutation(
                SOURCE,
                "pub fn flash_attention_causal_f32_b1_h1_n8_d16_v1(\n    q: &[f32]",
                "pub fn flash_attention_causal_f32_b1_h1_n8_d16_v1(\n    q: &mut [f32]",
            ),
        ),
        (
            "abi-output-type",
            mutation(
                SOURCE,
                "mut output: DisjointSlice<f32, Blocked<Index1D, 1, 2>>",
                "output: &[f32]",
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
            "output-block-witness",
            mutation(
                SOURCE,
                "lane_index.checked_block::<1, 2>()",
                "lane_index.checked_block::<2, 2>()",
            ),
        ),
        (
            "output-index",
            mutation(
                SOURCE,
                "output.get_block_mut(&output_block, 1)",
                "output.get_block_mut(&output_block, 0)",
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
        hostile_text.contains(
            "safe execution provider source closure does not match the reviewed V1 identity"
        ),
        "provider substitution did not fail at provider-source admission:\n{hostile_text}"
    );
}
