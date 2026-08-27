#![cfg(target_os = "linux")]

use std::fmt::Write as _;
use std::fs;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, BasicBlock, BlockId, DebugSourceMapDocumentV1, DebugSourceMapFileV1,
    Function, Kernel, LaunchDomain, LaunchExtent, MAX_SIMULATION_BUNDLE_BYTES_V1, Module,
    PreparedSimulationBundleV1, ScalarType, Signature, SimulationCompilerExecutionBindingV1,
    SimulationProductionKirIdentityV1, SimulationSourceLineageV1, Terminator, Type, ValueId,
    VerifiedCanonicalKernelIrV7, VerifiedCanonicalKernelIrV8,
};
use fe2o3_kir_sim::MAX_PERSISTED_SCHEDULE_BYTES_V1;
use sha2::{Digest, Sha256};

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

fn canonical_noop_with_buffer_variant() -> Vec<u8> {
    let mut module = noop_with_buffer_module();
    module.id = "cli-command-test-variant".into();
    VerifiedCanonicalKernelIrV7::from_module(module)
        .unwrap()
        .into_canonical_bytes()
}

fn simulation_bundle(target: &str) -> Vec<u8> {
    simulation_bundle_with_debug_map(target, None)
}

fn simulation_bundle_with_debug_map(target: &str, debug_source: Option<&[u8]>) -> Vec<u8> {
    let module = noop_with_buffer_module();
    let production = VerifiedCanonicalKernelIrV8::from_module(module.clone()).unwrap();
    let production_identity = SimulationProductionKirIdentityV1::v8(
        *production.identity().digest(),
        production.identity().canonical_length(),
    )
    .unwrap();
    let prepared = PreparedSimulationBundleV1::new(
        SimulationCompilerExecutionBindingV1::UnavailableExtractionOnly,
        SimulationSourceLineageV1::new([1; 32], 11, [2; 32], 22).unwrap(),
        production_identity,
        target,
        VerifiedCanonicalKernelIrV7::from_module(module).unwrap(),
    )
    .unwrap();
    let bundle = if let Some(source) = debug_source {
        let source_identity: [u8; 32] = Sha256::digest(source).into();
        let document = DebugSourceMapDocumentV1::new(
            prepared.debug_source_map_binding(),
            vec![
                DebugSourceMapFileV1::new(
                    source_identity,
                    u64::try_from(source.len()).unwrap(),
                    "schedule-test.rs".to_owned(),
                )
                .unwrap(),
            ],
            Vec::new(),
            Vec::new(),
        )
        .unwrap();
        prepared.finalize_with_source_map(document).unwrap()
    } else {
        prepared.finalize_without_source_map().unwrap()
    };
    bundle.into_canonical_bytes()
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
            "usage: fe2o3-kir-sim (--kir-v7 PATH | --bundle PATH) --request PATH [--output PATH] [--record-canonical-schedule PATH [--schedule-max-decisions COUNT] | --record-seeded-schedule PATH --schedule-seed U64 [--schedule-max-decisions COUNT] | --replay-schedule PATH]\n"
        );
        assert!(output.stderr.is_empty());
    }
}

#[test]
fn schedule_modes_and_record_only_parameters_are_mutually_exclusive() {
    for arguments in [
        vec![
            "--record-canonical-schedule",
            "canonical.json",
            "--replay-schedule",
            "replay.json",
        ],
        vec![
            "--record-canonical-schedule",
            "canonical.json",
            "--record-seeded-schedule",
            "seeded.json",
            "--schedule-seed",
            "7",
        ],
        vec!["--record-seeded-schedule", "seeded.json"],
        vec!["--replay-schedule", "replay.json", "--schedule-seed", "7"],
        vec![
            "--replay-schedule",
            "replay.json",
            "--schedule-max-decisions",
            "8",
        ],
    ] {
        let output = binary()
            .args(["--kir-v7", "missing.kir", "--request", "missing.json"])
            .args(arguments)
            .output()
            .unwrap();
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        let error: serde_json::Value = serde_json::from_slice(&output.stderr).unwrap();
        assert_eq!(error["stage"], "arguments");
        assert_eq!(error["kind"], "invalid_command_line");
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
        assert_eq!(
            admitted.input().simulation_bundle_identity(),
            Some(*admitted.bundle().identity().as_bytes())
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
fn canonical_and_seeded_schedules_persist_and_replay_for_raw_kir() {
    let directory = TestDirectory::new();
    let (kir, request) = write_success_fixture(&directory);

    for (record_flag, seed, expected_identity) in [
        (
            "--record-canonical-schedule",
            None,
            "workgroup_major_local_zyx_cooperative_v1",
        ),
        (
            "--record-seeded-schedule",
            Some("18446744073709551615"),
            "workgroup_major_seeded_runnable_cooperative_v1",
        ),
    ] {
        let schedule = directory
            .path()
            .join(format!("{}.json", record_flag.trim_start_matches("--")));
        let mut record = binary();
        record
            .arg("--kir-v7")
            .arg(&kir)
            .arg("--request")
            .arg(&request)
            .arg(record_flag)
            .arg(&schedule)
            .args(["--schedule-max-decisions", "8"]);
        if let Some(seed) = seed {
            record.args(["--schedule-seed", seed]);
        }
        let output = record.output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let result: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(result["schedule"]["identity"], expected_identity);
        let schedule_bytes = fs::read(&schedule).unwrap();
        assert!(!schedule_bytes.ends_with(b"\n"));
        assert_eq!(
            fs::metadata(&schedule).unwrap().permissions().mode() & 0o777,
            0o600
        );
        let persisted: serde_json::Value = serde_json::from_slice(&schedule_bytes).unwrap();
        assert_eq!(persisted["schema"], "fe2o3-simulation-schedule-v1");
        assert_eq!(persisted["artifact"]["kind"], "canonical_kir_v7");
        assert_eq!(persisted["schedule"]["identity"], expected_identity);
        assert_eq!(persisted["coverage"]["complete"], true);
        assert_eq!(persisted["decisions"].as_array().unwrap().len(), 2);
        if seed.is_some() {
            let mut schedule_sha256 = String::with_capacity(64);
            for byte in Sha256::digest(&schedule_bytes) {
                write!(&mut schedule_sha256, "{byte:02x}").unwrap();
            }
            assert_eq!(schedule_bytes.len(), 1_346);
            assert_eq!(
                schedule_sha256,
                "a3d0a28479bf6ee12a9bb10745903a208f815213aa94771768407b662eb369cc"
            );
        }

        let replay = binary()
            .arg("--kir-v7")
            .arg(&kir)
            .arg("--request")
            .arg(&request)
            .arg("--replay-schedule")
            .arg(&schedule)
            .output()
            .unwrap();
        assert!(
            replay.status.success(),
            "{}",
            String::from_utf8_lossy(&replay.stderr)
        );
        let replayed: serde_json::Value = serde_json::from_slice(&replay.stdout).unwrap();
        assert_eq!(replayed["schedule"]["identity"], expected_identity);
        assert_eq!(
            replayed["schedule"]["transcript_sha256"],
            result["schedule"]["transcript_sha256"]
        );
        assert_eq!(replayed["arguments"], result["arguments"]);
    }
}

#[test]
fn bundle_schedule_binds_exact_bundle_subject_and_embedded_kir() {
    let directory = TestDirectory::new();
    let bundle = directory.path().join("kernel.fe2sim");
    let request = directory.path().join("request.json");
    let schedule = directory.path().join("schedule.json");
    fs::write(
        &bundle,
        simulation_bundle_with_debug_map("gfx942:xnack-", Some(b"debug-map-a")),
    )
    .unwrap();
    fs::write(
        &request,
        br#"{"schema":"fe2o3-simulation-request-v1","kernel":"kernel","grid":[2,1,1],"workgroup":[1,1,1],"arguments":[{"kind":"buffer","element":"u8","access":"read_only","alignment":1,"bytes":"0x2a"}]}"#,
    )
    .unwrap();

    let recorded = binary()
        .arg("--bundle")
        .arg(&bundle)
        .arg("--request")
        .arg(&request)
        .arg("--record-canonical-schedule")
        .arg(&schedule)
        .args(["--schedule-max-decisions", "8"])
        .output()
        .unwrap();
    assert!(
        recorded.status.success(),
        "{}",
        String::from_utf8_lossy(&recorded.stderr)
    );
    let persisted: serde_json::Value =
        serde_json::from_slice(&fs::read(&schedule).unwrap()).unwrap();
    assert_eq!(persisted["artifact"]["kind"], "simulation_bundle_v1");
    assert_eq!(
        persisted["artifact"]["bundle_sha256"]
            .as_str()
            .unwrap()
            .len(),
        64
    );
    assert_eq!(
        persisted["artifact"]["subject_sha256"]
            .as_str()
            .unwrap()
            .len(),
        64
    );

    let replayed = binary()
        .arg("--bundle")
        .arg(&bundle)
        .arg("--request")
        .arg(&request)
        .arg("--replay-schedule")
        .arg(&schedule)
        .output()
        .unwrap();
    assert!(
        replayed.status.success(),
        "{}",
        String::from_utf8_lossy(&replayed.stderr)
    );

    let substituted_bundle = directory.path().join("substituted.fe2sim");
    fs::write(
        &substituted_bundle,
        simulation_bundle_with_debug_map("gfx942:xnack-", Some(b"debug-map-b")),
    )
    .unwrap();
    let substituted = binary()
        .arg("--bundle")
        .arg(substituted_bundle)
        .arg("--request")
        .arg(&request)
        .arg("--replay-schedule")
        .arg(&schedule)
        .output()
        .unwrap();
    assert!(!substituted.status.success());
    assert!(substituted.stdout.is_empty());
    let error: serde_json::Value = serde_json::from_slice(&substituted.stderr).unwrap();
    assert_eq!(error["kind"], "schedule_binding_mismatch");

    let raw_kir = directory.path().join("kernel.kir");
    fs::write(&raw_kir, canonical_noop_with_buffer()).unwrap();
    let substituted = binary()
        .arg("--kir-v7")
        .arg(raw_kir)
        .arg("--request")
        .arg(&request)
        .arg("--replay-schedule")
        .arg(&schedule)
        .output()
        .unwrap();
    assert!(!substituted.status.success());
    let error: serde_json::Value = serde_json::from_slice(&substituted.stderr).unwrap();
    assert_eq!(error["kind"], "schedule_binding_mismatch");
    assert_eq!(error["input"], "semantic_schedule");
}

#[test]
fn hostile_schedule_inputs_and_stale_requests_fail_before_execution() {
    let directory = TestDirectory::new();
    let (kir, request) = write_success_fixture(&directory);
    let schedule = directory.path().join("schedule.json");
    let recorded = binary()
        .arg("--kir-v7")
        .arg(&kir)
        .arg("--request")
        .arg(&request)
        .arg("--record-canonical-schedule")
        .arg(&schedule)
        .args(["--schedule-max-decisions", "8"])
        .output()
        .unwrap();
    assert!(recorded.status.success());

    let original_request = fs::read(&request).unwrap();
    let mut stale_request = original_request.clone();
    stale_request.push(b'\n');
    fs::write(&request, stale_request).unwrap();
    let stale = binary()
        .arg("--kir-v7")
        .arg(&kir)
        .arg("--request")
        .arg(&request)
        .arg("--replay-schedule")
        .arg(&schedule)
        .output()
        .unwrap();
    assert!(!stale.status.success());
    let error: serde_json::Value = serde_json::from_slice(&stale.stderr).unwrap();
    assert_eq!(error["kind"], "schedule_binding_mismatch");
    fs::write(&request, original_request).unwrap();

    let variant_kir = directory.path().join("variant.kir");
    fs::write(&variant_kir, canonical_noop_with_buffer_variant()).unwrap();
    let substituted = binary()
        .arg("--kir-v7")
        .arg(&variant_kir)
        .arg("--request")
        .arg(&request)
        .arg("--replay-schedule")
        .arg(&schedule)
        .output()
        .unwrap();
    assert!(!substituted.status.success());
    let error: serde_json::Value = serde_json::from_slice(&substituted.stderr).unwrap();
    assert_eq!(error["kind"], "schedule_binding_mismatch");

    let schedule_text = String::from_utf8(fs::read(&schedule).unwrap()).unwrap();
    for (name, substituted) in [
        (
            "limits",
            schedule_text.replacen("\"max_events\":1,", "\"max_events\":2,", 1),
        ),
        (
            "target",
            schedule_text.replacen(
                "\"identity\":\"amdgpu_64_little_endian_v1\",\"index_bits\":64",
                "\"identity\":\"little_endian_index32_v1\",\"index_bits\":32",
                1,
            ),
        ),
    ] {
        let path = directory.path().join(format!("{name}-substitution.json"));
        fs::write(&path, substituted).unwrap();
        let rejected = binary()
            .arg("--kir-v7")
            .arg(&kir)
            .arg("--request")
            .arg(&request)
            .arg("--replay-schedule")
            .arg(path)
            .output()
            .unwrap();
        assert!(!rejected.status.success(), "{name}");
        let error: serde_json::Value = serde_json::from_slice(&rejected.stderr).unwrap();
        assert_eq!(error["kind"], "schedule_binding_mismatch", "{name}");
    }

    let noncanonical = directory.path().join("noncanonical.json");
    let mut bytes = fs::read(&schedule).unwrap();
    bytes.push(b'\n');
    fs::write(&noncanonical, bytes).unwrap();
    let invalid = binary()
        .arg("--kir-v7")
        .arg(&kir)
        .arg("--request")
        .arg(&request)
        .arg("--replay-schedule")
        .arg(&noncanonical)
        .output()
        .unwrap();
    assert!(!invalid.status.success());
    let error: serde_json::Value = serde_json::from_slice(&invalid.stderr).unwrap();
    assert_eq!(error["kind"], "schedule_codec_rejected");

    let linked = directory.path().join("linked.json");
    symlink(&schedule, &linked).unwrap();
    for path in [linked, PathBuf::from("/dev/null")] {
        let rejected = binary()
            .arg("--kir-v7")
            .arg(&kir)
            .arg("--request")
            .arg(&request)
            .arg("--replay-schedule")
            .arg(path)
            .output()
            .unwrap();
        assert!(!rejected.status.success());
        let error: serde_json::Value = serde_json::from_slice(&rejected.stderr).unwrap();
        assert!(matches!(
            error["kind"].as_str(),
            Some("input_open_failed" | "input_not_regular")
        ));
        assert_eq!(error["input"], "semantic_schedule");
    }

    let oversized = directory.path().join("oversized.json");
    fs::File::create(&oversized)
        .unwrap()
        .set_len(u64::try_from(MAX_PERSISTED_SCHEDULE_BYTES_V1).unwrap() + 1)
        .unwrap();
    let rejected = binary()
        .arg("--kir-v7")
        .arg(kir)
        .arg("--request")
        .arg(request)
        .arg("--replay-schedule")
        .arg(oversized)
        .output()
        .unwrap();
    assert!(!rejected.status.success());
    let error: serde_json::Value = serde_json::from_slice(&rejected.stderr).unwrap();
    assert_eq!(error["kind"], "input_too_large");
}

#[test]
fn schedule_recording_is_no_replace_and_emits_nothing_on_execution_failure() {
    let directory = TestDirectory::new();
    let (kir, request) = write_success_fixture(&directory);
    let existing = directory.path().join("existing.json");
    fs::write(&existing, b"retained").unwrap();
    let rejected = binary()
        .arg("--kir-v7")
        .arg(&kir)
        .arg("--request")
        .arg(&request)
        .arg("--record-canonical-schedule")
        .arg(&existing)
        .args(["--schedule-max-decisions", "8"])
        .output()
        .unwrap();
    assert!(!rejected.status.success());
    assert!(rejected.stdout.is_empty());
    assert_eq!(fs::read(&existing).unwrap(), b"retained");

    let absent = directory.path().join("absent.json");
    let failed = binary()
        .arg("--kir-v7")
        .arg(kir)
        .arg("--request")
        .arg(request)
        .arg("--record-canonical-schedule")
        .arg(&absent)
        .args(["--schedule-max-decisions", "1"])
        .output()
        .unwrap();
    assert!(!failed.status.success());
    assert!(failed.stdout.is_empty());
    assert!(!absent.exists());
    let error: serde_json::Value = serde_json::from_slice(&failed.stderr).unwrap();
    assert_eq!(error["kind"], "execution_schedule_decision_limit");
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
fn recorded_schedule_is_disclosed_when_later_stdout_epipe_occurs() {
    let directory = TestDirectory::new();
    let kir = directory.path().join("kernel.kir");
    let request = directory.path().join("request.json");
    let schedule = directory.path().join("schedule.json");
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
        .arg("--record-canonical-schedule")
        .arg(&schedule)
        .args(["--schedule-max-decisions", "8"])
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
    assert_eq!(value["schedule_published"], true);
    let persisted: serde_json::Value =
        serde_json::from_slice(&fs::read(schedule).unwrap()).unwrap();
    assert_eq!(persisted["schema"], "fe2o3-simulation-schedule-v1");
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
