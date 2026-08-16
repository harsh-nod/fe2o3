use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};

use fe2o3_artifact_transaction::{
    BuildAttempt, BuildInvocation, BuildSession, ProducerIdentity, begin_build_attempt,
    consume_compiler_module_handoff_v1,
};
use fe2o3_compiler_ffi::{
    CodeObjectVersion, CompilerModuleHandoffV2, CompilerModuleKindV1, CompilerModuleSymbolRoleV1,
};
use sha2::{Digest as _, Sha256};

const PIPELINE: &str = "collected-moe-top2-v1";
const CRATE_NAME: &str = "fe2o3_collected_moe_top2_v1_fixture";
const REVIEWED_METADATA: &str = "fe2o3-moe-top2-v1-reviewed";
const COMPILER_CRATE_BINDING: &str =
    "fce826d20b8f2e4eca29180a2d9fc34949b51a07841dd7f79258625fc6a9f296";
const CARGO_METADATA_OBSERVATION: &str =
    "c1ab2dc02fa023687ac7394e15746c39668b5d46ad47c40eae012bc3f42d05c0";
const SOURCE_REMAP: &str = "/fe2o3-reviewed-workspace/moe-top2-v1.rs";
const WORKSPACE_REMAP: &str = "/fe2o3-reviewed-workspace";
const SOURCE: &str = include_str!("../../../examples/moe_top2_v1/src/kernel.rs");

static NEXT_OUTPUT: AtomicU64 = AtomicU64::new(0);
static FRONTEND_DEPENDENCIES: OnceLock<Result<(), String>> = OnceLock::new();

struct TestOutput {
    path: PathBuf,
}

struct CompileResult {
    process: Output,
    producer: ProducerIdentity,
    attempt: BuildAttempt,
    artifact_dir: PathBuf,
}

impl TestOutput {
    fn new(workspace: &Path) -> Self {
        let path = cargo_target(workspace).join(format!(
            "moe-top2-v1-{}-{}",
            std::process::id(),
            NEXT_OUTPUT.fetch_add(1, Ordering::Relaxed)
        ));
        if path.exists() {
            std::fs::remove_dir_all(&path).expect("remove stale MoE top-2 test output");
        }
        std::fs::create_dir_all(&path).expect("create MoE top-2 test output");
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
    cargo_metadata_observation: &'a str,
    source_remap: &'a str,
    workspace_remap: &'a str,
    mir_enable_passes: &'a str,
    overflow_checks: bool,
}

impl Default for CompilerProfile<'static> {
    fn default() -> Self {
        Self {
            target: "gfx942:xnack-",
            crate_name: CRATE_NAME,
            metadata: REVIEWED_METADATA,
            crate_binding: COMPILER_CRATE_BINDING,
            cargo_metadata_observation: CARGO_METADATA_OBSERVATION,
            source_remap: SOURCE_REMAP,
            workspace_remap: WORKSPACE_REMAP,
            mir_enable_passes: "-JumpThreading",
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
    cargo_target(workspace).join("moe-top2-v1-frontend-target")
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

fn begin_fixture_attempt(
    crate_root: &Path,
    artifact_dir: &Path,
    source: &str,
    profile: CompilerProfile<'_>,
) -> (ProducerIdentity, BuildAttempt) {
    let producer = ProducerIdentity::from_codegen(profile.crate_name, Some(crate_root))
        .expect("construct exact MoE top-2 fixture producer");
    let mut invocation = Sha256::new();
    for field in [
        b"FE2O3/MOE-TOP2-V1/TEST-INVOCATION/V1\0".as_slice(),
        source.as_bytes(),
        profile.target.as_bytes(),
        profile.crate_name.as_bytes(),
        profile.metadata.as_bytes(),
        profile.crate_binding.as_bytes(),
        profile.cargo_metadata_observation.as_bytes(),
        profile.source_remap.as_bytes(),
        profile.workspace_remap.as_bytes(),
        profile.mir_enable_passes.as_bytes(),
        artifact_dir.as_os_str().as_encoded_bytes(),
        &[u8::from(profile.overflow_checks)],
    ] {
        invocation.update((field.len() as u64).to_le_bytes());
        invocation.update(field);
    }
    let attempt = begin_build_attempt(
        artifact_dir,
        &producer,
        BuildInvocation::from_bytes(invocation.finalize().into()),
        BuildSession::from_bytes(*b"FE2O3-MOE-TOP2V1"),
    )
    .expect("begin exact MoE top-2 managed fixture attempt");
    (producer, attempt)
}

fn compile(
    workspace: &Path,
    output: &TestOutput,
    label: &str,
    source: &str,
    profile: CompilerProfile<'_>,
) -> CompileResult {
    build_frontend_dependencies(workspace).expect("build MoE top-2 frontend dependencies");
    let backend_target = cargo_target(workspace).join(profile_name());
    let frontend_target = frontend_target(workspace).join("debug");
    let backend = backend_target.join("librustc_codegen_fe2o3.so");
    let device = frontend_target.join("libfe2o3_device.rlib");
    let host = frontend_target.join("libfe2o3_host.rlib");
    for required in [&backend, &device, &host] {
        assert!(required.is_file(), "missing {}", required.display());
    }

    let contract = workspace.join("examples/moe_top2_v1/src/contract.rs");
    let exact_kernel = workspace.join("examples/moe_top2_v1/src/kernel.rs");
    let kernel = if source == SOURCE {
        exact_kernel
    } else {
        let path = output.path.join(format!("{label}-kernel.rs"));
        std::fs::write(&path, source).expect("write hostile MoE top-2 source");
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
    .expect("write path-only MoE top-2 fixture root");
    let artifact_dir = output.path.join(format!("{label}-artifacts"));
    std::fs::create_dir_all(&artifact_dir).expect("create empty artifact directory");
    let (producer, attempt) = begin_fixture_attempt(&crate_root, &artifact_dir, source, profile);
    let fixture_manifest =
        workspace.join("crates/rustc-codegen-fe2o3/tests/fixtures/collected-moe-top2-v1");

    let process = Command::new(std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()))
        .current_dir(workspace)
        .arg(&crate_root)
        .arg(format!(
            "--remap-path-prefix={}={}",
            kernel.display(),
            profile.source_remap,
        ))
        .arg(format!(
            "--remap-path-prefix={}={}",
            workspace.display(),
            profile.workspace_remap,
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
        .arg(format!("-Zmir-enable-passes={}", profile.mir_enable_passes))
        .arg(format!("-Zcodegen-backend={}", backend.display()))
        .arg("-o")
        .arg(output.path.join(format!("lib{label}.rlib")))
        .env("CARGO_MANIFEST_DIR", fixture_manifest)
        .env("FE2O3_CRATE_BINDING_ID_V1", profile.crate_binding)
        .env(
            "FE2O3_CARGO_METADATA_BUILD_OBSERVATION_V2",
            profile.cargo_metadata_observation,
        )
        .env("FE2O3_HSACO_DIR", &artifact_dir)
        .env("FE2O3_BUILD_ATTEMPT_V1", attempt.to_env_value())
        .env("FE2O3_TARGET", profile.target)
        .env("FE2O3_CODEGEN_PIPELINE", PIPELINE)
        .output()
        .expect("run MoE top-2 compiler fixture");
    CompileResult {
        process,
        producer,
        attempt,
        artifact_dir,
    }
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
            "moe_top2_v1",
            "--target-dir",
        ])
        .arg(target)
        .args([
            "exact_phase_a_source_authenticates_complete_moe_top2_profile",
            "--",
            "--nocapture",
        ])
        .env("CARGO_TARGET_DIR", target)
        .env("CARGO_INCREMENTAL", "0")
        .env("FE2O3_MOE_TOP2_REPORT_AUTHORITY", "1")
        .output()
        .expect("run relocated exact MoE top-2 profile")
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
        .split_once("MOE_TOP2_AUTHORITY ")
        .unwrap_or_else(|| panic!("missing authority marker:\n{text}"))
        .1;
    let authority = suffix.lines().next().expect("authority terminator").trim();
    assert_eq!(authority.len(), 64);
    assert!(authority.bytes().all(|byte| byte.is_ascii_hexdigit()));
    authority.to_owned()
}

#[test]
fn exact_phase_a_source_authenticates_complete_moe_top2_profile() {
    let workspace = workspace();
    let output = TestOutput::new(&workspace);
    let result = compile(
        &workspace,
        &output,
        "exact",
        SOURCE,
        CompilerProfile::default(),
    );
    let stderr = String::from_utf8_lossy(&result.process.stderr);
    assert!(
        stderr.contains("authenticated exact attributed source bytes"),
        "exact admission failed:\n{stderr}"
    );
    assert!(
        result.process.status.success(),
        "exact handoff failed:\n{stderr}"
    );
    for marker in [
        "exact rustc FnAbi, location-independent V3 trusted definitions and reviewed semantic-terminal manifest",
        "complete reachable portable-MIR closure modulo those identity-bound terminals 934c2205973e24216d537c5f89bc65d8e15dd68376dce477d1768e2936b4fc13",
        "closed deterministic finite-input MoE top-2 T8/E4/K2/C4 semantic KIR",
        "with 10 ordered routing steps",
        "lane-zero exclusive output ownership",
        "stable-prefix capacity dropping, permutation/inverse and sentinel-tail semantics",
        "published an inert Worker V2 compiler-module handoff",
        "one explicit kernel, five private helpers, no providers/imports, canonical target-machine layout identity, exact COV6 ABI/resources/effects",
        "no generic lowering, IEEE FP32 refinement, terminal-body refinement, compiler-refinement proof, source-to-Verus/model refinement, worker execution, finalizer, link result, artifact, host, runtime, load, launch, GPU, or hardware authority",
    ] {
        assert!(stderr.contains(marker), "missing `{marker}`:\n{stderr}");
    }
    if std::env::var_os("FE2O3_MOE_TOP2_REPORT_AUTHORITY").is_some() {
        let authority = stderr
            .split_once("consumed sealed source authority ")
            .expect("authenticated authority marker")
            .1
            .split_once(" (bound value ")
            .expect("authenticated authority terminator")
            .0;
        println!("MOE_TOP2_AUTHORITY {authority}");
    }
    let consumed =
        consume_compiler_module_handoff_v1(&result.artifact_dir, &result.producer, result.attempt)
            .expect("consume exact MoE top-2 compiler handoff once");
    assert_eq!(consumed.attempt(), result.attempt);
    assert!(!consumed.grants_compiler_authority());
    assert!(!consumed.grants_link_authority());
    assert!(!consumed.grants_load_authority());
    assert!(!consumed.grants_launch_authority());
    let handoff = CompilerModuleHandoffV2::decode(consumed.bytes())
        .expect("decode exact canonical MoE top-2 Worker V2 handoff");
    assert_eq!(handoff.canonical_bytes(), consumed.bytes());
    assert_eq!(handoff.kind(), CompilerModuleKindV1::LlvmTextIr);
    assert_eq!(
        handoff.target().as_amd_target_id().to_string(),
        "gfx942:xnack-"
    );
    assert_eq!(handoff.code_object_version(), CodeObjectVersion::V6);
    assert_eq!(
        handoff
            .symbol_manifest()
            .symbols(CompilerModuleSymbolRoleV1::KernelEntry)
            .collect::<Vec<_>>(),
        ["moe_top2_route_f32_t8_e4_k2_c4_v1"]
    );
    assert_eq!(
        handoff
            .symbol_manifest()
            .symbols(CompilerModuleSymbolRoleV1::KernelDescriptor)
            .collect::<Vec<_>>(),
        ["moe_top2_route_f32_t8_e4_k2_c4_v1.kd"]
    );
    assert!(!handoff.authenticates_compiler_origin());
    assert!(!handoff.grants_worker_authority());
    assert!(!handoff.grants_link_authority());
    assert!(!handoff.grants_load_authority());
    assert!(!handoff.grants_launch_authority());
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
                "4180ef61545684e646bd5227333e7514d22a2d379d7d657397df4d41f7a192d1",
                "5180ef61545684e646bd5227333e7514d22a2d379d7d657397df4d41f7a192d1",
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
                "pub fn moe_top2_route_f32_t8_e4_k2_c4_v1(\n    logits: &[f32]",
                "pub fn moe_top2_route_f32_t8_e4_k2_c4_v1(\n    logits: &mut [f32]",
            ),
        ),
        (
            "abi-output-type",
            mutation(
                SOURCE,
                "mut top2_experts: DisjointSlice<u32>",
                "top2_experts: &[u32]",
            ),
        ),
        (
            "abi-output-element",
            mutation(
                SOURCE,
                "mut inverse: DisjointSlice<u32>",
                "mut inverse: DisjointSlice<u64>",
            ),
        ),
        (
            "finite-input",
            mutation(
                SOURCE,
                "if !logits[index].is_finite()",
                "if false && !logits[index].is_finite()",
            ),
        ),
        (
            "tie-break",
            mutation(
                SOURCE,
                "candidate_expert < incumbent_expert",
                "candidate_expert > incumbent_expert",
            ),
        ),
        (
            "selection-call",
            mutation(
                SOURCE,
                "candidate_precedes_v1(score, expert, logits[token * MOE_EXPERTS_V1 + best], best)",
                "candidate_precedes_v1(logits[token * MOE_EXPERTS_V1 + best], best, score, expert)",
            ),
        ),
        (
            "selection-cfg",
            mutation(
                SOURCE,
                "else if second == usize::MAX",
                "else if best == usize::MAX",
            ),
        ),
        (
            "expert-loop-bound",
            mutation(
                SOURCE,
                "while expert < MOE_EXPERTS_V1 {\n        let score =",
                "while expert + 1 < MOE_EXPERTS_V1 {\n        let score =",
            ),
        ),
        (
            "requested-count",
            mutation(
                SOURCE,
                "staged_requested[selected[1] as usize] += 1;",
                "staged_requested[selected[1] as usize] += 2;",
            ),
        ),
        (
            "capacity-constant",
            mutation(
                SOURCE,
                "stable_rank < MOE_EXPERT_CAPACITY_V1 as u32",
                "stable_rank < 3_u32",
            ),
        ),
        (
            "admitted-clamp",
            mutation(
                SOURCE,
                "staged_requested[expert] > MOE_EXPERT_CAPACITY_V1 as u32",
                "staged_requested[expert] < MOE_EXPERT_CAPACITY_V1 as u32",
            ),
        ),
        (
            "exclusive-scan-operation",
            mutation(
                SOURCE,
                "staged_offsets[expert] + staged_admitted[expert]",
                "staged_offsets[expert] - staged_admitted[expert]",
            ),
        ),
        (
            "stable-prefix-order",
            mutation(SOURCE, "let mut route = 0;", "let mut route = 1;"),
        ),
        (
            "slot-operation",
            mutation(
                SOURCE,
                "staged_offsets[route_expert] + stable_rank",
                "staged_offsets[route_expert] - stable_rank",
            ),
        ),
        (
            "permutation-value",
            mutation(
                SOURCE,
                "staged_permutation[slot as usize] = route as u32;",
                "staged_permutation[slot as usize] = route as u32 + 1;",
            ),
        ),
        (
            "inverse-value",
            mutation(
                SOURCE,
                "staged_inverse[route] = slot;",
                "staged_inverse[route] = DROP_ROUTE_V1;",
            ),
        ),
        (
            "sentinel-initialization",
            mutation(
                SOURCE,
                "let mut staged_permutation = [DROP_ROUTE_V1; MOE_ROUTES_V1];",
                "let mut staged_permutation = [0_u32; MOE_ROUTES_V1];",
            ),
        ),
        (
            "ownership-lane",
            mutation(SOURCE, "if lane != 0", "if lane > 1"),
        ),
        (
            "commit-index",
            mutation(
                SOURCE,
                "write_value_v1(&mut inverse, index, staged_inverse[index]);",
                "write_value_v1(&mut inverse, index, staged_inverse[0]);",
            ),
        ),
        (
            "terminal-call",
            mutation(
                SOURCE,
                "if lane >= 64 {\n        fe2o3_device::trap();\n        return;\n    }",
                "if lane >= 64 {\n        return;\n    }",
            ),
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
        assert_rejected(&result.process, label);
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
                crate_name: "fe2o3_moe_top2_impostor",
                ..CompilerProfile::default()
            },
        ),
        (
            "metadata",
            CompilerProfile {
                metadata: "fe2o3-moe-top2-unreviewed",
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
            "package-observation",
            CompilerProfile {
                cargo_metadata_observation: "d1ab2dc02fa023687ac7394e15746c39668b5d46ad47c40eae012bc3f42d05c0",
                ..CompilerProfile::default()
            },
        ),
        (
            "source-path-relocation",
            CompilerProfile {
                source_remap: "/fe2o3-reviewed-workspace/relocated-moe-top2-v1.rs",
                ..CompilerProfile::default()
            },
        ),
        (
            "workspace-path-relocation",
            CompilerProfile {
                workspace_remap: "/unreviewed-workspace",
                ..CompilerProfile::default()
            },
        ),
        (
            "mir-passes",
            CompilerProfile {
                mir_enable_passes: "+JumpThreading",
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
        assert_rejected(&result.process, label);
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
