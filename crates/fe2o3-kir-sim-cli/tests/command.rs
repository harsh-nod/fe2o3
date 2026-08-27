#![cfg(target_os = "linux")]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, BasicBlock, BlockId, Function, Kernel, LaunchDomain, LaunchExtent,
    MAX_SIMULATION_BUNDLE_BYTES_V1, Module, ScalarType, Signature,
    SimulationCompilerExecutionBindingV1, SimulationProductionKirIdentityV1,
    SimulationSourceLineageV1, Terminator, Type, ValueId, VerifiedCanonicalKernelIrV7,
    VerifiedCanonicalKernelIrV8, VerifiedSimulationBundleV1,
};

static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new() -> Self {
        let sequence = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "fe2o3-kir-sim-command-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn binary() -> Command {
    Command::new(env!("CARGO_BIN_EXE_fe2o3-kir-sim"))
}

fn noop_with_buffer_module() -> Module {
    let element = Type::Scalar(ScalarType::U8);
    let slice = Type::slice(element, AddressSpace::Global, AccessMode::ReadOnly);
    let mut block = BasicBlock::new(BlockId(0));
    block.terminator = Some(Terminator::Return { values: vec![] });
    let entry = Function::kernel_entry(
        "entry",
        Signature::new(vec![slice], vec![]),
        vec![ValueId(0)],
        vec![block],
    );
    let mut module = Module::new("cli-command-test");
    module.functions.push(entry);
    module.kernels.push(Kernel::new(
        "kernel",
        "entry",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    ));
    module
}

fn canonical_noop_with_buffer() -> Vec<u8> {
    VerifiedCanonicalKernelIrV7::from_module(noop_with_buffer_module())
        .unwrap()
        .into_canonical_bytes()
}

fn simulation_bundle(target: &str) -> Vec<u8> {
    let module = noop_with_buffer_module();
    let production = VerifiedCanonicalKernelIrV8::from_module(module.clone()).unwrap();
    let production_identity = SimulationProductionKirIdentityV1::v8(
        *production.identity().digest(),
        production.identity().canonical_length(),
    )
    .unwrap();
    VerifiedSimulationBundleV1::new(
        SimulationCompilerExecutionBindingV1::UnavailableExtractionOnly,
        SimulationSourceLineageV1::new([1; 32], 11, [2; 32], 22).unwrap(),
        production_identity,
        target,
        VerifiedCanonicalKernelIrV7::from_module(module).unwrap(),
        None,
    )
    .unwrap()
    .into_canonical_bytes()
}

fn write_success_fixture(directory: &TestDirectory) -> (PathBuf, PathBuf) {
    let kir = directory.path().join("kernel.kir");
    let request = directory.path().join("request.json");
    fs::write(&kir, canonical_noop_with_buffer()).unwrap();
    fs::write(
        &request,
        br#"{"schema":"fe2o3-simulation-request-v1","kernel":"kernel","grid":[2,1,1],"workgroup":[1,1,1],"arguments":[{"kind":"buffer","element":"u8","access":"read_only","alignment":1,"bytes":"0x2a"}]}"#,
    )
    .unwrap();
    (kir, request)
}

#[test]
fn help_is_a_successful_input_free_command() {
    for argument in ["--help", "-h"] {
        let output = binary().arg(argument).env_clear().output().unwrap();
        assert!(output.status.success());
        assert_eq!(
            String::from_utf8(output.stdout).unwrap(),
            "usage: fe2o3-kir-sim (--kir-v7 PATH | --bundle PATH) --request PATH [--output PATH]\n"
        );
        assert!(output.stderr.is_empty());
    }
}

#[test]
fn bundle_targets_execute_exact_embedded_kir_without_authority() {
    for target in ["gfx942:xnack-", "gfx950:xnack-"] {
        let directory = TestDirectory::new();
        let bundle_path = directory.path().join("kernel.fe2sim");
        let request = directory.path().join("request.json");
        fs::write(&bundle_path, simulation_bundle(target)).unwrap();
        fs::write(
            &request,
            br#"{"schema":"fe2o3-simulation-request-v1","kernel":"kernel","grid":[2,1,1],"workgroup":[1,1,1],"arguments":[{"kind":"buffer","element":"u8","access":"read_only","alignment":1,"bytes":"0x2a"}]}"#,
        )
        .unwrap();

        let admitted =
            fe2o3_kir_sim_cli::load_debug_simulation_bundle_v1(&bundle_path, &request).unwrap();
        assert_eq!(admitted.bundle().target(), target);
        assert_eq!(
            admitted.input().simulation_target(),
            fe2o3_kir_sim::SimulationTargetV1::amdgpu_64()
        );
        assert_eq!(
            admitted.input().simulation_bundle_subject(),
            Some(*admitted.bundle().subject_identity())
        );
        assert!(!admitted.grants_proof_authority());
        assert!(!admitted.grants_artifact_authority());
        assert!(!admitted.grants_compiler_authority());
        assert!(!admitted.grants_hardware_authority());
        assert!(!admitted.grants_load_authority());
        assert!(!admitted.grants_launch_authority());
        assert!(!admitted.authenticates_compiler_execution());

        let output = binary()
            .arg("--bundle")
            .arg(bundle_path)
            .arg("--request")
            .arg(request)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(value["status"], "ok");
        assert_eq!(value["authority"], "observation_only");
        assert_eq!(value["simulated"], true);
        assert_eq!(value["hardware_observed"], false);
        assert_eq!(value["performance_prediction"], false);
    }
}

#[test]
fn bundle_command_line_is_exclusive_and_hostile_bundles_fail_closed() {
    let directory = TestDirectory::new();
    let request = directory.path().join("request.json");
    fs::write(
        &request,
        br#"{"schema":"fe2o3-simulation-request-v1","kernel":"kernel","grid":[1,1,1],"workgroup":[1,1,1],"arguments":[{"kind":"buffer","element":"u8","access":"read_only","alignment":1,"bytes":"0x2a"}]}"#,
    )
    .unwrap();
    let valid = simulation_bundle("gfx942:xnack-");
    let bundle = directory.path().join("kernel.fe2sim");
    fs::write(&bundle, &valid).unwrap();
    let kir = directory.path().join("kernel.kir");
    fs::write(&kir, canonical_noop_with_buffer()).unwrap();

    let exclusive = binary()
        .arg("--kir-v7")
        .arg(&kir)
        .arg("--bundle")
        .arg(&bundle)
        .arg("--request")
        .arg(&request)
        .output()
        .unwrap();
    assert!(!exclusive.status.success());
    let error: serde_json::Value = serde_json::from_slice(&exclusive.stderr).unwrap();
    assert_eq!(error["stage"], "arguments");
    assert_eq!(error["kind"], "invalid_command_line");

    let non_regular = binary()
        .args(["--bundle", "/dev/null"])
        .arg("--request")
        .arg(&request)
        .output()
        .unwrap();
    assert!(!non_regular.status.success());
    let error: serde_json::Value = serde_json::from_slice(&non_regular.stderr).unwrap();
    assert_eq!(error["kind"], "input_not_regular");
    assert_eq!(error["input"], "simulation_bundle");

    for (name, bytes) in [
        ("corrupted", {
            let mut bytes = valid.clone();
            bytes[0] ^= 1;
            bytes
        }),
        ("trailing", {
            let mut bytes = valid.clone();
            bytes.push(0);
            bytes
        }),
    ] {
        let path = directory.path().join(format!("{name}.fe2sim"));
        fs::write(&path, bytes).unwrap();
        let output = binary()
            .arg("--bundle")
            .arg(path)
            .arg("--request")
            .arg(&request)
            .output()
            .unwrap();
        assert!(!output.status.success());
        let error: serde_json::Value = serde_json::from_slice(&output.stderr).unwrap();
        assert_eq!(error["stage"], "input");
        assert_eq!(error["kind"], "simulation_bundle_rejected");
        assert_eq!(error["input"], "simulation_bundle");
    }

    let oversized = directory.path().join("oversized.fe2sim");
    fs::File::create(&oversized)
        .unwrap()
        .set_len(u64::try_from(MAX_SIMULATION_BUNDLE_BYTES_V1).unwrap() + 1)
        .unwrap();
    let output = binary()
        .arg("--bundle")
        .arg(oversized)
        .arg("--request")
        .arg(request)
        .output()
        .unwrap();
    assert!(!output.status.success());
    let error: serde_json::Value = serde_json::from_slice(&output.stderr).unwrap();
    assert_eq!(error["kind"], "input_too_large");
    assert_eq!(error["input"], "simulation_bundle");
}

#[test]
fn unsupported_bundle_target_and_request_kernel_mismatch_are_typed() {
    let directory = TestDirectory::new();
    let bundle = directory.path().join("kernel.fe2sim");
    let request = directory.path().join("request.json");
    fs::write(&bundle, simulation_bundle("unmapped-target-v1")).unwrap();
    fs::write(
        &request,
        br#"{"schema":"fe2o3-simulation-request-v1","kernel":"kernel","grid":[1,1,1],"workgroup":[1,1,1],"arguments":[{"kind":"buffer","element":"u8","access":"read_only","alignment":1,"bytes":"0x2a"}]}"#,
    )
    .unwrap();
    let output = binary()
        .arg("--bundle")
        .arg(&bundle)
        .arg("--request")
        .arg(&request)
        .output()
        .unwrap();
    assert!(!output.status.success());
    let error: serde_json::Value = serde_json::from_slice(&output.stderr).unwrap();
    assert_eq!(error["kind"], "simulation_bundle_target_unsupported");

    fs::write(&bundle, simulation_bundle("gfx942:xnack-")).unwrap();
    fs::write(
        &request,
        br#"{"schema":"fe2o3-simulation-request-v1","kernel":"stale_kernel","grid":[1,1,1],"workgroup":[1,1,1],"arguments":[]}"#,
    )
    .unwrap();
    let output = binary()
        .arg("--bundle")
        .arg(bundle)
        .arg("--request")
        .arg(request)
        .output()
        .unwrap();
    assert!(!output.status.success());
    let error: serde_json::Value = serde_json::from_slice(&output.stderr).unwrap();
    assert_eq!(error["stage"], "preflight");
    assert_eq!(error["kind"], "preflight_unknown_kernel");
}

#[test]
fn legacy_kir_flag_is_not_a_second_simulator_route() {
    let output = binary()
        .args(["--kir-v6", "kernel.kir", "--request", "request.json"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    let value: serde_json::Value = serde_json::from_slice(&output.stderr).unwrap();
    assert_eq!(value["stage"], "arguments");
    assert_eq!(value["kind"], "invalid_command_line");
}

#[test]
fn successful_stdout_is_complete_machine_readable_json() {
    let directory = TestDirectory::new();
    let (kir, request) = write_success_fixture(&directory);
    let output = binary()
        .arg("--kir-v7")
        .arg(kir)
        .arg("--request")
        .arg(request)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    assert_eq!(output.stdout.last(), Some(&b'\n'));
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["schema"], "fe2o3-simulation-result-v1");
    assert_eq!(value["status"], "ok");
    assert_eq!(value["authority"], "observation_only");
    assert_eq!(value["simulated"], true);
    assert_eq!(value["hardware_observed"], false);
    assert_eq!(value["hardware_validation"], false);
    assert_eq!(value["performance_prediction"], false);
    assert_eq!(
        value["target_profile"]["identity"],
        "amdgpu_64_little_endian_v1"
    );
    assert_eq!(value["target_profile"]["index_bits"], 64);
    assert_eq!(value["target_profile"]["max_workgroup_invocations"], 1024);
    assert_eq!(
        value["schedule"]["identity"],
        "workgroup_major_local_zyx_cooperative_v1"
    );
    assert_eq!(value["kir"]["sha256"].as_str().unwrap().len(), 64);
    assert!(value["kir"]["canonical_bytes"].as_u64().unwrap() > 0);
    assert_eq!(value["counts"]["invocations_executed"], 2);
    assert_eq!(value["counts"]["scheduled_slots_visited"], 2);
    assert_eq!(value["arguments"][0]["value"]["bytes"], "0x2a");
}

#[test]
fn successful_output_is_private_durable_no_replace_json() {
    let directory = TestDirectory::new();
    let (kir, request) = write_success_fixture(&directory);
    let result = directory.path().join("result.json");
    let output = binary()
        .arg("--kir-v7")
        .arg(&kir)
        .arg("--request")
        .arg(&request)
        .arg("--output")
        .arg(&result)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
    let bytes = fs::read(&result).unwrap();
    let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(value["status"], "ok");
    assert_eq!(
        fs::metadata(&result).unwrap().permissions().mode() & 0o777,
        0o600
    );

    let second = binary()
        .arg("--kir-v7")
        .arg(kir)
        .arg("--request")
        .arg(request)
        .arg("--output")
        .arg(&result)
        .output()
        .unwrap();
    assert!(!second.status.success());
    let error: serde_json::Value = serde_json::from_slice(&second.stderr).unwrap();
    assert_eq!(error["stage"], "output");
    assert_eq!(error["kind"], "output_already_exists");
    assert_eq!(fs::read(result).unwrap(), bytes);
}

#[test]
fn failures_are_stable_json_with_input_identity() {
    let output = binary().output().unwrap();
    assert!(!output.status.success());
    let value: serde_json::Value = serde_json::from_slice(&output.stderr).unwrap();
    assert_eq!(value["stage"], "arguments");
    assert_eq!(value["kind"], "invalid_command_line");

    let output = binary()
        .args(["--kir-v7", "/dev/null", "--request", "/dev/null"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    let value: serde_json::Value = serde_json::from_slice(&output.stderr).unwrap();
    assert_eq!(value["stage"], "input");
    assert_eq!(value["kind"], "input_not_regular");
    assert_eq!(value["input"], "kir_v7");

    let directory = TestDirectory::new();
    let oversized = directory.path().join("oversized.kir");
    fs::File::create(&oversized)
        .unwrap()
        .set_len(16 * 1024 * 1024 + 1)
        .unwrap();
    let output = binary()
        .arg("--kir-v7")
        .arg(&oversized)
        .args(["--request", "/dev/null"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    let value: serde_json::Value = serde_json::from_slice(&output.stderr).unwrap();
    assert_eq!(value["kind"], "input_too_large");
    assert_eq!(value["input"], "kir_v7");
}

#[test]
fn closed_stdout_reports_real_epipe_without_gpu_runtime() {
    let directory = TestDirectory::new();
    let kir = directory.path().join("kernel.kir");
    let request = directory.path().join("request.json");
    fs::write(&kir, canonical_noop_with_buffer()).unwrap();
    let bytes = "00".repeat(4 * 1024 * 1024);
    fs::write(
        &request,
        format!(
            "{{\"schema\":\"fe2o3-simulation-request-v1\",\"kernel\":\"kernel\",\"grid\":[1,1,1],\"workgroup\":[1,1,1],\"arguments\":[{{\"kind\":\"buffer\",\"element\":\"u8\",\"access\":\"read_only\",\"alignment\":1,\"bytes\":\"0x{bytes}\"}}]}}"
        ),
    )
    .unwrap();

    let mut child = binary()
        .arg("--kir-v7")
        .arg(&kir)
        .arg("--request")
        .arg(&request)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    drop(child.stdout.take());
    let output = child.wait_with_output().unwrap();
    assert!(!output.status.success());
    let value: serde_json::Value = serde_json::from_slice(&output.stderr).unwrap();
    assert_eq!(value["stage"], "output");
    assert_eq!(value["kind"], "output_write_failed");
}

#[test]
fn bound_request_rejects_post_admission_substitution_without_output() {
    let directory = TestDirectory::new();
    let (_, request) = write_success_fixture(&directory);
    let identity = fe2o3_kir_sim_cli::bind_request_v1(&request).unwrap();
    fs::write(
        &request,
        br#"{"schema":"fe2o3-simulation-request-v1","kernel":"kernel","grid":[3,1,1],"workgroup":[1,1,1],"arguments":[{"kind":"buffer","element":"u8","access":"read_only","alignment":1,"bytes":"0x2a"}]}"#,
    )
    .unwrap();
    let output = directory.path().join("result.json");

    let status = fe2o3_kir_sim_cli::run_captured_kir_v7_with_bound_request(
        &canonical_noop_with_buffer(),
        &request,
        identity,
        Some(&output),
    );

    assert_ne!(status, std::process::ExitCode::SUCCESS);
    assert!(!output.exists());
}
