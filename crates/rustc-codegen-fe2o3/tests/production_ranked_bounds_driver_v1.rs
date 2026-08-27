use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};

struct ScratchTarget {
    path: PathBuf,
}

impl ScratchTarget {
    fn new() -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock after Unix epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "fe2o3-production-ranked-bounds-{}-{nonce}",
            std::process::id()
        ));
        std::fs::create_dir(&path).expect("create ranked bounds target directory");
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for ScratchTarget {
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

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[test]
#[ignore = "requires the pinned nightly rust-src component and AMD target"]
fn ordinary_rust_bounds_and_production_pliron_pipeline_fail_closed() {
    let safe = run_extraction(&ScratchTarget::new(), false);
    assert!(
        safe.status.success()
            && safe
                .stderr
                .contains("all mandatory kernel checks clean true")
            && safe.stderr.contains("kernel.cond_br")
            && safe.stderr.contains("kernel.access Write")
            && !safe.stderr.contains("error[FE2O3-BOUNDS-001]"),
        "safe checked dynamic access did not pass generic PLIRON verification:\n{}",
        safe.stderr
    );

    let shifted = run_feature_extraction(&ScratchTarget::new(), "shifted");
    assert!(
        shifted.status.success()
            && shifted
                .stderr
                .contains("all mandatory kernel checks clean true")
            && shifted.stderr.contains("kernel.index_binary Add")
            && shifted.stderr.contains("kernel.cond_br")
            && shifted.stderr.contains("kernel.access Write"),
        "safe shifted disjoint access did not pass production extraction:\n{}",
        shifted.stderr,
    );

    let exclusive = run_feature_extraction(&ScratchTarget::new(), "grid_exclusive");
    assert!(
        exclusive.status.success()
            && exclusive
                .stderr
                .contains("all mandatory kernel checks clean true")
            && exclusive.stderr.contains("kernel.index_constant 7")
            && exclusive.stderr.contains("kernel.cond_br")
            && exclusive.stderr.contains("kernel.access Write"),
        "safe grid-exclusive access did not pass production extraction:\n{}",
        exclusive.stderr,
    );

    let blocked = run_feature_extraction(&ScratchTarget::new(), "blocked");
    assert!(
        blocked.status.success()
            && blocked
                .stderr
                .contains("all mandatory kernel checks clean true")
            && blocked.stderr.contains("kernel.index_binary Multiply")
            && blocked.stderr.contains("kernel.index_binary Add")
            && blocked.stderr.contains("kernel.access Write"),
        "safe blocked disjoint access did not pass production extraction:\n{}",
        blocked.stderr,
    );

    let oob = run_extraction(&ScratchTarget::new(), true);
    assert!(
        !oob.status.success(),
        "out-of-bounds Rust kernel was accepted"
    );
    assert!(
        oob.stderr.contains("error[FE2O3-BOUNDS-001]")
            && oob.stderr.contains("required: 64 < 64")
            && oob.stderr.contains("Rust source")
            && oob.stderr.contains(":63:20")
            && oob.stderr.contains("kernel.index_constant 64")
            && oob
                .stderr
                .contains("ranked PLIRON before rejected lowering")
            && oob
                .stderr
                .contains("lowering stopped before target IR or artifact emission"),
        "out-of-bounds diagnostic was incomplete:\n{}",
        oob.stderr,
    );
    for forbidden in ["kernel-ir-v1", "GeneralGemm", "Unknown/Unproved"] {
        assert!(
            !safe.stderr.contains(forbidden) && !oob.stderr.contains(forbidden),
            "production extraction entered forbidden path {forbidden:?}",
        );
    }
}

#[test]
#[ignore = "requires the pinned nightly rust-src component and AMD target"]
fn production_barrier_cfg_preserves_order_and_fails_closed() {
    for feature in ["barrier_after_access", "barrier_before_access"] {
        let output = run_feature_extraction(&ScratchTarget::new(), feature);
        assert!(
            output.status.success()
                && output
                    .stderr
                    .contains("all mandatory kernel checks clean true")
                && output.stderr.contains("kernel.access Write")
                && output.stderr.contains("gpu.barrier"),
            "{feature} did not preserve a clean ranked CFG:\n{}",
            output.stderr,
        );
    }

    for feature in ["barrier_divergent", "barrier_early_return"] {
        let output = run_feature_extraction(&ScratchTarget::new(), feature);
        assert!(
            !output.status.success()
                && output.stderr.contains("error[FE2O3-BARRIER-001]")
                && output.stderr.contains("divergent collective barrier paths"),
            "{feature} did not fail closed as divergent:\n{}",
            output.stderr,
        );
    }

    let cyclic = run_feature_extraction(&ScratchTarget::new(), "barrier_loop");
    assert!(
        !cyclic.status.success()
            && cyclic.stderr.contains("error[FE2O3-BARRIER-002]")
            && cyclic.stderr.contains("cyclic control flow"),
        "cyclic barrier did not remain incomplete:\n{}",
        cyclic.stderr,
    );

    let helper = run_feature_extraction(&ScratchTarget::new(), "barrier_helper");
    assert!(
        !helper.status.success()
            && helper.stderr.contains(
                "a call terminator before exact callable memory-effect summaries are available"
            ),
        "helper-mediated barrier bypassed the semantic boundary:\n{}",
        helper.stderr,
    );
}

#[test]
#[ignore = "requires the pinned nightly rust-src component and AMD target"]
fn ordinary_kernel_source_exports_one_verified_authority_free_simulation_bundle() {
    let target = ScratchTarget::new();
    let bundle_path = target.path().join("copy-static.fe2sim");
    let mut command = base_command("check", target.path());
    command
        .env(
            "RUSTC_WORKSPACE_WRAPPER",
            env!("CARGO_BIN_EXE_fe2o3-rustc-extract"),
        )
        .env(
            "FE2O3_EXTRACT_CRATE_V1",
            "fe2o3_production_ranked_bounds_fixture",
        )
        .env("FE2O3_EXTRACT_SIMULATION_BUNDLE_PATH_V1", &bundle_path)
        .args(["--features", "barrier_before_access"]);
    let result = output(command, "run production simulation-bundle extraction");

    assert!(
        result.status.success(),
        "ordinary source simulation-bundle extraction failed:\n{}",
        result.stderr,
    );
    assert!(
        result
            .stderr
            .contains("sole target-neutral Kernel IR lowering")
            && result
                .stderr
                .contains("exact verified KIR V7 simulation bundle")
            && result
                .stderr
                .contains("compiler_execution_binding=extraction_only_unavailable")
            && result
                .stderr
                .contains("authenticates_compiler_execution=false")
            && result
                .stderr
                .contains("proof/artifact/compiler/hardware/load/launch authority false"),
        "simulation-bundle diagnostic overclaimed or omitted its transaction boundary:\n{}",
        result.stderr,
    );
    let bytes = std::fs::read(&bundle_path).expect("read exact simulation bundle");
    let bundle = fe2o3_kernel_ir::VerifiedSimulationBundleV1::from_canonical_bytes(bytes)
        .expect("decode compiler-produced simulation bundle");
    assert_eq!(bundle.target(), "gfx942:xnack-");
    assert_eq!(bundle.kernel_count(), 1);
    assert_eq!(
        bundle.compiler_execution_binding(),
        &fe2o3_kernel_ir::SimulationCompilerExecutionBindingV1::UnavailableExtractionOnly
    );
    assert!(
        bundle
            .require_canonical_compiler_execution_association()
            .is_err()
    );
    assert!(
        bundle
            .source_lineage()
            .rustc_identity_inventory_receipt_bytes()
            > 0
    );
    assert!(bundle.source_lineage().rustc_preflight_plan_receipt_bytes() > 0);
    let map_bytes = bundle
        .debug_map()
        .expect("compiler extraction embeds one exact source map");
    let map = fe2o3_kernel_ir::DebugSourceMapDocumentV1::from_json_bytes(map_bytes)
        .expect("compiler source map uses the strict shared codec");
    assert_eq!(
        map.binding().bundle_subject_identity(),
        *bundle.subject_identity()
    );
    assert_eq!(
        map.binding().canonical_kir().digest(),
        *bundle.canonical_kir_v7_identity().digest()
    );
    assert_eq!(
        map.binding().canonical_kir().canonical_bytes(),
        bundle.canonical_kir_v7_identity().canonical_length()
    );
    assert_eq!(
        bundle.debug_map_identity(),
        Some(fe2o3_kernel_ir::simulation_debug_map_identity_v1(map_bytes))
    );
    let source_file = map
        .files()
        .iter()
        .find(|file| {
            file.display_path()
                .ends_with("production-ranked-bounds-device/src/lib.rs")
        })
        .expect("map retains the ordinary-source display path");
    assert_eq!(
        source_file.byte_len(),
        std::fs::metadata(workspace().join(
            "crates/rustc-codegen-fe2o3/tests/fixtures/production-ranked-bounds-device/src/lib.rs",
        ))
        .unwrap()
        .len()
    );
    assert!(!map.sites().is_empty());
    assert!(!map.eliminated().is_empty());
    assert!(map.sites().windows(2).any(|sites| {
        sites[0].site().function_ordinal() == sites[1].site().function_ordinal()
            && sites[0].site().block_ordinal() == sites[1].site().block_ordinal()
            && sites[0].site().operation_ordinal().checked_add(1)
                == Some(sites[1].site().operation_ordinal())
            && sites[0].spans() == sites[1].spans()
    }));
    assert!(!bundle.canonical_kir_v7().is_empty());
    assert!(!bundle.grants_proof_authority());
    assert!(!bundle.grants_artifact_authority());
    assert!(!bundle.grants_compiler_authority());
    assert!(!bundle.grants_hardware_authority());
    assert!(!bundle.grants_load_authority());
    assert!(!bundle.grants_launch_authority());
    assert!(!bundle.authenticates_compiler_execution());

    let simulation_request_path = target.path().join("request.json");
    std::fs::write(
        &simulation_request_path,
        serde_json::to_vec(&json!({
            "schema": "fe2o3-simulation-request-v1",
            "kernel": "barrier_before_access",
            "grid": [1, 1, 1],
            "workgroup": [64, 1, 1],
            "arguments": [{
                "kind": "buffer",
                "element": "f32",
                "access": "read_write",
                "alignment": 4,
                "bytes": format!("0x{}", "00".repeat(64 * 4)),
            }],
        }))
        .unwrap(),
    )
    .unwrap();
    let (mapped_site, breakpoint_byte) = map
        .sites()
        .iter()
        .find_map(|site| {
            site.spans()
                .iter()
                .filter(|span| span.file_identity() == source_file.identity())
                .find_map(|span| {
                    (span.byte_start()..span.byte_end())
                        .find(|byte| {
                            map.sites()
                                .iter()
                                .flat_map(|other| other.spans())
                                .filter(|other_span| {
                                    other_span.file_identity() == source_file.identity()
                                        && other_span.byte_start() < *byte + 1
                                        && *byte < other_span.byte_end()
                                })
                                .count()
                                == 1
                        })
                        .map(|byte| (site, byte))
                })
        })
        .expect("ordinary source has at least one unambiguous mapped source byte");
    let map_identity = bundle.debug_map_identity().unwrap();
    let site = mapped_site.site();
    let mut protocol_input = Vec::new();
    for request in [
        json!({
            "operation": "resolve_source",
            "schema": "fe2o3-debug-request-v1",
            "request_id": 1,
            "expected_revision": 0,
            "site": {
                "function_ordinal": site.function_ordinal(),
                "block_ordinal": site.block_ordinal(),
                "point": {
                    "kind": "operation",
                    "operation_ordinal": site.operation_ordinal(),
                },
            },
        }),
        json!({
            "operation": "set_breakpoints",
            "schema": "fe2o3-debug-request-v1",
            "request_id": 2,
            "expected_revision": 0,
            "breakpoints": [{
                "enabled": true,
                "kind": {
                    "kind": "source",
                    "source": {
                        "map_identity": hex(&map_identity),
                        "provenance": "compiler_bundle_bound",
                        "file_identity": hex(&source_file.identity()),
                        "byte_start": breakpoint_byte,
                        "byte_end": breakpoint_byte + 1,
                    },
                },
            }],
        }),
        json!({
            "operation": "continue",
            "schema": "fe2o3-debug-request-v1",
            "request_id": 3,
            "expected_revision": 1,
            "max_events": 65536,
        }),
        json!({
            "operation": "inspect_stack",
            "schema": "fe2o3-debug-request-v1",
            "request_id": 4,
            "expected_revision": 2,
            "scope": { "level": "dispatch" },
            "page": { "limit": 16 },
        }),
        json!({
            "operation": "step",
            "schema": "fe2o3-debug-request-v1",
            "request_id": 5,
            "expected_revision": 2,
            "direction": "forward",
            "granularity": "source",
            "count": 1,
        }),
    ] {
        serde_json::to_writer(&mut protocol_input, &request).unwrap();
        protocol_input.push(b'\n');
    }

    let debug_target = target.path().join("debug-cli-target");
    let build_debugger = Command::new(env!("CARGO"))
        .current_dir(workspace())
        .args([
            "build",
            "--quiet",
            "--locked",
            "-p",
            "fe2o3-debug-cli",
            "--bin",
            "fe2o3-debug",
            "--target-dir",
        ])
        .arg(&debug_target)
        .output()
        .expect("build standalone debugger for compiler-output integration");
    assert!(
        build_debugger.status.success(),
        "debugger build failed:\n{}",
        String::from_utf8_lossy(&build_debugger.stderr)
    );
    let mut debugger = Command::new(debug_target.join("debug/fe2o3-debug"))
        .args(["sim", "--bundle"])
        .arg(&bundle_path)
        .arg("--request")
        .arg(&simulation_request_path)
        .args(["--protocol", "jsonl", "--wave-width", "64"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("run debugger on compiler-produced simulation bundle");
    debugger
        .stdin
        .take()
        .unwrap()
        .write_all(&protocol_input)
        .unwrap();
    let debug_output = debugger.wait_with_output().unwrap();
    assert!(
        debug_output.status.success(),
        "debugger rejected compiler output:\n{}",
        String::from_utf8_lossy(&debug_output.stderr)
    );
    assert!(debug_output.stderr.is_empty());
    let responses = debug_output
        .stdout
        .split(|byte| *byte == b'\n')
        .filter(|line| !line.is_empty())
        .map(|line| serde_json::from_slice::<Value>(line).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(responses.len(), 5);
    assert!(
        responses.iter().all(|response| response["status"] == "ok"),
        "debugger returned a typed error response: {responses:#?}",
    );
    assert_eq!(responses[0]["result"]["result"], "source");
    assert_eq!(
        responses[0]["result"]["site"]["source"]["location"]["provenance"],
        "compiler_bundle_bound"
    );
    assert_eq!(
        responses[0]["result"]["site"]["source"]["location"]["map_identity"],
        hex(&map_identity)
    );
    assert_eq!(responses[2]["result"]["stop"]["reason"], "breakpoint");
    assert_eq!(responses[3]["result"]["result"], "stack");
    assert!(
        responses[3]["result"]["frames"]
            .as_array()
            .is_some_and(|frames| !frames.is_empty())
    );
    assert_eq!(responses[4]["operation"], "step");
    assert!(responses.iter().all(|response| {
        response["session"]["simulated"] == true
            && response["session"]["hardware_observed"] == false
            && response["session"]["performance_prediction"] == false
    }));
}

struct ExtractionOutput {
    status: std::process::ExitStatus,
    stderr: String,
}

fn run_extraction(target: &ScratchTarget, oob: bool) -> ExtractionOutput {
    let mut command = base_command("check", target.path());
    command
        .env("FE2O3_EXTRACT_RANKED_MEMORY_V1", "1")
        .env(
            "RUSTC_WORKSPACE_WRAPPER",
            env!("CARGO_BIN_EXE_fe2o3-rustc-extract"),
        )
        .env(
            "FE2O3_EXTRACT_CRATE_V1",
            "fe2o3_production_ranked_bounds_fixture",
        );
    if oob {
        command.args(["--features", "oob"]);
    }
    output(command, "run AMD extraction fixture")
}

fn run_feature_extraction(target: &ScratchTarget, feature: &str) -> ExtractionOutput {
    let mut command = base_command("check", target.path());
    command
        .env("FE2O3_EXTRACT_RANKED_MEMORY_V1", "1")
        .env(
            "RUSTC_WORKSPACE_WRAPPER",
            env!("CARGO_BIN_EXE_fe2o3-rustc-extract"),
        )
        .env(
            "FE2O3_EXTRACT_CRATE_V1",
            "fe2o3_production_ranked_bounds_fixture",
        )
        .args(["--features", feature]);
    output(command, "run safe mapped AMD extraction fixture")
}

fn base_command(action: &str, target_dir: &Path) -> Command {
    let mut command = Command::new(env!("CARGO"));
    command
        .current_dir(workspace())
        .env(
            "FE2O3_CARGO_METADATA_BUILD_OBSERVATION_V2",
            "55".repeat(32),
        )
        .env("FE2O3_CRATE_BINDING_ID_V1", "77".repeat(32))
        .env_remove("RUSTFLAGS")
        .env_remove("CARGO_ENCODED_RUSTFLAGS")
        .env(
            "CARGO_TARGET_AMDGCN_AMD_AMDHSA_RUSTFLAGS",
            "-Zalways-encode-mir -Ctarget-cpu=gfx942 -Ctarget-feature=-xnack,+wavefrontsize64,-wavefrontsize32",
        )
        .args([
            action,
            "--locked",
            "-Zbuild-std=core",
            "-p",
            "fe2o3-production-ranked-bounds-fixture",
            "--target",
            "amdgcn-amd-amdhsa",
            "--target-dir",
        ])
        .arg(target_dir);
    command
}

fn output(mut command: Command, label: &str) -> ExtractionOutput {
    let output = command
        .output()
        .unwrap_or_else(|error| panic!("{label}: {error}"));
    ExtractionOutput {
        status: output.status,
        stderr: String::from_utf8(output.stderr).expect("rustc diagnostic is UTF-8"),
    }
}
